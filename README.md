# repo-syncer

> **Keep your GitHub forks fresh.** A beautiful terminal UI that batch syncs your locally cloned forks with upstream — because life's too short for `git fetch upstream && git rebase` on 50 repos.

## The Problem

You've forked dozens of repos over the years. They're scattered across your machine. Upstream moves on. Your forks fall behind. Syncing them manually? Tedious.

## The Solution

**repo-syncer** gives you a single command to:

- Find all your locally cloned forks
- Sync them with upstream (stashing your work, handling branches, restoring state)
- Skip gracefully when there are conflicts or unpushed commits

All in a slick TUI with vim keybindings.

## Installation

```bash
# Clone and build
git clone https://github.com/kevinmichaelchen/repo-syncer.git
cd repo-syncer
cargo install --path .
```

**Requirements:** [GitHub CLI](https://cli.github.com/) (`gh`) must be installed and authenticated.

## Usage

```bash
# Interactive mode — select which forks to sync
repo-syncer

# Sync all locally cloned forks, no questions asked
repo-syncer --yes

# See what would happen without making changes
repo-syncer --dry-run

# Include forks you haven't cloned yet (will clone them)
repo-syncer --clone-missing

# List forks you haven't cloned (useful for discovery)
repo-syncer --list-uncloned

# Custom directory for cloned repos (default: ~/dev)
repo-syncer --tool-home ~/projects
```

## Keybindings

| Key       | Action           |
| --------- | ---------------- |
| `j` / `k` | Navigate up/down |
| `Space`   | Toggle selection |
| `a`       | Select all       |
| `Enter`   | Start sync       |
| `r`       | Reset (in Done)  |
| `q`       | Quit             |

## How It Works

For each fork, repo-syncer:

1. **Stashes** uncommitted changes
2. **Syncs** with upstream via `gh repo sync`
3. **Pulls** the latest changes
4. **Restores** your original branch and stash

If there are unpushed commits that would conflict, it skips that repo and moves on — no data loss, no drama.

## Configuration

| Flag              | Env Var     | Default | Description                             |
| ----------------- | ----------- | ------- | --------------------------------------- |
| `--tool-home`     | `TOOL_HOME` | `~/dev` | Where repos are cloned                  |
| `--dry-run`       |             | `false` | Preview without changes                 |
| `--yes` `-y`      |             | `false` | Skip confirmation, sync all             |
| `--clone-missing` |             | `false` | Include and clone forks not yet on disk |
| `--list-uncloned` |             | `false` | List uncloned forks and exit            |

## License

MIT
