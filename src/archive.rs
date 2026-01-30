use anyhow::Result;
use tokio::fs;
use tokio::sync::mpsc;

use crate::config::ArchiveConfig;
use crate::event::RequestEvent;

/// Async task that writes prompts to disk
pub async fn archive_writer(
    mut rx: mpsc::Receiver<RequestEvent>,
    config: ArchiveConfig,
) -> Result<()> {
    if !config.enabled {
        tracing::info!("Prompt archiving disabled");
        // Drain the channel without doing anything
        while rx.recv().await.is_some() {}
        return Ok(());
    }

    // Ensure directory exists
    fs::create_dir_all(&config.directory).await?;

    tracing::info!("Archiving prompts to {:?}", config.directory);

    while let Some(event) = rx.recv().await {
        if let Err(e) = save_prompt(&event, &config).await {
            tracing::error!("Failed to save prompt: {}", e);
        }
    }

    Ok(())
}

async fn save_prompt(event: &RequestEvent, config: &ArchiveConfig) -> Result<()> {
    let timestamp = event.timestamp.format("%Y%m%d_%H%M%S%.3f");
    let base_name = format!("{}_{}", timestamp, event.provider);

    for format in &config.format {
        let path = match format.as_str() {
            "markdown" | "md" => {
                let path = config.directory.join(format!("{}.md", base_name));
                let content = format_markdown(event);
                fs::write(&path, content).await?;
                path
            }
            "json" => {
                let path = config.directory.join(format!("{}.json", base_name));
                let content = serde_json::to_string_pretty(&event.raw_body)?;
                fs::write(&path, content).await?;
                path
            }
            _ => {
                tracing::warn!("Unknown archive format: {}", format);
                continue;
            }
        };

        tracing::debug!("Saved prompt to {:?}", path);
    }

    Ok(())
}

fn format_markdown(event: &RequestEvent) -> String {
    let mut md = String::new();

    // Header
    md.push_str(&format!("# {} Request\n\n", capitalize(&event.provider)));
    md.push_str(&format!("- **Timestamp:** {}\n", event.timestamp));
    md.push_str(&format!("- **Model:** {}\n", event.model));
    md.push_str(&format!("- **Tokens:** {}\n", event.tokens));
    md.push_str(&format!("- **Path:** {}\n\n", event.path));

    // Messages
    md.push_str("## Messages\n\n");

    for msg in &event.messages {
        md.push_str(&format!("### {}\n\n", capitalize(&msg.role)));
        md.push_str(&msg.content);
        md.push_str("\n\n");
    }

    md
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_format_markdown() {
        let event = RequestEvent {
            timestamp: Utc::now(),
            provider: "anthropic".to_string(),
            model: "claude-3".to_string(),
            tokens: 100,
            messages: vec![
                crate::event::Message {
                    role: "user".to_string(),
                    content: "Hello!".to_string(),
                },
            ],
            raw_body: serde_json::json!({}),
            path: "/v1/messages".to_string(),
        };

        let md = format_markdown(&event);
        assert!(md.contains("# Anthropic Request"));
        assert!(md.contains("**Model:** claude-3"));
        assert!(md.contains("### User"));
        assert!(md.contains("Hello!"));
    }
}
