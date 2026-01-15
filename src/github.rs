use crate::types::{Fork, GhFork};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Fetch all forks from GitHub for the authenticated user.
pub fn fetch_forks(tool_home: &Path) -> Result<Vec<Fork>> {
    let output = Command::new("gh")
        .args([
            "repo",
            "list",
            "--fork",
            "--limit",
            "200",
            "--json",
            "name,owner,parent,defaultBranchRef,isArchived,description,primaryLanguage",
        ])
        .output()
        .context("Failed to run gh CLI. Is it installed and authenticated?")?;

    if !output.status.success() {
        anyhow::bail!(
            "gh command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let gh_forks: Vec<GhFork> = serde_json::from_slice(&output.stdout)?;

    let forks: Vec<Fork> = gh_forks
        .into_iter()
        .filter(|f| !f.is_archived)
        .filter_map(|f| {
            let parent = f.parent?;
            let default_branch = f
                .default_branch_ref
                .map_or_else(|| "main".to_string(), |b| b.name);

            let local_path = tool_home.join(&f.owner.login).join(&f.name);
            let is_cloned = local_path.exists();

            Some(Fork {
                name: f.name,
                owner: f.owner.login,
                parent_owner: parent.owner.login,
                parent_name: parent.name,
                default_branch,
                local_path,
                is_cloned,
                description: f.description,
                primary_language: f.primary_language.map(|l| l.name),
            })
        })
        .collect();

    Ok(forks)
}

/// Truncate an error message for display in the TUI.
pub fn truncate_error(err: &str) -> String {
    let cleaned = err.trim().lines().next().unwrap_or(err);
    if cleaned.len() > 30 {
        format!("{}...", &cleaned[..27])
    } else {
        cleaned.to_string()
    }
}
