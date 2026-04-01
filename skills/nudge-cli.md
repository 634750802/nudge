# Nudge -- Event Subscription Skill

You have access to `nudge`, a CLI tool that lets you subscribe to external events (GitHub activity, timers) and block until they happen.

## When to use

- You need to wait for a PR to be merged, CI to pass, or an issue to be closed
- You need to pause work and resume after a time delay
- You want to run a command when an event fires

## Commands

### `nudge wait` -- block until condition is met

```bash
nudge wait <source> <args...> [--timeout <duration>] [--repo owner/repo]
```

Blocks your current process until the condition fires. The event payload is printed as JSON to stdout.

```bash
nudge wait github pr 42 merged
nudge wait github ci 123 success
nudge wait github issue 50 new-comment
nudge wait timer 30m
nudge wait github pr 42 merged --timeout 2h
```

### `nudge on` -- register callback, exit immediately

```bash
nudge on <source> <args...> --run "<command>" [--timeout <duration>] [--repo owner/repo]
```

Registers a subscription that executes `--run` when the event fires. Returns immediately.

```bash
nudge on github pr 42 merged --run "echo 'PR merged!'"
nudge on timer 6h --run "claude -p 'Grace period expired. Check status.'"
```

### `nudge list` / `nudge cancel` / `nudge status`

```bash
nudge list                        # List all subscriptions
nudge list --source github        # Filter by source
nudge list --status active        # Filter by status
nudge cancel <id>                 # Cancel a subscription
nudge status                      # Show daemon status and counts
```

## Supported events

### GitHub

Requires `gh` CLI to be authenticated. `--repo owner/repo` is optional if you're in a git repo with a GitHub remote.

| Command | Fires when |
|---------|-----------|
| `github pr <N> merged` | PR is merged |
| `github pr <N> closed` | PR is closed |
| `github pr <N> ci-passed` | All CI checks pass on PR |
| `github pr <N> new-comment` | A new comment is added |
| `github pr <N> label:<name>` | Label is added to PR |
| `github issue <N> closed` | Issue is closed |
| `github issue <N> new-comment` | A new comment is added |
| `github issue <N> label:<name>` | Label is added to issue |
| `github ci <N> success` | CI run succeeds |
| `github ci <N> failure` | CI run fails |
| `github ci <N> completed` | CI run completes (any outcome) |

### Timer

| Command | Fires when |
|---------|-----------|
| `timer <duration>` | Duration elapses (e.g., `30s`, `5m`, `2h`, `7d`) |

## Patterns

### Wait for CI then act

```bash
nudge wait github ci 123 success --timeout 1h
# CI passed -- proceed with deployment
```

### Run command on event

```bash
nudge on github pr 42 merged --run "./deploy.sh staging"
```

### Delay before retry

```bash
nudge wait timer 5m
# 5 minutes later, retry the operation
```
