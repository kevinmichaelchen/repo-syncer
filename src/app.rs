use crate::types::{CacheStatus, Fork, ForkStats, ModalAction, Mode, SyncStatus};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct App {
    pub forks: Vec<Fork>,
    pub statuses: Vec<SyncStatus>,
    pub state: TableState,
    pub selected: Vec<bool>,
    pub mode: Mode,
    pub dry_run: bool,
    pub tool_home: PathBuf,
    pub spinner_tick: usize,
    pub last_tick: Instant,
    pub modal_button: usize,
    pub modal_action: ModalAction,
    // Search state
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub fuzzy_matcher: SkimMatcherV2,
    // Stats cache
    pub stats_cache: Option<ForkStats>,
    // Status message
    pub status_message: Option<(String, Instant)>,
    // Cache status
    pub cache_status: CacheStatus,
}

impl App {
    pub fn new(
        forks: Vec<Fork>,
        dry_run: bool,
        tool_home: PathBuf,
        cache_status: CacheStatus,
    ) -> Self {
        let len = forks.len();
        let mut state = TableState::default();
        if !forks.is_empty() {
            state.select(Some(0));
        }
        let search_results: Vec<usize> = (0..len).collect();
        Self {
            forks,
            statuses: vec![SyncStatus::Pending; len],
            state,
            selected: vec![false; len],
            mode: Mode::Selecting,
            dry_run,
            tool_home,
            spinner_tick: 0,
            last_tick: Instant::now(),
            modal_button: 1,
            modal_action: ModalAction::Sync,
            search_query: String::new(),
            search_results,
            fuzzy_matcher: SkimMatcherV2::default(),
            stats_cache: None,
            status_message: None,
            cache_status,
        }
    }

    pub fn visible_forks(&self) -> &[usize] {
        &self.search_results
    }

    pub fn current_fork_index(&self) -> Option<usize> {
        let visible_idx = self.state.selected()?;
        self.search_results.get(visible_idx).copied()
    }

    pub fn current_fork(&self) -> Option<&Fork> {
        self.current_fork_index().map(|i| &self.forks[i])
    }

    pub fn next(&mut self) {
        let visible = self.visible_forks();
        if visible.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + 1) % visible.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let visible = self.visible_forks();
        if visible.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    visible.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn toggle_selection(&mut self) {
        if let Some(idx) = self.current_fork_index() {
            self.selected[idx] = !self.selected[idx];
        }
    }

    pub fn select_all(&mut self) {
        let visible = self.visible_forks().to_vec();
        let all_selected = visible.iter().all(|&i| self.selected[i]);
        for &i in &visible {
            self.selected[i] = !all_selected;
        }
    }

    pub fn selected_count(&self) -> usize {
        self.selected.iter().filter(|&&s| s).count()
    }

    pub fn tick_spinner(&mut self) {
        if self.last_tick.elapsed() >= Duration::from_millis(80) {
            self.spinner_tick = (self.spinner_tick + 1) % SPINNER_FRAMES.len();
            self.last_tick = Instant::now();
        }
        // Clear old status messages
        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }
        }
    }

    pub fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_tick]
    }

    pub fn mark_selected_as_pending(&mut self) {
        for (i, selected) in self.selected.iter().enumerate() {
            if *selected {
                self.statuses[i] = SyncStatus::Pending;
            }
        }
    }

    pub fn is_all_done(&self) -> bool {
        self.statuses.iter().enumerate().all(|(i, status)| {
            !self.selected[i]
                || matches!(
                    status,
                    SyncStatus::Synced | SyncStatus::Skipped(_) | SyncStatus::Failed(_)
                )
        })
    }

    pub fn reset_for_next_round(&mut self) {
        for i in 0..self.forks.len() {
            if matches!(self.statuses[i], SyncStatus::Synced) {
                self.selected[i] = false;
            }
            self.statuses[i] = SyncStatus::Pending;
        }
        self.modal_button = 1;
    }

    pub fn summary(&self) -> (usize, usize, usize) {
        let mut synced = 0;
        let mut skipped = 0;
        let mut failed = 0;
        for (i, status) in self.statuses.iter().enumerate() {
            if !self.selected[i] {
                continue;
            }
            match status {
                SyncStatus::Synced => synced += 1,
                SyncStatus::Skipped(_) => skipped += 1,
                SyncStatus::Failed(_) => failed += 1,
                _ => {}
            }
        }
        (synced, skipped, failed)
    }

    pub fn update_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_results = (0..self.forks.len()).collect();
        } else {
            let mut results: Vec<(usize, i64)> = self
                .forks
                .iter()
                .enumerate()
                .filter_map(|(i, fork)| {
                    let haystack = format!("{}/{}", fork.parent_owner, fork.name);
                    self.fuzzy_matcher
                        .fuzzy_match(&haystack, &self.search_query)
                        .map(|score| (i, score))
                })
                .collect();
            results.sort_by(|a, b| b.1.cmp(&a.1));
            self.search_results = results.into_iter().map(|(i, _)| i).collect();
        }
        // Reset selection to first result
        if self.search_results.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    pub fn compute_stats(&mut self) {
        let mut lang_counts: HashMap<String, u64> = HashMap::new();
        let mut cloned = 0;
        let mut uncloned = 0;
        let mut synced = 0;
        let mut pending = 0;
        let mut failed = 0;

        for (i, fork) in self.forks.iter().enumerate() {
            if fork.is_cloned {
                cloned += 1;
            } else {
                uncloned += 1;
            }

            let lang = fork
                .primary_language
                .clone()
                .unwrap_or_else(|| "Unknown".to_string());
            *lang_counts.entry(lang).or_insert(0) += 1;

            match &self.statuses[i] {
                SyncStatus::Synced => synced += 1,
                SyncStatus::Failed(_) | SyncStatus::Skipped(_) => failed += 1,
                _ => pending += 1,
            }
        }

        let mut by_language: Vec<(String, u64)> = lang_counts.into_iter().collect();
        by_language.sort_by(|a, b| b.1.cmp(&a.1));
        by_language.truncate(8); // Top 8 languages

        self.stats_cache = Some(ForkStats {
            by_language,
            total: self.forks.len(),
            cloned,
            uncloned,
            synced,
            pending,
            failed,
        });
    }

    pub fn show_message(&mut self, msg: &str) {
        self.status_message = Some((msg.to_string(), Instant::now()));
    }

    /// Get forks selected for syncing as (index, fork) pairs.
    pub fn forks_to_sync(&self) -> Vec<(usize, Fork)> {
        self.forks
            .iter()
            .enumerate()
            .filter(|(i, _)| self.selected[*i])
            .map(|(i, f)| (i, f.clone()))
            .collect()
    }

    /// Remove a fork from the list (e.g., after archiving).
    pub fn remove_fork(&mut self, idx: usize) {
        if idx < self.forks.len() {
            self.forks.remove(idx);
            self.statuses.remove(idx);
            self.selected.remove(idx);
            self.update_search();
        }
    }
}
