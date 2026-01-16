use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

// ============================================================
// STORAGE TRAIT
// ============================================================

/// Trait for fork metadata storage backends.
/// Implementations can use `SQLite`, `HelixDB`, or any other datastore.
pub trait ForkStore: Send {
    /// Load all forks from storage.
    fn load_forks(&self, tool_home: &Path) -> Result<Vec<Fork>>;

    /// Save multiple forks to storage.
    fn save_forks(&self, forks: &[Fork]) -> Result<()>;

    /// Check if the store is empty.
    fn is_empty(&self) -> Result<bool>;

    /// Get the timestamp of the last full sync.
    fn last_full_sync(&self) -> Result<Option<DateTime<Utc>>>;

    /// Set the timestamp of the last full sync.
    fn set_last_full_sync(&self, when: DateTime<Utc>) -> Result<()>;
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
    Deleting,
    /// Sync completed. Option<u32> is the number of commits fast-forwarded.
    Synced(Option<u32>),
    Skipped(String),
    Failed(String),
}

impl SyncStatus {
    pub fn display(&self) -> String {
        match self {
            Self::Pending => "Pending".to_string(),
            Self::Checking => "Checking".to_string(),
            Self::Cloning => "Cloning".to_string(),
            Self::Stashing => "Stashing".to_string(),
            Self::Fetching => "Fetching".to_string(),
            Self::Syncing => "Syncing".to_string(),
            Self::Restoring => "Restoring".to_string(),
            Self::Archiving => "Archiving".to_string(),
            Self::Deleting => "Deleting".to_string(),
            Self::Synced(None) => "Synced".to_string(),
            Self::Synced(Some(0)) => "Up-to-date".to_string(),
            Self::Synced(Some(n)) => format!("+{n} commits"),
            Self::Skipped(reason) | Self::Failed(reason) => reason.clone(),
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum Mode {
    Selecting,
    Search,
    StatsOverlay,
    ConfirmModal,
    ErrorPopup,
    Syncing,
    Done,
}

// ============================================================
// TOAST & ERROR HANDLING
// ============================================================

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: std::time::Instant,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // Reserved for future toast notifications
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[allow(dead_code)] // Reserved for future toast notifications
impl Toast {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Info,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Success,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: ToastLevel::Error,
            created_at: std::time::Instant::now(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ErrorDetails {
    pub title: String,
    pub message: String,
    pub action: Option<ErrorAction>,
}

#[derive(Clone, Debug)]
pub struct ErrorAction {
    pub label: String,
    pub command: String,
}

#[derive(PartialEq, Clone)]
pub enum ModalAction {
    Sync,
    Clone,
    Archive,
    Delete,
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
    ForkDeleted(usize),
    ForksRefreshed(Vec<Fork>),
    RefreshFailed(String),
    /// An error occurred that may have an actionable fix
    ActionableError(ErrorDetails),
}
