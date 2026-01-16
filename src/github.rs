use crate::types::Fork;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

// ============================================================
// GRAPHQL TYPES
// ============================================================

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLData {
    viewer: GraphQLViewer,
}

#[derive(Debug, Deserialize)]
struct GraphQLViewer {
    repositories: GraphQLRepositories,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQLRepositories {
    page_info: GraphQLPageInfo,
    nodes: Vec<GraphQLFork>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQLPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQLFork {
    name: String,
    owner: GraphQLOwner,
    parent: Option<GraphQLParent>,
    default_branch_ref: Option<GraphQLBranchRef>,
    description: Option<String>,
    primary_language: Option<GraphQLLanguage>,
    created_at: String,
    updated_at: String,
    is_archived: bool,
}

#[derive(Debug, Deserialize)]
struct GraphQLOwner {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLParent {
    name: String,
    owner: GraphQLOwner,
}

#[derive(Debug, Deserialize)]
struct GraphQLBranchRef {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLLanguage {
    name: String,
}

// ============================================================
// REST API TYPES (legacy fallback)
// ============================================================

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GhFork {
    name: String,
    owner: GhOwner,
    parent: Option<GhParent>,
    default_branch_ref: Option<GhBranchRef>,
    is_archived: bool,
    description: Option<String>,
    primary_language: Option<GhLanguage>,
}

#[derive(Debug, Deserialize, Clone)]
struct GhOwner {
    login: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GhParent {
    name: String,
    owner: GhOwner,
}

#[derive(Debug, Deserialize, Clone)]
struct GhBranchRef {
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
struct GhLanguage {
    name: String,
}

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
                created_at: None, // REST API doesn't provide this efficiently
                updated_at: None,
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

// ============================================================
// GRAPHQL FETCHING
// ============================================================

const GRAPHQL_QUERY: &str = r"
query($cursor: String) {
  viewer {
    repositories(
      first: 100
      isFork: true
      orderBy: {field: CREATED_AT, direction: DESC}
      after: $cursor
    ) {
      pageInfo { hasNextPage endCursor }
      nodes {
        name
        owner { login }
        parent { name owner { login } }
        defaultBranchRef { name }
        description
        primaryLanguage { name }
        createdAt
        updatedAt
        isArchived
      }
    }
  }
}
";

/// Fetch all forks using GraphQL API (sorted by creation date, newest first).
pub fn fetch_forks_graphql(tool_home: &Path) -> Result<Vec<Fork>> {
    let mut all_forks = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={GRAPHQL_QUERY}"),
        ];

        if let Some(ref c) = cursor {
            args.push("-f".to_string());
            args.push(format!("cursor={c}"));
        }

        let output = Command::new("gh")
            .args(&args)
            .output()
            .context("Failed to run gh CLI for GraphQL query")?;

        if !output.status.success() {
            anyhow::bail!(
                "gh graphql failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let response: GraphQLResponse = serde_json::from_slice(&output.stdout)
            .context("Failed to parse GraphQL response")?;

        if let Some(errors) = response.errors {
            let messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            anyhow::bail!("GraphQL errors: {}", messages.join(", "));
        }

        let data = response.data.context("No data in GraphQL response")?;
        let repos = data.viewer.repositories;

        for node in repos.nodes {
            if node.is_archived {
                continue;
            }

            let Some(parent) = node.parent else {
                continue;
            };

            let default_branch = node
                .default_branch_ref
                .map_or_else(|| "main".to_string(), |b| b.name);

            let local_path = tool_home.join(&node.owner.login).join(&node.name);
            let is_cloned = local_path.exists();

            let created_at = DateTime::parse_from_rfc3339(&node.created_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc));

            let updated_at = DateTime::parse_from_rfc3339(&node.updated_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc));

            all_forks.push(Fork {
                name: node.name,
                owner: node.owner.login,
                parent_owner: parent.owner.login,
                parent_name: parent.name,
                default_branch,
                local_path,
                is_cloned,
                description: node.description,
                primary_language: node.primary_language.map(|l| l.name),
                created_at,
                updated_at,
            });
        }

        if repos.page_info.has_next_page {
            cursor = repos.page_info.end_cursor;
        } else {
            break;
        }
    }

    Ok(all_forks)
}

/// Fetch forks, trying GraphQL first with REST fallback.
pub fn fetch_forks_with_fallback(tool_home: &Path) -> Result<Vec<Fork>> {
    match fetch_forks_graphql(tool_home) {
        Ok(forks) => Ok(forks),
        Err(e) => {
            eprintln!("GraphQL fetch failed, falling back to REST: {e}");
            fetch_forks(tool_home)
        }
    }
}
