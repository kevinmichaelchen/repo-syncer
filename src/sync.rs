use crate::github::truncate_error;
use crate::types::{Fork, SyncResult, SyncStatus};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Start syncing selected forks in a background thread.
pub fn start_syncing(
    forks_to_sync: Vec<(usize, Fork)>,
    dry_run: bool,
    tx: mpsc::Sender<SyncResult>,
) {
    thread::spawn(move || {
        for (idx, fork) in forks_to_sync {
            sync_single_fork(idx, &fork, dry_run, &tx);
            thread::sleep(Duration::from_millis(100));
        }
    });
}

/// Clone a single fork in the background.
pub fn clone_fork_async(idx: usize, fork: Fork, dry_run: bool, tx: mpsc::Sender<SyncResult>) {
    thread::spawn(move || {
        clone_single_fork(idx, &fork, dry_run, &tx);
    });
}

/// Delete a single fork in the background (removes local clone and deletes from GitHub).
pub fn delete_fork_async(idx: usize, fork: Fork, dry_run: bool, tx: mpsc::Sender<SyncResult>) {
    thread::spawn(move || {
        let send = |status: SyncStatus| {
            let _ = tx.send(SyncResult::StatusUpdate(idx, status));
        };

        send(SyncStatus::Deleting);

        if dry_run {
            thread::sleep(Duration::from_millis(500));
            send(SyncStatus::Synced);
            let _ = tx.send(SyncResult::ForkDeleted(idx));
            return;
        }

        // Step 1: Delete local directory if it exists
        if fork.local_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&fork.local_path) {
                send(SyncStatus::Failed(truncate_error(&format!(
                    "rm local: {e}"
                ))));
                return;
            }
        }

        // Step 2: Delete the fork from GitHub
        let repo = format!("{}/{}", fork.owner, fork.name);
        let result = Command::new("gh")
            .args(["repo", "delete", &repo, "--yes"])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                send(SyncStatus::Synced);
                let _ = tx.send(SyncResult::ForkDeleted(idx));
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr);
                send(SyncStatus::Failed(truncate_error(&err)));
            }
            Err(e) => {
                send(SyncStatus::Failed(truncate_error(&e.to_string())));
            }
        }
    });
}

/// Archive a single fork in the background (async, non-blocking).
pub fn archive_fork_async(idx: usize, fork: Fork, dry_run: bool, tx: mpsc::Sender<SyncResult>) {
    thread::spawn(move || {
        let send = |status: SyncStatus| {
            let _ = tx.send(SyncResult::StatusUpdate(idx, status));
        };

        send(SyncStatus::Archiving);

        if dry_run {
            thread::sleep(Duration::from_millis(500));
            send(SyncStatus::Synced);
            let _ = tx.send(SyncResult::ForkArchived(idx));
            return;
        }

        let repo = format!("{}/{}", fork.owner, fork.name);
        let result = Command::new("gh")
            .args(["repo", "archive", &repo, "--yes"])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                send(SyncStatus::Synced);
                let _ = tx.send(SyncResult::ForkArchived(idx));
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr);
                send(SyncStatus::Failed(truncate_error(&err)));
            }
            Err(e) => {
                send(SyncStatus::Failed(truncate_error(&e.to_string())));
            }
        }
    });
}

/// Clone a single fork (runs in caller's thread context).
pub fn clone_single_fork(idx: usize, fork: &Fork, dry_run: bool, tx: &mpsc::Sender<SyncResult>) {
    let send = |status: SyncStatus| {
        let _ = tx.send(SyncResult::StatusUpdate(idx, status));
    };

    send(SyncStatus::Cloning);

    if dry_run {
        thread::sleep(Duration::from_millis(500));
        send(SyncStatus::Synced);
        let _ = tx.send(SyncResult::ForkCloned(idx));
        return;
    }

    // Ensure parent directory exists
    if let Some(parent) = fork.local_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            send(SyncStatus::Failed(format!("mkdir: {e}")));
            return;
        }
    }

    let clone_result = Command::new("gh")
        .args([
            "repo",
            "clone",
            &format!("{}/{}", fork.owner, fork.name),
            fork.local_path.to_string_lossy().as_ref(),
        ])
        .output();

    match clone_result {
        Ok(output) if output.status.success() => {
            send(SyncStatus::Synced);
            let _ = tx.send(SyncResult::ForkCloned(idx));
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            send(SyncStatus::Failed(truncate_error(&err)));
        }
        Err(e) => {
            send(SyncStatus::Failed(truncate_error(&e.to_string())));
        }
    }
}

