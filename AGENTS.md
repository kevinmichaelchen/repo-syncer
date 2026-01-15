# AGENTS.md

Instructions for AI agents working on this codebase.

## Project Overview

**repo-syncer** is a Rust TUI application that syncs GitHub forks with their upstream repositories. It uses the `gh` CLI for GitHub operations and `git` for local repository management.

## Architecture

- **Single-file design**: All code lives in `src/main.rs` (~900 lines)
- **No async**: Uses `std::thread` and `mpsc` channels for background operations
- **TUI framework**: `ratatui` + `crossterm`
- **GitHub integration**: Shells out to `gh` CLI (no direct API calls)

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
                        ↓                                  ↓
                  Mode::Selecting ←──────────────────── (reset)
```

## Code Quality

### Clippy Configuration

We use strict clippy with some pragmatic exceptions (see `Cargo.toml`):

- `all` and `pedantic` at deny level
- Allowed: `too_many_lines`, `collapsible_if`, cast-related lints

### Testing

- No unit tests currently (operations are mostly I/O with `gh`/`git`)
- Test manually with `--dry-run` flag
- CI runs `cargo check`, `cargo clippy`, `cargo fmt --check`

## Common Tasks

### Adding a New Sync Status

1. Add variant to `SyncStatus` enum
2. Update `SyncStatus::display()` method
3. Update status icon match in `ui()` function
4. Update style match in `ui()` function

### Modifying Git Operations

All git operations are in `sync_single_fork()`. The function:

1. Checks clone status
2. Stashes if dirty
3. Syncs with upstream
4. Restores original state

Always ensure state restoration happens even on error paths.

## Dependencies

| Crate       | Purpose              |
| ----------- | -------------------- |
| `clap`      | CLI argument parsing |
| `ratatui`   | TUI rendering        |
| `crossterm` | Terminal I/O         |
| `serde`     | JSON deserialization |
| `anyhow`    | Error handling       |

## External Requirements

- `gh` CLI must be installed and authenticated
- `git` must be available in PATH
