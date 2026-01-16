# repo-syncer

> **Keep your GitHub forks fresh.** A beautiful terminal UI that batch syncs your locally cloned forks with upstream — because life's too short for `git fetch upstream && git rebase` on 50 repos.

## The Problem

You've forked dozens of repos over the years. They're scattered across your machine. Upstream moves on. Your forks fall behind. Syncing them manually? Tedious.

## The Solution

**repo-syncer** gives you a unified TUI to:

- View all your forks (cloned and uncloned) in one place
- See fork details: description, language, branch, clone status
- Sync cloned forks with upstream (stashing your work, handling branches, restoring state)
- Clone uncloned forks directly from the TUI
- Open forks in your browser or editor
- Archive forks you no longer need
- Search/filter forks by name
- View language statistics

All in a slick two-pane TUI with vim keybindings.

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
# Interactive mode — unified TUI with all forks
repo-syncer

# Sync all cloned forks, no questions asked
repo-syncer --yes

# See what would happen without making changes
repo-syncer --dry-run

# Custom directory for cloned repos (default: ~/dev)
repo-syncer --tool-home ~/projects
```

## Keybindings

### Navigation

| Key       | Action           |
| --------- | ---------------- |
| `j` / `k` | Navigate up/down |
| `Space`   | Toggle selection |
| `a`       | Select all       |
| `/`       | Search/filter    |

### Actions

| Key     | Action                           |
| ------- | -------------------------------- |
| `Enter` | Sync selected forks              |
| `c`     | Clone current fork (if uncloned) |
| `o`     | Open in browser                  |
| `e`     | Open in editor ($EDITOR)         |
| `x`     | Archive fork (with confirmation) |
| `d`     | Toggle stats dashboard           |
| `R`     | Refresh from GitHub              |

### General

| Key   | Action                 |
| ----- | ---------------------- |
| `q`   | Quit                   |
| `Esc` | Cancel / Close overlay |
| `r`   | Reset (in Done mode)   |

## How It Works

For each fork, repo-syncer:

1. **Stashes** uncommitted changes
2. **Syncs** with upstream via `gh repo sync`
3. **Pulls** the latest changes
4. **Restores** your original branch and stash

If there are unpushed commits that would conflict, it skips that repo and moves on — no data loss, no drama.

## Features

### Two-Pane Layout

On wide terminals (100+ chars), you get a details pane showing:

- Fork name and parent repository
- Description
- Primary language
- Default branch
- Clone status and local path

### Fuzzy Search

Press `/` to enter search mode. Type to filter forks by name. Results are sorted by match quality.

### Stats Dashboard

Press `d` to see a statistics overlay showing:

- Total, cloned, and uncloned fork counts
- Language distribution bar chart

### Direct Actions

- **Clone**: Press `c` on any uncloned fork to clone it immediately
- **Open in Browser**: Press `o` to open the fork on GitHub
- **Open in Editor**: Press `e` to open cloned forks in your `$EDITOR`
- **Archive**: Press `x` to archive forks you no longer need

All actions are non-blocking and run asynchronously in the background.

### SQLite Caching

Fork metadata is cached locally at `~/.cache/repo-syncer/forks.db` for:

- **Instant startup** - No waiting for GitHub API on every launch
- **Offline mode** - Browse and manage forks without network access
- **Background refresh** - Press `R` to update from GitHub in the background

The title bar shows cache status: `(cached)`, `(refreshing...)`, or `(offline)`.

## Configuration

| Flag             | Env Var     | Default | Description                        |
| ---------------- | ----------- | ------- | ---------------------------------- |
| `--tool-home`    | `TOOL_HOME` | `~/dev` | Where repos are cloned             |
| `--dry-run`      |             | `false` | Preview without changes            |
| `--yes` `-y`     |             | `false` | Skip confirmation, sync all cloned |
| `--refresh` `-r` |             | `false` | Force refresh from GitHub          |

## Project Structure

```
src/
├── main.rs      # Entry point and event loop
├── cli.rs       # CLI argument parsing
├── types.rs     # Data structures (Fork, SyncStatus, Mode, etc.)
├── github.rs    # GitHub API interactions (GraphQL + REST)
├── cache.rs     # SQLite caching for fork metadata
├── sync.rs      # Sync/clone/archive operations (async)
├── app.rs       # Application state and logic
└── ui.rs        # TUI rendering
```

## Development

```bash
# Run with dry-run mode
cargo run -- --dry-run

# Run linters and checks
hk check

# Format code
cargo fmt

# Run clippy
cargo clippy
```

### Code Quality

- **File length limit**: 500 lines max per file (enforced via `scripts/check-file-length.sh`)
- **Clippy**: All warnings treated as errors
- **Formatting**: Enforced via `cargo fmt`

### Faster Builds with sccache

For faster incremental builds, install [sccache](https://github.com/mozilla/sccache):

```bash
cargo install sccache

# Create local cargo config
mkdir -p .cargo
echo '[build]
rustc-wrapper = "sccache"' > .cargo/config.toml
```

The `.cargo/` directory is gitignored for local configuration.

## License

MIT
