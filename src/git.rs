//! Read-only git repository operations via `git2`.
//!
//! Provides the minimal git operations needed for drift analysis:
//! tag enumeration and single-file reads at arbitrary revisions.

use std::path::Path;

use anyhow::{Context, Result};
use git2::{Repository, Tree};

/// Resolve a tag (or any revspec) to the commit's root tree.
fn resolve_tree<'repo>(repo: &'repo Repository, tag: &str) -> Result<Tree<'repo>> {
    let obj = repo
        .revparse_single(tag)
        .with_context(|| format!("resolving revision `{tag}`"))?;
    let commit = obj
        .peel_to_commit()
        .with_context(|| format!("`{tag}` does not point to a commit"))?;
    let tree = commit
        .tree()
        .with_context(|| format!("reading tree for `{tag}`"))?;
    Ok(tree)
}

/// List all tags in the repository.
///
/// Returns tag names in the order git reports them (lexicographic by refname).
///
/// # Errors
///
/// Returns an error if the repository cannot be opened or tags cannot be listed.
pub fn list_tags(repo_path: &Path) -> Result<Vec<String>> {
    let repo = Repository::open(repo_path)
        .with_context(|| format!("opening repo at {}", repo_path.display()))?;
    let tags = repo.tag_names(None).context("listing tags")?;
    let result: Vec<String> = tags.iter().flatten().map(ToString::to_string).collect();
    Ok(result)
}

/// Read a single file at a given tag/revision.
///
/// Returns `Ok(Some(content))` if the file exists and is valid UTF-8,
/// `Ok(None)` if the file does not exist at that revision, or an error for other failures.
///
/// # Errors
///
/// Returns an error if the repository cannot be opened, the tag cannot be resolved,
/// or the tree entry is not a blob.
pub fn read_file_at_tag(repo_path: &Path, tag: &str, file_path: &str) -> Result<Option<String>> {
    let repo = Repository::open(repo_path)
        .with_context(|| format!("opening repo at {}", repo_path.display()))?;
    let tree = resolve_tree(&repo, tag)?;

    let Ok(entry) = tree.get_path(Path::new(file_path)) else {
        return Ok(None);
    };

    let blob = repo
        .find_blob(entry.id())
        .with_context(|| format!("`{file_path}` at `{tag}` is not a blob"))?;

    if blob.is_binary() {
        return Ok(None);
    }

    if let Ok(content) = String::from_utf8(blob.content().to_vec()) {
        Ok(Some(content))
    } else {
        eprintln!("warning: `{file_path}` at `{tag}` is not valid UTF-8, skipping");
        Ok(None)
    }
}
