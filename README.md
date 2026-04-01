# Nudge

Subscription system for AI agents -- wait for events, get nudged when they happen.

```bash
nudge wait github pr 42 merged            # block until PR 42 is merged
nudge wait github ci 2199 success         # block until CI passes
nudge wait timer 30m                      # block for 30 minutes
nudge wait github issue 2208 new-comment  # block until new comment appears
```

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/634750802/nudge/main/install.sh | sh
```

Or install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/634750802/nudge/main/install.sh | sh -s v0.1.1
```

Custom install directory:

```bash
curl -fsSL https://raw.githubusercontent.com/634750802/nudge/main/install.sh | NUDGE_INSTALL_DIR=~/.local/bin sh
```

## Usage

### `nudge wait` -- block until condition is met

The agent blocks and the event is printed as JSON to stdout when the condition fires.

```bash
# Wait for a PR to be merged
nudge wait github pr 42 merged --repo owner/repo

# Wait for CI to pass
nudge wait github ci 2199 success --repo owner/repo

# Wait for a new comment on an issue
nudge wait github issue 2208 new-comment --repo owner/repo

# Wait for a duration
nudge wait timer 30m

# With timeout
nudge wait github pr 42 merged --repo owner/repo --timeout 2h
```

### `nudge on` -- register callback, exit immediately

Registers a persistent subscription. When the condition fires, nudge executes the `--run` command.

```bash
# Run a command when PR is merged
nudge on github pr 42 merged --repo owner/repo --run "claude -p 'PR 42 merged. Deploy to staging.'"

# Run a command when CI passes
nudge on github ci 2199 success --repo owner/repo --run "claude -p 'CI passed. Run gate review.'"

# Run a command after a delay
nudge on timer 6h --run "claude -p 'Grace period expired.'"
```

### `nudge list` -- show subscriptions

```bash
nudge list
nudge list --source github
nudge list --status active
```

### `nudge cancel` -- cancel a subscription

```bash
nudge cancel <subscription-id>
```

### `nudge status` -- show daemon status

```bash
nudge status
```

## How it works

Nudge runs a background daemon that manages subscriptions and polls for conditions. The daemon auto-starts when you run `nudge wait` or `nudge on`.

- **GitHub sources** are checked via `gh` CLI (no token management needed)
- **Timers** use the internal clock
- **Webhooks** are available via `--enable-webhooks` on the daemon

Subscriptions are stored in SQLite at `~/.nudge/subscriptions.db` and survive process restarts.

## Agent Skills

Nudge ships with skill files that teach AI agents how to use it. Add them to your agent's context (e.g., in `CLAUDE.md` or system prompt).

| Skill file | Use case |
|------------|----------|
| [`skills/nudge-cli.md`](skills/nudge-cli.md) | Direct use -- blocking `wait`, `on` callbacks, `list`/`cancel` |
| [`skills/nudge-agent.md`](skills/nudge-agent.md) | Inside `nudge agent` -- non-blocking `--detach` with `--memo` |

### Claude Code

Add to your project's `CLAUDE.md`:

```markdown
<nudge-skill>
{{paste contents of skills/nudge-cli.md}}
</nudge-skill>
```

Or for agents managed by `nudge agent`:

```markdown
<nudge-skill>
{{paste contents of skills/nudge-agent.md}}
</nudge-skill>
```

### `nudge agent` with idle prompt

When running `nudge agent`, pass the agent skill file so Claude knows how to use nudge:

```bash
nudge agent --idle-every 4h --idle-prompt-file idle.md -- --model sonnet --permission-mode bypassPermissions --append-system-prompt-file skills/nudge-agent.md
```

## Build from source

```bash
git clone https://github.com/634750802/nudge.git
cd nudge
cargo build --release
```

## License

MIT
