//! Dependency drift analysis across git tags.
//!
//! Reads a Gradle version catalog (`gradle/libs.versions.toml`) at each tag in a git repository
//! and builds a timeline of dependency version changes. The result can be rendered as a
//! self-contained HTML report.

pub mod catalog;
pub mod git;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use catalog::VersionEntry;

/// The full payload injected into the HTML template.
#[derive(Debug, Serialize)]
pub struct DriftPayload {
    /// Tag names in chronological order (oldest first).
    pub tags: Vec<String>,
    /// One entry per dependency, with positional version arrays.
    pub dependencies: Vec<Dependency>,
}

/// A single dependency tracked across tags.
#[derive(Debug, Serialize)]
pub struct Dependency {
    /// Human-readable name (key with `_version` stripped, underscores → hyphens).
    pub name: String,
    /// Category inferred from comment headers in the version catalog
    /// (e.g. `"internal"`, `"external"`, `"test"`, `"plugin"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// `versions[i]` is the resolved version string at `tags[i]`, or `null` if the
    /// dependency was absent at that tag.
    pub versions: Vec<Option<String>>,
}

/// Parsed catalog snapshot for a single tag: maps dep name → (version, category).
type CatalogSnapshot = BTreeMap<String, (String, String)>;

/// Extract dependency drift data from a git repository.
///
/// For each tag in `tags`, reads the file at `catalog_path` (e.g.
/// `gradle/libs.versions.toml`) and parses the `[versions]` section. Returns a
/// [`DriftPayload`] with the union of all dependency names across all tags.
///
/// # Errors
///
/// Returns an error if a tag cannot be resolved or a file read fails.
pub fn extract_drift(
    repo_path: &Path,
    tags: &[String],
    catalog_path: &str,
) -> Result<DriftPayload> {
    // For each tag, parse the catalog into a map of name → (version, category).
    let mut per_tag: Vec<CatalogSnapshot> = Vec::with_capacity(tags.len());

    for tag in tags {
        let map = if let Some(content) = git::read_file_at_tag(repo_path, tag, catalog_path)? {
            let entries = catalog::parse_versions(&content);
            entries_to_snapshot(entries)
        } else {
            eprintln!("  warning: {catalog_path} not found at tag {tag}");
            BTreeMap::new()
        };
        per_tag.push(map);
    }

    // Collect the union of all dependency names (sorted).
    let all_names: Vec<String> = {
        let mut set = std::collections::BTreeSet::new();
        for map in &per_tag {
            for key in map.keys() {
                set.insert(key.clone());
            }
        }
        set.into_iter().collect()
    };

    // Build the dependency list with positional version arrays.
    // Category is taken from the latest tag where the dependency exists.
    let dependencies: Vec<Dependency> = all_names
        .into_iter()
        .map(|name| {
            let versions: Vec<Option<String>> = per_tag
                .iter()
                .map(|map| map.get(&name).map(|(v, _)| v.clone()))
                .collect();

            // Use category from the last tag that has this dependency.
            let category = per_tag
                .iter()
                .rev()
                .find_map(|map| map.get(&name).map(|(_, c)| c.clone()));

            Dependency {
                name,
                category,
                versions,
            }
        })
        .collect();

    Ok(DriftPayload {
        tags: tags.to_vec(),
        dependencies,
    })
}

/// Convert parsed version entries into a snapshot map.
fn entries_to_snapshot(entries: Vec<VersionEntry>) -> CatalogSnapshot {
    entries
        .into_iter()
        .map(|e| (e.name, (e.version, e.category)))
        .collect()
}

/// Generate a self-contained HTML report by injecting JSON into the template.
///
/// The template contains a `/*__DATA__*/null/*__END__*/` placeholder inside a `<script>` tag
/// that gets replaced with the serialized [`DriftPayload`] JSON.
///
/// # Errors
///
/// Returns an error if JSON serialization of the payload fails.
pub fn render_html(payload: &DriftPayload) -> Result<String> {
    let template = include_str!("dep-drift.html");
    let json = serde_json::to_string(payload).context("serializing DriftPayload to JSON")?;
    // Escape "</script>" sequences that could break out of the <script> tag.
    // serde_json does not escape "/" by default.
    let json = json.replace("</", "<\\/");
    let html = template.replace("/*__DATA__*/null/*__END__*/", &json);
    Ok(html)
}
