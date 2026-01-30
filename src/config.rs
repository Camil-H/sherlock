use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub proxy: ProxyConfig,
    pub dashboard: DashboardConfig,
    pub providers: HashMap<String, ProviderConfig>,
    pub archive: ArchiveConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub port: u16,
    pub bind_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub token_limit: u64,
    pub max_log_entries: usize,
    pub refresh_rate_hz: u32,
    pub prompt_preview_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub host: String,
    pub base_url: String,
    pub env_vars: Vec<String>,
    pub path_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub format: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut providers = HashMap::new();

        providers.insert(
            "anthropic".to_string(),
            ProviderConfig {
                host: "api.anthropic.com".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                env_vars: vec!["ANTHROPIC_BASE_URL".to_string()],
                path_pattern: "/v1/messages".to_string(),
            },
        );

        providers.insert(
            "openai".to_string(),
            ProviderConfig {
                host: "api.openai.com".to_string(),
                base_url: "https://api.openai.com".to_string(),
                env_vars: vec!["OPENAI_BASE_URL".to_string()],
                path_pattern: "/v1/chat/completions".to_string(),
            },
        );

        providers.insert(
            "gemini".to_string(),
            ProviderConfig {
                host: "generativelanguage.googleapis.com".to_string(),
                base_url: "https://generativelanguage.googleapis.com".to_string(),
                env_vars: vec![
                    "GOOGLE_GEMINI_BASE_URL".to_string(),
                    "GEMINI_API_BASE_URL".to_string(),
                    "GEMINI_BASEURL".to_string(),
                ],
                path_pattern: "generateContent".to_string(),
            },
        );

        Self {
            proxy: ProxyConfig {
                port: 8080,
                bind_address: "127.0.0.1".to_string(),
            },
            dashboard: DashboardConfig {
                token_limit: 200_000,
                max_log_entries: 100,
                refresh_rate_hz: 4,
                prompt_preview_length: 200,
            },
            providers,
            archive: ArchiveConfig {
                enabled: true,
                directory: PathBuf::from("~/.sherlock/prompts"),
                format: vec!["markdown".to_string(), "json".to_string()],
            },
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let expanded_path = expand_tilde(path);

        if expanded_path.exists() {
            let content = std::fs::read_to_string(&expanded_path)?;
            let mut config: Config = serde_json::from_str(&content)?;
            // Expand tilde in archive directory
            config.archive.directory = expand_tilde(&config.archive.directory);
            Ok(config)
        } else {
            tracing::info!(
                "Config file not found at {:?}, using defaults",
                expanded_path
            );
            let mut config = Config::default();
            config.archive.directory = expand_tilde(&config.archive.directory);
            Ok(config)
        }
    }

    pub fn with_overrides(mut self, port: Option<u16>, limit: Option<u64>) -> Self {
        if let Some(p) = port {
            self.proxy.port = p;
        }
        if let Some(l) = limit {
            self.dashboard.token_limit = l;
        }
        self
    }

    /// Save the default config to the specified path
    pub fn save_default(path: &Path) -> Result<()> {
        let expanded_path = expand_tilde(path);

        if let Some(parent) = expanded_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config = Config::default();
        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&expanded_path, content)?;

        tracing::info!("Saved default config to {:?}", expanded_path);
        Ok(())
    }
}

/// Expand ~ to home directory
pub fn expand_tilde(path: &Path) -> PathBuf {
    if let Some(path_str) = path.to_str() {
        if path_str.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&path_str[2..]);
            }
        } else if path_str == "~" {
            if let Some(home) = dirs::home_dir() {
                return home;
            }
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.proxy.port, 8080);
        assert_eq!(config.dashboard.token_limit, 200_000);
        assert!(config.providers.contains_key("anthropic"));
        assert!(config.providers.contains_key("openai"));
        assert!(config.providers.contains_key("gemini"));
    }

    #[test]
    fn test_with_overrides() {
        let config = Config::default().with_overrides(Some(9090), Some(100_000));
        assert_eq!(config.proxy.port, 9090);
        assert_eq!(config.dashboard.token_limit, 100_000);
    }
}
