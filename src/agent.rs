use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use crate::cli::AgentArgs;
use crate::store::Store;
use crate::subscription::Subscription;

/// Manages state for running Claude Code CLI turns via `--resume`.
struct AgentRunner {
    /// Claude Code session ID, captured from the first turn's init message.
    session_id: Option<String>,
    /// Unique identifier for this agent instance (short UUID).
    agent_id: String,
    /// If set, run an idle turn after this duration of inactivity.
    idle_every: Option<Duration>,
    /// Prompt content to use for idle turns.
    idle_prompt: Option<String>,
    /// Maximum wall-clock time for a single Claude Code turn.
    turn_timeout: Duration,
    /// Timestamp of the last completed turn.
    last_turn_at: Option<Instant>,
    /// Tracks consecutive turns that completed within 10s (for backoff).
    consecutive_fast_turns: u32,
}

/// Run a single Claude Code CLI turn with the given prompt.
///
/// Builds and spawns: `claude -p --output-format stream-json [--resume <id>] "<prompt>"`
/// Parses NDJSON stdout to capture the session ID on init and detect turn completion.
/// Applies `turn_timeout` — kills the child process if exceeded.
async fn run_claude_turn(runner: &mut AgentRunner, prompt: &str) -> Result<()> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg("--output-format")
        .arg("stream-json");

    if let Some(ref session_id) = runner.session_id {
        cmd.arg("--resume").arg(session_id);
    }

    cmd.arg(prompt);

    cmd.env("NUDGE_AGENT_ID", &runner.agent_id);

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .stdin(std::process::Stdio::null());

    tracing::info!(
        agent_id = %runner.agent_id,
        has_session = runner.session_id.is_some(),
        "Starting Claude Code turn"
    );

    let mut child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!("Failed to spawn claude CLI: {e}. Is `claude` on PATH?")
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture claude stdout"))?;

    let reader = tokio::io::BufReader::new(stdout);
    let mut lines = reader.lines();

    let process_lines = async {
        while let Some(line) = lines.next_line().await? {
            // Try to parse as JSON to extract session_id and detect completion
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                // Check for init message: {"type":"system","subtype":"init","session_id":"..."}
                if value.get("type").and_then(|v| v.as_str()) == Some("system")
                    && value.get("subtype").and_then(|v| v.as_str()) == Some("init")
                {
                    if let Some(sid) = value.get("session_id").and_then(|v| v.as_str()) {
                        tracing::info!(session_id = %sid, "Captured session ID");
                        runner.session_id = Some(sid.to_string());
                    }
                    continue;
                }

                // Check for result message: {"type":"result",...}
                if value.get("type").and_then(|v| v.as_str()) == Some("result") {
                    tracing::debug!("Turn completed (result message received)");
                    break;
                }
            }

            // Log all other lines for debugging
            eprintln!("[claude] {line}");
        }
        Ok::<(), anyhow::Error>(())
    };

    // Apply turn timeout
    match tokio::time::timeout(runner.turn_timeout, process_lines).await {
        Ok(result) => result?,
        Err(_) => {
            tracing::warn!(
                timeout_secs = runner.turn_timeout.as_secs(),
                "Turn timed out, killing child process"
            );
            let _ = child.kill().await;
            anyhow::bail!(
                "Claude Code turn timed out after {}s",
                runner.turn_timeout.as_secs()
            );
        }
    }

    // Wait for the child process to exit
    let _ = child.wait().await;

    runner.last_turn_at = Some(Instant::now());

    tracing::info!(agent_id = %runner.agent_id, "Turn finished");
    Ok(())
}

/// Format fired subscriptions into a prompt for Claude Code.
///
/// Produces a numbered list like:
/// ```text
/// Events received:
/// 1. foo/bar pr #42 merged -- Memo: Check if tests pass
/// 2. timer 5m
/// ```
fn format_events_with_memos(subs: &[Subscription]) -> String {
    let mut lines = vec!["Events received:".to_string()];
    for (i, sub) in subs.iter().enumerate() {
        let summary = sub.condition_summary();
        let line = if let Some(ref memo) = sub.memo {
            format!("{}. {} -- Memo: {}", i + 1, summary, memo)
        } else {
            format!("{}. {}", i + 1, summary)
        };
        lines.push(line);
    }
    lines.join("\n")
}

