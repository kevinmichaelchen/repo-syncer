use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "repo-syncer")]
#[command(about = "Interactive TUI to sync GitHub forks with their upstream repositories")]
pub struct Args {
    /// Home directory for cloned repos (default: $HOME/dev)
    #[arg(long, env = "TOOL_HOME")]
    pub tool_home: Option<PathBuf>,

    /// Dry run - show what would be done without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation modal and sync all
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Force refresh from GitHub (ignore cache)
    #[arg(long, short = 'r')]
    pub refresh: bool,
}
