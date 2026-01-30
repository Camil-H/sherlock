use anyhow::Result;
use once_cell::sync::Lazy;
use serde_json::Value;
use tiktoken_rs::CoreBPE;

use crate::event::{Message, RequestEvent};

/// Cached tiktoken encoding for cl100k_base (used by Claude and GPT-4)
static ENCODING: Lazy<CoreBPE> = Lazy::new(|| {
    tiktoken_rs::cl100k_base().expect("Failed to load cl100k_base encoding")
});

/// Count the number of tokens in a text string
pub fn count_tokens(text: &str) -> usize {
    ENCODING.encode_ordinary(text).len()
}

/// Parse a request body and create a RequestEvent
pub fn parse_request(body: &[u8], path: &str, provider: &str) -> Result<RequestEvent> {
    let raw_body: Value = serde_json::from_slice(body)?;

    let (model, messages, total_text) = match provider {
        "anthropic" => parse_anthropic_request(&raw_body)?,
        "openai" => parse_openai_request(&raw_body)?,
        "gemini" => parse_gemini_request(&raw_body)?,
        _ => {
            // Generic fallback
            let model = raw_body
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let text = extract_text_from_value(&raw_body);
            (model, vec![], text)
        }
    };

    let tokens = count_tokens(&total_text);

    Ok(RequestEvent {
        timestamp: chrono::Utc::now(),
        provider: provider.to_string(),
        model,
        tokens,
        messages,
        raw_body,
        path: path.to_string(),
    })
}

/// Parse Anthropic Messages API request
fn parse_anthropic_request(body: &Value) -> Result<(String, Vec<Message>, String)> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let mut messages = Vec::new();
    // Pre-allocate for typical request sizes
    let mut all_text = String::with_capacity(4096);

    // Handle system prompt
    if let Some(system) = body.get("system") {
        let system_text = extract_text_from_value(system);
        if !system_text.is_empty() {
            messages.push(Message {
                role: "system".to_string(),
                content: system_text.clone(),
            });
            all_text.push_str(&system_text);
            all_text.push('\n');
        }
    }

    // Handle messages array
    if let Some(Value::Array(msgs)) = body.get("messages") {
        for msg in msgs {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let content = if let Some(content_val) = msg.get("content") {
                extract_text_from_value(content_val)
            } else {
                String::new()
            };

            if !content.is_empty() {
                all_text.push_str(&content);
                all_text.push('\n');
            }

            messages.push(Message { role, content });
        }
    }

    Ok((model, messages, all_text))
}

/// Parse OpenAI Chat Completions API request
fn parse_openai_request(body: &Value) -> Result<(String, Vec<Message>, String)> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let mut messages = Vec::new();
    // Pre-allocate for typical request sizes
    let mut all_text = String::with_capacity(4096);

    if let Some(Value::Array(msgs)) = body.get("messages") {
        for msg in msgs {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let content = if let Some(content_val) = msg.get("content") {
                extract_text_from_value(content_val)
            } else {
                String::new()
            };

            if !content.is_empty() {
                all_text.push_str(&content);
                all_text.push('\n');
            }

            messages.push(Message { role, content });
        }
    }

    Ok((model, messages, all_text))
}

/// Parse Google Gemini API request
fn parse_gemini_request(body: &Value) -> Result<(String, Vec<Message>, String)> {
    // Gemini model is typically in the URL path, not the body
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gemini")
        .to_string();

    let mut messages = Vec::new();
    // Pre-allocate for typical request sizes
    let mut all_text = String::with_capacity(4096);

    // Handle systemInstruction
    if let Some(system) = body.get("systemInstruction") {
        let system_text = extract_text_from_value(system);
        if !system_text.is_empty() {
            messages.push(Message {
                role: "system".to_string(),
                content: system_text.clone(),
            });
            all_text.push_str(&system_text);
            all_text.push('\n');
        }
    }

    // Handle contents array
    if let Some(Value::Array(contents)) = body.get("contents") {
        for content in contents {
            let role = content
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("user")
                .to_string();

            // Gemini uses "parts" array
            let text = if let Some(Value::Array(parts)) = content.get("parts") {
                parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                extract_text_from_value(content)
            };

            if !text.is_empty() {
                all_text.push_str(&text);
                all_text.push('\n');
            }

            messages.push(Message {
                role,
                content: text,
            });
        }
    }

    Ok((model, messages, all_text))
}

/// Recursively extract all text from a JSON value
pub fn extract_text_from_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .map(extract_text_from_value)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(obj) => {
            // Prioritize "text" and "content" fields
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                return text.to_string();
            }
            if let Some(content) = obj.get("content") {
                return extract_text_from_value(content);
            }
            // Fall back to extracting from all values
            obj.values()
                .map(extract_text_from_value)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => String::new(),
    }
}

/// Detect provider from request path
pub fn detect_provider(path: &str, providers: &std::collections::HashMap<String, crate::config::ProviderConfig>) -> Option<String> {
    for (name, config) in providers {
        if path.contains(&config.path_pattern) {
            return Some(name.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens() {
        let count = count_tokens("Hello, world!");
        assert!(count > 0);
    }

    #[test]
    fn test_extract_text_from_value() {
        let value = serde_json::json!({
            "content": [
                {"type": "text", "text": "Hello"},
                {"type": "text", "text": "World"}
            ]
        });
        let text = extract_text_from_value(&value);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_parse_anthropic_request() {
        let body = serde_json::json!({
            "model": "claude-3-5-sonnet-20250514",
            "system": "You are a helpful assistant.",
            "messages": [
                {"role": "user", "content": "Hello!"}
            ]
        });

        let (model, messages, _) = parse_anthropic_request(&body).unwrap();
        assert_eq!(model, "claude-3-5-sonnet-20250514");
        assert_eq!(messages.len(), 2); // system + user
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
    }

    #[test]
    fn test_parse_openai_request() {
        let body = serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Hello!"}
            ]
        });

        let (model, messages, _) = parse_openai_request(&body).unwrap();
        assert_eq!(model, "gpt-4");
        assert_eq!(messages.len(), 2);
    }
}