/// Check whether enough idle time has elapsed to trigger an idle turn.
fn should_idle(runner: &AgentRunner) -> bool {
    let idle_every = match runner.idle_every {
        Some(d) => d,
        None => return false,
    };

    // No idle prompt configured means we can't run idle turns
    if runner.idle_prompt.is_none() {
        return false;
    }

    match runner.last_turn_at {
        Some(last) => last.elapsed() >= idle_every,
        // Never ran a turn yet -- idle after the interval from start
        None => true,
    }
}

/// Per-session rate limiter.
///
/// Consecutive turns completing within 10s trigger exponential backoff:
/// 10s -> 20s -> 40s -> 80s -> 160s -> 300s (capped).
async fn rate_limit_check(runner: &mut AgentRunner) {
    if let Some(last) = runner.last_turn_at {
        let elapsed = last.elapsed();
        if elapsed < Duration::from_secs(10) {
            runner.consecutive_fast_turns += 1;
            let backoff = Duration::from_secs(
                (10 * 2u64.pow(runner.consecutive_fast_turns.min(5))).min(300),
            );
            tracing::warn!(
                backoff_secs = backoff.as_secs(),
                consecutive = runner.consecutive_fast_turns,
                "Rate limiting: backing off"
            );
            tokio::time::sleep(backoff).await;
        } else {
            runner.consecutive_fast_turns = 0;
        }
    }
}

/// Parse a duration string (e.g., "30s", "5m", "2h") into a `std::time::Duration`.
fn parse_std_duration(s: &str) -> Result<Duration> {
    let secs = crate::subscription::parse_duration_secs(s)?;
    Ok(Duration::from_secs(secs as u64))
}

