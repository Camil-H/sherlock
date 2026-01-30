use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sherlock", about = "LLM traffic inspector and token usage tracker")]
#[command(version, author)]
pub struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "~/.sherlock/config.json")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the proxy server and dashboard
    Start {
        /// Override proxy port
        #[arg(short, long)]
        port: Option<u16>,

        /// Override token limit for fuel gauge
        #[arg(short, long)]
        limit: Option<u64>,
    },

    /// Run Claude Code through the proxy
    Claude {
        /// Arguments to pass to claude
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run Happy (Claude frontend) through the proxy
    Happy {
        /// Arguments to pass to happy
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run Gemini CLI through the proxy
    Gemini {
        /// Arguments to pass to gemini
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run OpenAI Codex through the proxy
    Codex {
        /// Arguments to pass to codex
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run any command with a specified provider
    Run {
        /// Provider name (anthropic, openai, gemini)
        #[arg(short = 'P', long)]
        provider: String,

        /// Command and arguments to run
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
}
