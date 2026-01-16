use chrono::{DateTime, Utc};
use std::path::PathBuf;

// ============================================================
// APPLICATION TYPES
// ============================================================

#[derive(Debug, Clone)]
pub struct Fork {
    pub name: String,
    pub owner: String,
    pub parent_owner: String,
    pub parent_name: String,
    pub default_branch: String,
    pub local_path: PathBuf,
    pub is_cloned: bool,
    pub description: Option<String>,
    pub primary_language: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CacheStatus {
    Fresh,
    Stale { refreshing: bool },
    Offline,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SyncStatus {
    Pending,
    Checking,
    Cloning,
    Stashing,
    Fetching,
    Syncing,
    Restoring,
    Archiving,
    Synced,
    Skipped(String),
    Failed(String),
}

impl SyncStatus {
    pub fn display(&self) -> &str {
        match self {
            Self::Pending => "Pending",
            Self::Checking => "Checking",
            Self::Cloning => "Cloning",
            Self::Stashing => "Stashing",
            Self::Fetching => "Fetching",
            Self::Syncing => "Syncing",
            Self::Restoring => "Restoring",
            Self::Archiving => "Archiving",
            Self::Synced => "Synced",
            Self::Skipped(reason) | Self::Failed(reason) => reason,
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum Mode {
    Selecting,
    Search,
    StatsOverlay,
    ConfirmModal,
    Syncing,
    Done,
}

#[derive(PartialEq, Clone)]
pub enum ModalAction {
    Sync,
    Clone,
    Archive,
}

#[allow(dead_code)] // Fields reserved for future stats display
pub struct ForkStats {
    pub by_language: Vec<(String, u64)>,
    pub total: usize,
    pub cloned: usize,
    pub uncloned: usize,
    pub synced: usize,
    pub pending: usize,
    pub failed: usize,
}

// ============================================================
// CHANNEL MESSAGES
// ============================================================

#[derive(Debug)]
pub enum SyncResult {
    StatusUpdate(usize, SyncStatus),
    ForkCloned(usize),
    ForkArchived(usize),
    ForksRefreshed(Vec<Fork>),
    RefreshFailed(String),
}
