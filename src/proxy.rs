use anyhow::Result;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::config::{ProviderConfig, ProxyConfig};
use crate::event::RequestEvent;
use crate::parser::{detect_provider, parse_request};

/// HTTP proxy server that intercepts LLM API requests
pub struct ProxyServer {
    config: ProxyConfig,
    providers: Arc<HashMap<String, ProviderConfig>>,
    client: reqwest::Client,
    event_tx: mpsc::Sender<RequestEvent>,
}

impl ProxyServer {
    pub fn new(
        config: ProxyConfig,
        providers: HashMap<String, ProviderConfig>,
        event_tx: mpsc::Sender<RequestEvent>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            config,
            providers: Arc::new(providers),
            client,
            event_tx,
        }
    }

    pub async fn run(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!("Proxy server listening on {}", addr);

        // Wrap shared state in Arc for cloning into tasks
        let client = Arc::new(self.client);
        let providers = self.providers;
        let event_tx = self.event_tx;

        loop {
            let (stream, remote_addr) = listener.accept().await?;
            let io = TokioIo::new(stream);

            tracing::debug!("Accepted connection from {}", remote_addr);

            // Clone for the spawned task
            let client = Arc::clone(&client);
            let providers = Arc::clone(&providers);
            let event_tx = event_tx.clone();

            tokio::spawn(async move {
                let client = Arc::clone(&client);
                let providers = Arc::clone(&providers);
                let event_tx = event_tx.clone();

                let service = service_fn(move |req| {
                    let client = Arc::clone(&client);
                    let providers = Arc::clone(&providers);
                    let event_tx = event_tx.clone();

                    async move {
                        handle_request(req, &client, &providers, event_tx).await
                    }
                });

                if let Err(e) = http1::Builder::new()
                    .serve_connection(io, service)
                    .await
                {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    client: &reqwest::Client,
    providers: &HashMap<String, ProviderConfig>,
    event_tx: mpsc::Sender<RequestEvent>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    let path = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    tracing::debug!("{} {}", method, path);

    // Detect provider from path
    let provider_name = detect_provider(path, providers);

    if provider_name.is_none() {
        tracing::warn!("Unknown provider for path: {}", path);
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from("Unknown provider")))
            .unwrap());
    }

    let provider_name = provider_name.unwrap();
    let provider_config = providers.get(&provider_name).unwrap();

    // Read body
    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::error!("Failed to read request body: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Failed to read request body")))
                .unwrap());
        }
    };

    // Parse request and emit event (non-blocking)
    if !body_bytes.is_empty() {
        match parse_request(&body_bytes, path, &provider_name) {
            Ok(event) => {
                if let Err(e) = event_tx.try_send(event) {
                    tracing::warn!("Failed to send event: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to parse request: {}", e);
            }
        }
    }

    // Forward to upstream
    let upstream_url = format!("{}{}", provider_config.base_url, path);

    let mut upstream_req = client.request(method_to_reqwest(&method), &upstream_url);

    // Copy headers, skipping hop-by-hop headers
    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if !is_hop_by_hop_header(&name_str) {
            if let Ok(value_str) = value.to_str() {
                upstream_req = upstream_req.header(name.as_str(), value_str);
            }
        }
    }

    // Set body (use Bytes directly to avoid copy)
    upstream_req = upstream_req.body(body_bytes.clone());

    // Send request
    let upstream_resp = match upstream_req.send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Upstream request failed: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!("Upstream error: {}", e))))
                .unwrap());
        }
    };

    // Build response
    let status = upstream_resp.status();
    let resp_headers = upstream_resp.headers().clone();

    let resp_body = match upstream_resp.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Failed to read upstream response: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from("Failed to read upstream response")))
                .unwrap());
        }
    };

    let mut response = Response::builder().status(status.as_u16());

    // Copy response headers
    for (name, value) in resp_headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if !is_hop_by_hop_header(&name_str) && name_str != "content-encoding" {
            if let Ok(value_str) = value.to_str() {
                response = response.header(name.as_str(), value_str);
            }
        }
    }

    Ok(response.body(Full::new(resp_body)).unwrap())
}

fn method_to_reqwest(method: &Method) -> reqwest::Method {
    match *method {
        Method::GET => reqwest::Method::GET,
        Method::POST => reqwest::Method::POST,
        Method::PUT => reqwest::Method::PUT,
        Method::DELETE => reqwest::Method::DELETE,
        Method::PATCH => reqwest::Method::PATCH,
        Method::HEAD => reqwest::Method::HEAD,
        Method::OPTIONS => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    }
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
            | "host"
    )
}
