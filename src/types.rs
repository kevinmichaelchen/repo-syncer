use serde::Deserialize;
use std::path::PathBuf;

// ============================================================
// GITHUB API TYPES
// ============================================================

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GhFork {
    pub name: String,
    pub owner: GhOwner,
    pub parent: Option<GhParent>,
    pub default_branch_ref: Option<GhBranchRef>,
    pub is_archived: bool,
    pub description: Option<String>,
    pub primary_language: Option<GhLanguage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GhOwner {
    pub login: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GhParent {
    pub name: String,
    pub owner: GhOwner,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GhBranchRef {
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GhLanguage {
    pub name: String,
}

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
}
