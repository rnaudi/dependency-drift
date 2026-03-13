//! Parse Gradle version catalogs (`libs.versions.toml`).
//!
//! Extracts `(name, version, category)` tuples from the `[versions]` section. Handles three
//! version formats found in real catalogs:
//!
//! - **Exact**: `"3.2.0"` → `"3.2.0"`
//! - **Range**: `"[2.28, 2.29)"` → lower bound `"2.28"`
//! - **Dynamic**: `"1.7.+"` → `"1.7"` (trailing `+` stripped)
//! - **Latest**: `"latest.release"` → kept as-is (JS `parseSemver` returns null gracefully)
//!
//! Categories are inferred from `#` comment headers in the `[versions]` section:
//!
//! - Lines before any comment → `"internal"`
//! - `# External dependencies` → `"external"`
//! - `# Test dependencies` → `"test"`
//! - `# Plugins and processors` → `"plugin"`
//! - Any other `# Foo` → lowercase trimmed text

/// A parsed version entry from the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionEntry {
    /// Human-readable name (key with `_version` stripped, underscores → hyphens).
    pub name: String,
    /// Normalized version string.
    pub version: String,
    /// Category inferred from comment headers.
    pub category: String,
}

/// Parse the `[versions]` section of a Gradle version catalog.
///
/// Returns [`VersionEntry`] values with cleaned-up names and comment-based categories.
#[must_use]
pub fn parse_versions(content: &str) -> Vec<VersionEntry> {
    let mut result = Vec::new();
    let mut in_versions = false;
    let mut current_category = "internal".to_string();

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect section headers.
        if trimmed.starts_with('[') {
            in_versions = trimmed == "[versions]";
            continue;
        }

        if !in_versions {
            continue;
        }

        // Track comment headers as category boundaries.
        if trimmed.starts_with('#') {
            current_category = categorize_comment(trimmed);
            continue;
        }

        // Skip blank lines.
        if trimmed.is_empty() {
            continue;
        }

        // Parse `key = "value"` lines.
        if let Some((key, value)) = parse_version_line(trimmed) {
            let name = clean_key(&key);
            let version = normalize_version(&value);
            result.push(VersionEntry {
                name,
                version,
                category: current_category.clone(),
            });
        }
    }

    result
}

/// Map a `# Comment` header to a category slug.
fn categorize_comment(comment: &str) -> String {
    let text = comment.trim_start_matches('#').trim().to_lowercase();

    // Recognize well-known headers.
    if text.starts_with("external") {
        return "external".to_string();
    }
    if text.starts_with("test") {
        return "test".to_string();
    }
    if text.starts_with("plugin") {
        return "plugin".to_string();
    }

    // Fall back to the raw text.
    if text.is_empty() {
        "internal".to_string()
    } else {
        text
    }
}

/// Clean a TOML key into a human-readable dependency name.
///
/// - Strip `_version` suffix: `hello_world_version` → `hello-world`
/// - Convert underscores to hyphens: `acme_sdk` → `acme-sdk`
fn clean_key(key: &str) -> String {
    let stripped = key.strip_suffix("_version").unwrap_or(key);
    stripped.replace('_', "-")
}

/// Parse a single `key = "value"` line, returning `(key, raw_value)`.
fn parse_version_line(line: &str) -> Option<(String, String)> {
    let (key, rest) = line.split_once('=')?;
    let key = key.trim().to_string();
    let rest = rest.trim();

    // Extract the string value between quotes.
    let value = rest.strip_prefix('"')?.strip_suffix('"')?.to_string();

    Some((key, value))
}

