//! CLI entry point for dependency-drift.
//!
//! Reads `gradle/libs.versions.toml` from git tags in a repository and generates
//! a self-contained HTML report showing how dependency versions change over time.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use semver::Version;

/// Dependency drift timeline across git tags.
///
/// Reads gradle/libs.versions.toml at each tag and generates a self-contained HTML
/// report showing how dependency versions change over time.
#[derive(Parser)]
#[command(
    name = "dependency-drift",
    version,
    about = "Dependency drift timeline across git tags"
)]
struct Args {
    /// Path to a local git repository.
    #[arg(long_help = "\
Path to a local git repository with tagged releases.

Tags are parsed as semver (a leading 'v' prefix is stripped). \
Non-semver tags are skipped.")]
    repo_path: PathBuf,

    /// Number of most recent semver tags to include. Use 0 for all tags.
    #[arg(long, default_value_t = 10)]
    last: usize,

    /// Path to the version catalog file within the repository.
    #[arg(long, default_value = "gradle/libs.versions.toml")]
    catalog: String,

    /// Output file path.
    #[arg(long, short, default_value = "dep-drift.html")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    run(&args)
}

fn run(args: &Args) -> Result<()> {
    let repo_path = &args.repo_path;

    let raw_tags = dependency_drift::git::list_tags(repo_path)
        .with_context(|| format!("listing tags in {}", repo_path.display()))?;

    if raw_tags.is_empty() {
        bail!("no tags found in {}", repo_path.display());
    }

    let total_tags = raw_tags.len();

    // Filter to semver, sort ascending.
    let mut semver_tags: Vec<(Version, String)> = raw_tags
        .into_iter()
        .filter_map(|tag| {
            let stripped = tag.strip_prefix('v').unwrap_or(&tag);
            Version::parse(stripped).ok().map(|v| (v, tag))
        })
        .collect();

    let skipped = total_tags - semver_tags.len();
    if skipped > 0 {
        eprintln!(
            "found {total_tags} tags ({skipped} non-semver skipped, {} semver)",
            semver_tags.len()
        );
    } else {
        eprintln!("found {total_tags} tags (all semver)");
    }

    if semver_tags.is_empty() {
        bail!(
            "no semver-compatible tags found in {} ({total_tags} tags exist but none parse as semver)",
            repo_path.display()
        );
    }

    semver_tags.sort_by(|(a, _), (b, _)| a.cmp(b));
    let tags: Vec<String> = semver_tags.into_iter().map(|(_, tag)| tag).collect();

    // `--last 0` means all tags; otherwise take the last N (clamped).
    let tags = if args.last == 0 {
        tags
    } else {
        let n = args.last.min(tags.len());
        tags[tags.len() - n..].to_vec()
    };

    eprintln!(
        "analyzing {} tags: {} \u{2192} {}",
        tags.len(),
        tags[0],
        tags[tags.len() - 1]
    );

    for tag in &tags {
        eprintln!("  reading: {tag}");
    }

    let payload = dependency_drift::extract_drift(repo_path, &tags, &args.catalog)?;

    eprintln!(
        "found {} dependencies across {} tags",
        payload.dependencies.len(),
        payload.tags.len()
    );

    let html = dependency_drift::render_html(&payload)?;

    std::fs::write(&args.output, &html)
        .with_context(|| format!("writing output to {}", args.output.display()))?;

    eprintln!("wrote {}", args.output.display());
    Ok(())
}
