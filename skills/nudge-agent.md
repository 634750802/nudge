# Nudge -- Event Subscription Skill (Agent Mode)

You are running inside a `nudge agent` session. You can subscribe to external events and **end your turn** -- a new turn will start automatically when the event fires, with your memo as context.

## How it works

1. Run `nudge wait ... --detach --memo "..."` to register a subscription
2. End your current turn immediately
3. When the event fires, nudge starts a new turn with the event details and your memo

You do NOT block or poll. The `--detach` flag returns immediately and the agent loop handles the rest.

## Command

```bash
nudge wait <source> <args...> --detach --memo "<what to do when event fires>" [--timeout <duration>] [--repo owner/repo]
```

- `--detach`: Required. Register and return immediately.
- `--memo`: Strongly recommended. A note to your future self about what to do when the event fires. This will appear in your next turn's prompt.
- `--timeout`: Optional. Auto-cancel if the event doesn't fire within this duration.
- `--repo`: Optional if you're in a git repo with a GitHub remote.

## Examples

```bash
# Watch for PR merge, then deploy
nudge wait github pr 42 merged --detach --memo "PR 42 merged. Deploy to staging and run smoke tests."

# Watch for CI to pass
nudge wait github ci 123 success --detach --memo "CI passed on PR 123. Post approval review comment."

# Watch for new comments on an issue
nudge wait github issue 50 new-comment --detach --memo "New comment on issue 50. Read and respond."

# Set a timer reminder
nudge wait timer 2h --detach --memo "2 hours elapsed. Check deployment metrics."

# With timeout
nudge wait github pr 42 merged --detach --memo "PR merged. Continue release." --timeout 4h
```

## Monitoring multiple events

You can register multiple subscriptions in one turn. Each fires independently and starts a separate turn.

```bash
nudge wait github pr 10 merged --detach --memo "PR 10 merged. Start integration tests."
nudge wait github issue 20 closed --detach --memo "Issue 20 resolved. Update tracking doc."
```

After registering, **end your turn**.

## Checking subscriptions

```bash
nudge list                        # List all subscriptions
nudge list --status active        # Show only active ones
nudge cancel <id>                 # Cancel a subscription
```

## Supported events

### GitHub

| Event | Usage |
|-------|-------|
| PR merged | `github pr <N> merged` |
| PR closed | `github pr <N> closed` |
| PR CI passed | `github pr <N> ci-passed` |
| PR new comment | `github pr <N> new-comment` |
| PR label added | `github pr <N> label:<name>` |
| Issue closed | `github issue <N> closed` |
| Issue new comment | `github issue <N> new-comment` |
| Issue label added | `github issue <N> label:<name>` |
| CI success | `github ci <N> success` |
| CI failure | `github ci <N> failure` |
| CI completed | `github ci <N> completed` |

### Timer

| Event | Usage |
|-------|-------|
| Duration elapsed | `timer <duration>` (e.g., `30s`, `5m`, `2h`, `7d`) |

## Important

- Always use `--detach`. Never use plain `nudge wait` (blocking) inside an agent session.
- Always include `--memo` so your next turn has context.
- End your turn after registering subscriptions. Do not continue working.
