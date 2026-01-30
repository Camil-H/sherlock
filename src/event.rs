use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Event emitted when a request is intercepted by the proxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEvent {
    /// Timestamp when the request was intercepted
    pub timestamp: DateTime<Utc>,
    /// Provider name (anthropic, openai, gemini)
    pub provider: String,
    /// Model identifier
    pub model: String,
    /// Total token count for the request
    pub tokens: usize,
    /// Normalized messages
    pub messages: Vec<Message>,
    /// Raw request body
    pub raw_body: serde_json::Value,
    /// API endpoint path
    pub path: String,
}

/// A normalized message from any provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Simplified request info for dashboard display
#[derive(Debug, Clone)]
pub struct RequestInfo {
    /// Time in HH:MM:SS format
    pub time: String,
    /// Provider name (capitalized)
    pub provider: String,
    /// Model name
    pub model: String,
    /// Token count
    pub tokens: usize,
}

impl From<&RequestEvent> for RequestInfo {
    fn from(event: &RequestEvent) -> Self {
        Self {
            time: event.timestamp.format("%H:%M:%S").to_string(),
            provider: capitalize(&event.provider),
            model: event.model.clone(),
            tokens: event.tokens,
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

impl RequestEvent {
    /// Extract the last user message from the event
    pub fn last_user_message(&self) -> Option<&str> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("anthropic"), "Anthropic");
        assert_eq!(capitalize("openai"), "Openai");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_last_user_message() {
        let event = RequestEvent {
            timestamp: Utc::now(),
            provider: "anthropic".to_string(),
            model: "claude-3".to_string(),
            tokens: 100,
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: "First".to_string(),
                },
                Message {
                    role: "assistant".to_string(),
                    content: "Response".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "Second".to_string(),
                },
            ],
            raw_body: serde_json::json!({}),
            path: "/v1/messages".to_string(),
        };

        assert_eq!(event.last_user_message(), Some("Second"));
    }
}
