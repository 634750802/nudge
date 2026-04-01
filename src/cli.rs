use std::path::PathBuf;
use clap::{Parser, Subcommand, Args};

#[derive(Parser)]
#[command(name = "nudge", about = "Subscription system for AI agents")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Block until condition is met, print event JSON to stdout
    Wait(WaitArgs),
    /// Register persistent subscription with callback, exit immediately
    On(OnArgs),
    /// List subscriptions
    List(ListArgs),
    /// Cancel a subscription
    Cancel {
        /// Subscription ID
        id: String,
    },
    /// Show daemon status
    Status,
    /// Start the daemon (usually auto-started)
    Daemon(DaemonArgs),
    /// Run as a long-lived agent with idle-turn scheduling
    Agent(AgentArgs),
}

#[derive(Args)]
pub struct WaitArgs {
    /// Source: github, timer, webhook
    pub source: String,
    /// Source-specific arguments (e.g., "pr 42 merged", "30m")
    pub args: Vec<String>,
    /// Timeout duration (e.g., "1h", "30m")
    #[arg(long)]
    pub timeout: Option<String>,
    /// GitHub repo (owner/repo)
    #[arg(long)]
    pub repo: Option<String>,
    /// Register subscription and return immediately (don't block)
    #[arg(long)]
    pub detach: bool,
    /// Note for what to do when event fires
    #[arg(long)]
    pub memo: Option<String>,
    /// Agent ID (set automatically via NUDGE_AGENT_ID env var)
    #[arg(long, env = "NUDGE_AGENT_ID", hide = true)]
    pub agent_id: Option<String>,
}

#[derive(Args)]
pub struct OnArgs {
    /// Source: github, timer, webhook
    pub source: String,
    /// Source-specific arguments
    pub args: Vec<String>,
    /// Command to execute when event fires
    #[arg(long)]
    pub run: String,
    /// Timeout duration
    #[arg(long)]
    pub timeout: Option<String>,
    /// GitHub repo (owner/repo)
    #[arg(long)]
    pub repo: Option<String>,
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by source
    #[arg(long)]
    pub source: Option<String>,
    /// Filter by status
    #[arg(long)]
    pub status: Option<String>,
}

#[derive(Args)]
pub struct DaemonArgs {
    /// Enable webhook HTTP listener
    #[arg(long)]
    pub enable_webhooks: bool,
    /// Webhook listen port
    #[arg(long, default_value = "9876")]
    pub webhook_port: u16,
}

#[derive(Args)]
pub struct AgentArgs {
    /// Interval for idle-turn checks (e.g., "4h", "30m")
    #[arg(long)]
    pub idle_every: Option<String>,
    /// File containing the idle-turn prompt
    #[arg(long)]
    pub idle_prompt_file: Option<PathBuf>,
    /// Maximum duration for a single turn (default: 600s)
    #[arg(long, default_value = "600s")]
    pub turn_timeout: Option<String>,
    /// Extra flags passed to claude CLI (after --)
    #[arg(last = true)]
    pub claude_args: Vec<String>,
}
