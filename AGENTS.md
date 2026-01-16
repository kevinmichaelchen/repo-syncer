# AGENTS.md

Instructions for AI agents working on this codebase.

## Project Overview

**repo-syncer** is a Rust TUI application that syncs GitHub forks with their
upstream repositories. It uses the `gh` CLI for GitHub operations and `git` for
local repository management.

## Architecture

```
src/
├── main.rs      # Entry point and event loop
├── cli.rs       # CLI argument parsing (clap)
├── types.rs     # Data structures (Fork, SyncStatus, Mode, etc.)
├── github.rs    # GitHub API interactions (GraphQL + REST via gh CLI)
├── cache.rs     # SQLite caching for fork metadata
├── sync.rs      # Sync/clone/archive operations (async via threads)
├── app.rs       # Application state and logic
└── ui.rs        # TUI rendering (ratatui)
```

### Key Design Decisions

- **Modular architecture**: Code split into focused modules (~200-400 lines
  each)
- **No async runtime**: Uses `std::thread` and `mpsc` channels for background
  operations
- **Pluggable storage**: `ForkStore` trait in `types.rs` abstracts storage
  backends
- **SQLite default**: `SqliteStore` in `cache.rs` implements `ForkStore`
- **GitHub GraphQL API**: Used for sorted fork fetching (via `gh api graphql`)
- **Offline support**: Works with cached data when GitHub is unavailable

## Key Patterns

### Error Handling

- Use `anyhow::Result<T>` for all fallible functions
- Use `.context()` for meaningful error messages
- In sync operations, continue on failure and report at end

### Git Operations

- Always use `git -C PATH` to operate on repos without changing directory
- Check for dirty state before operations
- Stash/unstash automatically to preserve user work
- Skip repos with unpushed commits (don't force-push or rebase)

### TUI State Machine

```
Mode::Selecting → Mode::ConfirmModal → Mode::Syncing → Mode::Done
      ↓                  ↓                                  ↓
Mode::Search       Mode::Selecting ←──────────────────── (reset)
      ↓
Mode::StatsOverlay
```

### Channel Messages

Background operations communicate via `SyncResult` enum:

- `StatusUpdate(idx, status)` - Update sync status for a fork
- `ForkCloned(idx)` - Mark fork as cloned
- `ForkArchived(idx)` - Remove fork from list
- `ForksRefreshed(forks)` - Replace fork list from background refresh
- `RefreshFailed(error)` - Show refresh error message

## Code Quality

### Clippy Configuration

We use strict clippy with some pragmatic exceptions (see `Cargo.toml`):

- `all` and `pedantic` at deny level
- Allowed: `too_many_lines`, `collapsible_if`, cast-related lints

### File Length Limit

- Maximum 500 lines per file (enforced via `scripts/check-file-length.sh`)
- Run `./scripts/check-file-length.sh` to verify

### Testing

- Unit tests in `cache.rs` for database operations
- Test manually with `--dry-run` flag
- CI runs `cargo check`, `cargo clippy`, `cargo fmt --check`

## Common Tasks

### Adding a New Sync Status

1. Add variant to `SyncStatus` enum in `types.rs`
2. Update `SyncStatus::display()` method
3. Update status icon match in `ui.rs` `render_fork_list()`
4. Update style match in `ui.rs` `render_fork_list()`

### Adding a New Keybinding

1. Add handler in `main.rs` `handle_selecting_mode()` (or appropriate mode
   handler)
2. Update help text in `ui.rs` `render_help_bar()`
3. Document in README.md keybindings table

### Modifying Git Operations

Git operations are in `sync.rs`. The `sync_single_fork()` function:

1. Checks clone status
2. Stashes if dirty
3. Syncs with upstream via `gh repo sync`
4. Pulls latest changes
5. Restores original state (branch + stash)

Always ensure state restoration happens even on error paths.

### Adding a New Storage Backend

Storage is abstracted via the `ForkStore` trait in `types.rs`:

```rust
pub trait ForkStore: Send {
    fn load_forks(&self, tool_home: &Path) -> Result<Vec<Fork>>;
    fn save_forks(&self, forks: &[Fork]) -> Result<()>;
    fn is_empty(&self) -> Result<bool>;
    fn last_full_sync(&self) -> Result<Option<DateTime<Utc>>>;
    fn set_last_full_sync(&self, when: DateTime<Utc>) -> Result<()>;
}
```

To add a new backend (e.g., `HelixDB`):

1. Create `src/helix.rs` with a struct implementing `ForkStore`
2. Add `mod helix;` to `main.rs`
3. Replace `SqliteStore::open()` with your store constructor

No changes needed to `app.rs`, `sync.rs`, or other modules.

### Current Storage Implementation

The default `SqliteStore` in `cache.rs` provides:

- `open()` - Open or create the database at `~/.cache/repo-syncer/forks.db`
- `load_forks()` - Load all cached forks
- `save_forks()` - Save forks to cache
- `last_full_sync()` / `set_last_full_sync()` - Track refresh times

## Dependencies

| Crate           | Purpose                         |
| --------------- | ------------------------------- |
| `clap`          | CLI argument parsing            |
| `ratatui`       | TUI rendering                   |
| `crossterm`     | Terminal I/O                    |
| `serde`         | JSON deserialization            |
| `anyhow`        | Error handling                  |
| `rusqlite`      | SQLite database                 |
| `dirs`          | XDG cache directory             |
| `chrono`        | DateTime handling               |
| `fuzzy-matcher` | Fuzzy search for fork filtering |

## External Requirements

- `gh` CLI must be installed and authenticated
- `git` must be available in PATH