/// Normalize a version string:
/// - Range `[2.28, 2.29)` or `[2.28, 3.0)` → lower bound `2.28`
/// - Dynamic `1.7.+` → `1.7`
/// - Exact `3.2.0` → `3.2.0`
fn normalize_version(raw: &str) -> String {
    let trimmed = raw.trim();

    // Range version: starts with `[` or `(`
    if trimmed.starts_with('[') || trimmed.starts_with('(') {
        // Extract lower bound: everything between the opening bracket and the first comma.
        if let Some(comma_pos) = trimmed.find(',') {
            let lower = &trimmed[1..comma_pos];
            return lower.trim().to_string();
        }
        // Malformed range — return as-is.
        return trimmed.to_string();
    }

    // Dynamic version: ends with `+`
    if let Some(stripped) = trimmed.strip_suffix('+') {
        // Also strip trailing `.` if present (e.g. `1.7.+` → `1.7`)
        return stripped.strip_suffix('.').unwrap_or(stripped).to_string();
    }

    // Exact version — return as-is.
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_version ────────────────────────────────────────────────

    #[test]
    fn exact_version() {
        assert_eq!(normalize_version("3.2.0"), "3.2.0");
    }

    #[test]
    fn range_version_inclusive_exclusive() {
        assert_eq!(normalize_version("[2.28, 2.29)"), "2.28");
    }

    #[test]
    fn range_version_wide() {
        assert_eq!(normalize_version("[33.0.0, 34.0)"), "33.0.0");
    }

    #[test]
    fn dynamic_version_plus() {
        assert_eq!(normalize_version("1.7.+"), "1.7");
    }

    #[test]
    fn dynamic_version_trailing_plus() {
        assert_eq!(normalize_version("2023.9.+"), "2023.9");
    }

    #[test]
    fn latest_release_passthrough() {
        assert_eq!(normalize_version("latest.release"), "latest.release");
    }

    // ── clean_key ────────────────────────────────────────────────────────

    #[test]
    fn strip_version_suffix() {
        assert_eq!(clean_key("hello_world_version"), "hello-world");
    }

    #[test]
    fn strip_version_suffix_compound() {
        assert_eq!(clean_key("acme_sdk_version"), "acme-sdk");
    }

    #[test]
    fn no_version_suffix() {
        assert_eq!(clean_key("gatling"), "gatling");
    }

    #[test]
    fn underscores_to_hyphens() {
        assert_eq!(
            clean_key("apache_commons_validator_version"),
            "apache-commons-validator"
        );
    }

    // ── categorize_comment ───────────────────────────────────────────────

    #[test]
    fn category_external() {
        assert_eq!(categorize_comment("# External dependencies"), "external");
    }

    #[test]
    fn category_test() {
        assert_eq!(categorize_comment("# Test dependencies"), "test");
    }

    #[test]
    fn category_plugin() {
        assert_eq!(categorize_comment("# Plugins and processors"), "plugin");
    }

    #[test]
    fn category_custom() {
        assert_eq!(categorize_comment("# My custom group"), "my custom group");
    }

    #[test]
    fn category_empty_comment() {
        assert_eq!(categorize_comment("#"), "internal");
    }

    // ── parse_versions (integration) ─────────────────────────────────────

    #[test]
    fn parse_full_versions_section() {
        let content = r#"
[versions]
hello_world_version = "10.0.2"
guava_version = "[33.0.0, 34.0)"

# External dependencies
slf4j_version = "1.7.+"
open_rewrite_version = "latest.release"

# Test dependencies
junit_version = "[5.7.0, 6.0)"

[libraries]
hello-world = { group = "com.example", name = "hello-world", version.ref = "hello_world_version" }
"#;

        let result = parse_versions(content);
        assert_eq!(result.len(), 5);

        assert_eq!(
            result[0],
            VersionEntry {
                name: "hello-world".to_string(),
                version: "10.0.2".to_string(),
                category: "internal".to_string(),
            }
        );
        assert_eq!(
            result[1],
            VersionEntry {
                name: "guava".to_string(),
                version: "33.0.0".to_string(),
                category: "internal".to_string(),
            }
        );
        assert_eq!(
            result[2],
            VersionEntry {
                name: "slf4j".to_string(),
                version: "1.7".to_string(),
                category: "external".to_string(),
            }
        );
        assert_eq!(
            result[3],
            VersionEntry {
                name: "open-rewrite".to_string(),
                version: "latest.release".to_string(),
                category: "external".to_string(),
            }
        );
        assert_eq!(
            result[4],
            VersionEntry {
                name: "junit".to_string(),
                version: "5.7.0".to_string(),
                category: "test".to_string(),
            }
        );
    }

    #[test]
    fn empty_content_returns_empty() {
        assert!(parse_versions("").is_empty());
    }

    #[test]
    fn no_versions_section_returns_empty() {
        let content = "[libraries]\nfoo = { group = \"bar\" }";
        assert!(parse_versions(content).is_empty());
    }

    #[test]
    fn plugin_category() {
        let content = "\
[versions]
# Plugins and processors
spotless_plugin_version = \"8.1.0\"
spotbugs_version = \"4.9.8\"
";
        let result = parse_versions(content);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "spotless-plugin");
        assert_eq!(result[0].category, "plugin");
        assert_eq!(result[1].name, "spotbugs");
        assert_eq!(result[1].category, "plugin");
    }
}