/// Sync a single fork with its upstream (runs in caller's thread context).
pub fn sync_single_fork(idx: usize, fork: &Fork, dry_run: bool, tx: &mpsc::Sender<SyncResult>) {
    let send = |status: SyncStatus| {
        let _ = tx.send(SyncResult::StatusUpdate(idx, status));
    };

    send(SyncStatus::Checking);

    if dry_run {
        thread::sleep(Duration::from_millis(500));
        send(SyncStatus::Synced);
        return;
    }

    // Check if repo exists locally
    if !fork.local_path.exists() {
        clone_single_fork(idx, fork, dry_run, tx);
        return;
    }

    // Repo exists locally - sync it
    let path_str = fork.local_path.to_string_lossy();

    // Check for uncommitted changes
    let status_output = Command::new("git")
        .args(["-C", &path_str, "status", "--porcelain"])
        .output();

    let is_dirty = match status_output {
        Ok(output) => !output.stdout.is_empty(),
        Err(e) => {
            send(SyncStatus::Failed(truncate_error(&e.to_string())));
            return;
        }
    };

    // Get current branch
    let branch_output = Command::new("git")
        .args(["-C", &path_str, "rev-parse", "--abbrev-ref", "HEAD"])
        .output();

    let original_branch = match branch_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => {
            send(SyncStatus::Failed("get branch failed".to_string()));
            return;
        }
    };

    // Check for unpushed commits
    let unpushed = Command::new("git")
        .args([
            "-C",
            &path_str,
            "log",
            &format!("origin/{}..HEAD", fork.default_branch),
            "--oneline",
        ])
        .output();

    if let Ok(output) = unpushed {
        if !output.stdout.is_empty() {
            send(SyncStatus::Skipped("unpushed commits".to_string()));
            return;
        }
    }

    // Stash if dirty
    let mut stashed = false;
    if is_dirty {
        send(SyncStatus::Stashing);
        let stash_result = Command::new("git")
            .args([
                "-C",
                &path_str,
                "stash",
                "push",
                "-m",
                "repo-syncer auto-stash",
            ])
            .output();

        match stash_result {
            Ok(output) if output.status.success() => {
                stashed = true;
            }
            _ => {
                send(SyncStatus::Failed("stash failed".to_string()));
                return;
            }
        }
    }

    // Checkout default branch if not on it
    let on_default_branch = original_branch == fork.default_branch;
    if !on_default_branch {
        let checkout_result = Command::new("git")
            .args(["-C", &path_str, "checkout", &fork.default_branch])
            .output();

        if checkout_result.is_err() || !checkout_result.unwrap().status.success() {
            // Try to restore state
            if stashed {
                let _ = Command::new("git")
                    .args(["-C", &path_str, "stash", "pop"])
                    .output();
            }
            send(SyncStatus::Failed("checkout failed".to_string()));
            return;
        }
    }

    // Sync with upstream using gh repo sync
    send(SyncStatus::Syncing);
    let sync_result = Command::new("gh")
        .args([
            "repo",
            "sync",
            &format!("{}/{}", fork.owner, fork.name),
            "--source",
            &format!("{}/{}", fork.parent_owner, fork.parent_name),
            "--branch",
            &fork.default_branch,
        ])
        .output();

    let sync_success = match sync_result {
        Ok(output) => output.status.success(),
        Err(_) => false,
    };

    if !sync_success {
        // Try to restore state
        if !on_default_branch {
            let _ = Command::new("git")
                .args(["-C", &path_str, "checkout", &original_branch])
                .output();
        }
        if stashed {
            let _ = Command::new("git")
                .args(["-C", &path_str, "stash", "pop"])
                .output();
        }
        send(SyncStatus::Failed("sync failed".to_string()));
        return;
    }

    // Pull the changes locally
    send(SyncStatus::Fetching);
    let pull_result = Command::new("git")
        .args(["-C", &path_str, "pull", "--ff-only"])
        .output();

    if pull_result.is_err() || !pull_result.unwrap().status.success() {
        // Try fetch + reset instead
        let _ = Command::new("git")
            .args(["-C", &path_str, "fetch", "origin"])
            .output();
        let _ = Command::new("git")
            .args([
                "-C",
                &path_str,
                "reset",
                "--hard",
                &format!("origin/{}", fork.default_branch),
            ])
            .output();
    }

    // Restore original branch if we changed it
    if !on_default_branch {
        send(SyncStatus::Restoring);
        let _ = Command::new("git")
            .args(["-C", &path_str, "checkout", &original_branch])
            .output();
    }

    // Pop stash if we stashed
    if stashed {
        send(SyncStatus::Restoring);
        let _ = Command::new("git")
            .args(["-C", &path_str, "stash", "pop"])
            .output();
    }

    send(SyncStatus::Synced);
}
