mod archive;
mod cli;
mod config;
mod dashboard;
mod event;
mod parser;
mod proxy;

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::archive::archive_writer;
use crate::cli::{Cli, Command};
use crate::config::Config;
use crate::dashboard::Dashboard;
use crate::event::RequestEvent;
use crate::proxy::ProxyServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "sherlock=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let cli = Cli::parse();
    let config = Config::load(&cli.config)?;

    match cli.command {
        Command::Start { port, limit } => {
            let config = config.with_overrides(port, limit);
            run_server(config).await?;
        }
        Command::Claude { args } => {
            run_tool("anthropic", "claude", args, &config).await?;
        }
        Command::Happy { args } => {
            run_tool("anthropic", "happy", args, &config).await?;
        }
        Command::Gemini { args } => {
            run_tool("gemini", "gemini", args, &config).await?;
        }
        Command::Codex { args } => {
            run_tool("openai", "codex", args, &config).await?;
        }
        Command::Run { provider, command } => {
            if command.is_empty() {
                anyhow::bail!("No command specified");
            }
            run_tool(&provider, &command[0], command[1..].to_vec(), &config).await?;
        }
    }

    Ok(())
}

async fn run_server(config: Config) -> Result<()> {
    // Create channels for communication
    let (event_tx, event_rx) = mpsc::channel::<RequestEvent>(1000);
    let (archive_tx, archive_rx) = mpsc::channel::<RequestEvent>(100);

    // Spawn proxy server
    let proxy_config = config.proxy.clone();
    let providers = config.providers.clone();
    let proxy = ProxyServer::new(proxy_config, providers, event_tx);

    let proxy_handle = tokio::spawn(async move {
        if let Err(e) = proxy.run().await {
            tracing::error!("Proxy server error: {}", e);
        }
    });

    // Spawn archive writer
    let archive_config = config.archive.clone();
    let archive_handle = tokio::spawn(async move {
        if let Err(e) = archive_writer(archive_rx, archive_config).await {
            tracing::error!("Archive writer error: {}", e);
        }
    });

    // Run dashboard in main task (needs terminal access)
    let dashboard = Dashboard::new(config.dashboard);
    let result = dashboard.run(event_rx, archive_tx).await;

    // Cleanup
    proxy_handle.abort();
    archive_handle.abort();

    result
}

async fn run_tool(
    provider: &str,
    tool_name: &str,
    args: Vec<String>,
    config: &Config,
) -> Result<()> {
    use std::process::Stdio;
    use tokio::process::Command as TokioCommand;

    let provider_config = config
        .providers
        .get(provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", provider))?;

    let proxy_url = format!("http://{}:{}", config.proxy.bind_address, config.proxy.port);

    let mut cmd = TokioCommand::new(tool_name);
    cmd.args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Set environment variables for the provider
    for env_var in &provider_config.env_vars {
        cmd.env(env_var, &proxy_url);
    }

    tracing::info!(
        "Running {} with {} set to {}",
        tool_name,
        provider_config.env_vars.join(", "),
        proxy_url
    );

    let status = cmd.status().await?;

    if !status.success() {
        if let Some(code) = status.code() {
            std::process::exit(code);
        }
    }

    Ok(())
}