/// Entry point for the `nudge agent` command.
///
/// Runs an event-driven agent loop that:
/// 1. Polls the subscription store for fired events targeting this agent
/// 2. Formats events (with optional memos) into prompts
/// 3. Sends prompts to Claude Code CLI, maintaining session context via `--resume`
/// 4. Optionally runs idle turns when no events have arrived for a configurable period
pub async fn run(args: AgentArgs) -> Result<()> {
    // 1. Generate a short agent ID
    let agent_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

    // 2. Parse --idle-every duration
    let idle_every = args
        .idle_every
        .as_deref()
        .map(parse_std_duration)
        .transpose()?;

    // 3. Read --idle-prompt-file content
    let idle_prompt = match args.idle_prompt_file {
        Some(ref path) => {
            let content = std::fs::read_to_string(path).map_err(|e| {
                anyhow::anyhow!("Failed to read idle prompt file {}: {e}", path.display())
            })?;
            Some(content)
        }
        None => None,
    };

    // 4. Parse --turn-timeout (default is "600s" from CLI)
    let turn_timeout = parse_std_duration(args.turn_timeout.as_deref().unwrap_or("600s"))?;

    // 5. Ensure the daemon is running (it processes subscriptions)
    crate::daemon::ensure_running()?;

    tracing::info!(
        agent_id = %agent_id,
        idle_every_secs = idle_every.map(|d| d.as_secs()),
        turn_timeout_secs = turn_timeout.as_secs(),
        "Agent starting"
    );

    let mut runner = AgentRunner {
        session_id: None,
        agent_id: agent_id.clone(),
        idle_every,
        idle_prompt,
        turn_timeout,
        last_turn_at: None,
        consecutive_fast_turns: 0,
    };

    // Main loop: poll for events, run Claude turns, handle idle
    loop {
        let db = Store::open_default()?;

        // Check for fired events targeting this agent
        let events = db.list_fired_for_agent(&agent_id)?;

        if !events.is_empty() {
            // Mark events as dispatched so they aren't re-processed
            let ids: Vec<String> = events.iter().map(|e| e.id.clone()).collect();
            db.mark_dispatched(&ids)?;

            let prompt = format_events_with_memos(&events);
            drop(db);

            tracing::info!(
                event_count = events.len(),
                "Dispatching {} event(s) to Claude",
                events.len()
            );

            rate_limit_check(&mut runner).await;
            run_claude_turn(&mut runner, &prompt).await?;
        } else if should_idle(&runner) {
            // Run an idle turn
            if let Some(ref prompt) = runner.idle_prompt {
                let prompt = prompt.clone();
                drop(db);

                tracing::info!("Running idle turn");

                rate_limit_check(&mut runner).await;
                run_claude_turn(&mut runner, &prompt).await?;
            } else {
                drop(db);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        } else {
            drop(db);
            // No events and not idle-due -- sleep before next poll
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscription::{Condition, Subscription};

    fn make_sub(id: &str, memo: Option<&str>) -> Subscription {
        Subscription {
            id: id.into(),
            source: "timer".into(),
            condition: Condition::Timer {
                duration: "5m".into(),
                fire_at: 0,
            },
            mode: "wait".into(),
            callback: None,
            status: "fired".into(),
            created_at: 0,
            expires_at: None,
            event_data: None,
            memo: memo.map(|s| s.to_string()),
            agent_id: None,
        }
    }

    #[test]
    fn test_format_events_single_no_memo() {
        let subs = vec![make_sub("a", None)];
        let result = format_events_with_memos(&subs);
        assert_eq!(result, "Events received:\n1. timer 5m");
    }

    #[test]
    fn test_format_events_single_with_memo() {
        let subs = vec![make_sub("a", Some("Check CI status"))];
        let result = format_events_with_memos(&subs);
        assert_eq!(
            result,
            "Events received:\n1. timer 5m -- Memo: Check CI status"
        );
    }

    #[test]
    fn test_format_events_multiple_mixed() {
        let subs = vec![
            make_sub("a", Some("First memo")),
            make_sub("b", None),
            make_sub("c", Some("Third memo")),
        ];
        let result = format_events_with_memos(&subs);
        let expected = "Events received:\n\
                        1. timer 5m -- Memo: First memo\n\
                        2. timer 5m\n\
                        3. timer 5m -- Memo: Third memo";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_should_idle_no_config() {
        let runner = AgentRunner {
            session_id: None,
            agent_id: "test".into(),
            idle_every: None,
            idle_prompt: None,
            turn_timeout: Duration::from_secs(600),
            last_turn_at: None,
            consecutive_fast_turns: 0,
        };
        assert!(!should_idle(&runner));
    }

    #[test]
    fn test_should_idle_no_prompt() {
        let runner = AgentRunner {
            session_id: None,
            agent_id: "test".into(),
            idle_every: Some(Duration::from_secs(60)),
            idle_prompt: None,
            turn_timeout: Duration::from_secs(600),
            last_turn_at: None,
            consecutive_fast_turns: 0,
        };
        assert!(!should_idle(&runner));
    }

    #[test]
    fn test_should_idle_never_ran() {
        let runner = AgentRunner {
            session_id: None,
            agent_id: "test".into(),
            idle_every: Some(Duration::from_secs(60)),
            idle_prompt: Some("check in".into()),
            turn_timeout: Duration::from_secs(600),
            last_turn_at: None,
            consecutive_fast_turns: 0,
        };
        // First idle should fire immediately since no turn has ever run
        assert!(should_idle(&runner));
    }

    #[test]
    fn test_should_idle_recently_ran() {
        let runner = AgentRunner {
            session_id: None,
            agent_id: "test".into(),
            idle_every: Some(Duration::from_secs(300)),
            idle_prompt: Some("check in".into()),
            turn_timeout: Duration::from_secs(600),
            last_turn_at: Some(Instant::now()),
            consecutive_fast_turns: 0,
        };
        // Just ran, should not idle yet
        assert!(!should_idle(&runner));
    }

    #[test]
    fn test_parse_std_duration() {
        assert_eq!(parse_std_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_std_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(
            parse_std_duration("2h").unwrap(),
            Duration::from_secs(7200)
        );
    }
}
