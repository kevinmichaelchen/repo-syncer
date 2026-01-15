# Future Ideas

Brainstorming for future versions of repo-syncer.

## Implemented

These ideas have been implemented:

- ✅ **Language & Stats Dashboard** - Bar chart of forks by language, total/cloned/uncloned counts
- ✅ **Interactive Filtering** - `/` to fuzzy search by name
- ✅ **Batch Operations** - Archive, clone, open in browser/editor
- ✅ **Modular Architecture** - Split into focused modules (cli, types, github, sync, app, ui)

---

## LLM-Assisted Categorization

Use an LLM to automatically classify forks into categories:

- **Tools** - CLIs, utilities, developer tools
- **Libraries** - packages, SDKs, frameworks
- **Demos** - example projects, tutorials, proof-of-concepts
- **Configs** - dotfiles, templates, boilerplates
- **Contributions** - forks where you've submitted PRs

Could analyze repo name, description, README, and file structure to infer category.

## Enhanced Stats Dashboard

Expand the statistics view with:

- **Pie chart** of forks by primary language
- **Timeline** of when forks were created vs last synced
- **Size breakdown** - disk usage per fork
- **Sync history** - track success/failure over time

## SQLite Caching

Cache fork metadata locally to:

- Avoid repeated GitHub API calls (rate limits)
- Enable offline browsing of fork list
- Track sync history over time
- Store LLM-generated categories persistently

Schema idea:

```sql
CREATE TABLE forks (
  id TEXT PRIMARY KEY,
  name TEXT,
  owner TEXT,
  parent_owner TEXT,
  language TEXT,
  description TEXT,
  category TEXT,
  last_synced_at DATETIME,
  created_at DATETIME,
  updated_at DATETIME
);

CREATE TABLE sync_history (
  id INTEGER PRIMARY KEY,
  fork_id TEXT,
  synced_at DATETIME,
  status TEXT,
  commits_pulled INTEGER
);
```

## Advanced Filtering

Expand filtering beyond name search:

- `f` to filter by category (once LLM categorization is added)
- `l` to filter by language
- `s` to filter by sync status (synced, pending, failed)
- Combined filters (e.g., "Go repos that failed to sync")

## Onefetch Integration

Leverage [onefetch](https://github.com/o2sh/onefetch) as a library to show rich repository info:

- Git history statistics
- Contributors
- License detection
- Code composition breakdown

Would require onefetch to expose a library API (currently CLI-only).

## Notifications

Optional integrations:

- Desktop notification when sync completes
- Slack/Discord webhook for sync summaries
- GitHub Actions to run scheduled syncs

## Bulk Delete

Add ability to delete forks (not just archive):

- `X` (shift+x) to delete with double confirmation
- Batch delete with multi-select
- Show warning about irreversibility

## Keyboard Shortcuts Help

Press `?` to show a help overlay with all keybindings, similar to vim's `:help`.

## Configuration File

Support a config file (`~/.config/repo-syncer/config.toml`) for:

- Default tool home directory
- Excluded repos (never show certain forks)
- Custom keybindings
- Theme/color preferences
