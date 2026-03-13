use std::path::Path;

use git2::{Repository, Signature};
use tempfile::TempDir;

/// Create a temp git repo where each `(tag_name, catalog_content)` pair becomes a
/// commit with the catalog file written at `catalog_path`, then tagged.
///
/// Returns the `TempDir` — the repo lives at `tmpdir.path()`.
fn create_test_repo(catalog_path: &str, tags: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    let repo = Repository::init(dir.path()).expect("failed to init repo");
    let sig = Signature::now("test", "test@test.com").expect("failed to create signature");

    for (tag_name, content) in tags {
        // Write the catalog file to disk.
        let file_path = dir.path().join(catalog_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create parent dirs");
        }
        std::fs::write(&file_path, content).expect("failed to write catalog file");

        // Stage and commit.
        let mut index = repo.index().expect("failed to get index");
        index
            .add_path(Path::new(catalog_path))
            .expect("failed to add file to index");
        index.write().expect("failed to write index");
        let tree_id = index.write_tree().expect("failed to write tree");
        let tree = repo.find_tree(tree_id).expect("failed to find tree");

        let parent: Vec<git2::Commit<'_>> = match repo.head() {
            Ok(head) => vec![
                head.peel_to_commit()
                    .expect("failed to peel HEAD to commit"),
            ],
            Err(_) => vec![],
        };
        let parent_refs: Vec<&git2::Commit<'_>> = parent.iter().collect();

        let commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, tag_name, &tree, &parent_refs)
            .expect("failed to create commit");

        let commit_obj = repo
            .find_object(commit_id, None)
            .expect("failed to find commit object");
        repo.tag_lightweight(tag_name, &commit_obj, false)
            .expect("failed to create tag");
    }

    dir
}

#[test]
fn list_tags_empty_repo() {
    let dir = TempDir::new().unwrap();
    Repository::init(dir.path()).unwrap();

    let tags = dependency_drift::git::list_tags(dir.path()).unwrap();
    assert!(tags.is_empty());
}

#[test]
fn list_tags_returns_tags() {
    let dir = create_test_repo(
        "libs.versions.toml",
        &[
            ("v1.0.0", "[versions]\na = \"1.0\""),
            ("v2.0.0", "[versions]\na = \"2.0\""),
            ("v3.0.0", "[versions]\na = \"3.0\""),
        ],
    );

    let tags = dependency_drift::git::list_tags(dir.path()).unwrap();
    assert_eq!(tags.len(), 3);
    assert!(tags.contains(&"v1.0.0".to_string()));
    assert!(tags.contains(&"v2.0.0".to_string()));
    assert!(tags.contains(&"v3.0.0".to_string()));
}

#[test]
fn read_file_at_tag_returns_content() {
    let catalog = "[versions]\nfoo = \"1.0.0\"";
    let dir = create_test_repo("libs.versions.toml", &[("v1.0.0", catalog)]);

    let content =
        dependency_drift::git::read_file_at_tag(dir.path(), "v1.0.0", "libs.versions.toml")
            .unwrap();
    assert_eq!(content.as_deref(), Some(catalog));
}

#[test]
fn read_file_at_tag_missing_file() {
    let dir = create_test_repo(
        "libs.versions.toml",
        &[("v1.0.0", "[versions]\na = \"1.0\"")],
    );

    let content =
        dependency_drift::git::read_file_at_tag(dir.path(), "v1.0.0", "nonexistent.toml").unwrap();
    assert_eq!(content, None);
}

#[test]
fn read_file_at_tag_invalid_tag() {
    let dir = create_test_repo(
        "libs.versions.toml",
        &[("v1.0.0", "[versions]\na = \"1.0\"")],
    );

    let result =
        dependency_drift::git::read_file_at_tag(dir.path(), "no-such-tag", "libs.versions.toml");
    assert!(result.is_err());
}

#[test]
fn extract_drift_single_tag() {
    let catalog = "\
[versions]
kotlin_version = \"2.0.0\"
guava_version = \"33.0.0\"
";
    let dir = create_test_repo("gradle/libs.versions.toml", &[("v1.0.0", catalog)]);
    let tags = vec!["v1.0.0".to_string()];

    let payload =
        dependency_drift::extract_drift(dir.path(), &tags, "gradle/libs.versions.toml").unwrap();

    assert_eq!(payload.tags, vec!["v1.0.0"]);
    assert_eq!(payload.dependencies.len(), 2);

    let guava = payload
        .dependencies
        .iter()
        .find(|d| d.name == "guava")
        .unwrap();
    assert_eq!(guava.versions, vec![Some("33.0.0".to_string())]);

    let kotlin = payload
        .dependencies
        .iter()
        .find(|d| d.name == "kotlin")
        .unwrap();
    assert_eq!(kotlin.versions, vec![Some("2.0.0".to_string())]);
}

#[test]
fn extract_drift_tracks_version_changes() {
    let v1 = "[versions]\nkotlin_version = \"1.9.0\"";
    let v2 = "[versions]\nkotlin_version = \"2.0.0\"";
    let dir = create_test_repo(
        "gradle/libs.versions.toml",
        &[("v1.0.0", v1), ("v2.0.0", v2)],
    );
    let tags = vec!["v1.0.0".to_string(), "v2.0.0".to_string()];

    let payload =
        dependency_drift::extract_drift(dir.path(), &tags, "gradle/libs.versions.toml").unwrap();

    let kotlin = payload
        .dependencies
        .iter()
        .find(|d| d.name == "kotlin")
        .unwrap();
    assert_eq!(
        kotlin.versions,
        vec![Some("1.9.0".to_string()), Some("2.0.0".to_string())]
    );
}

#[test]
fn extract_drift_dependency_added_later() {
    let v1 = "[versions]\nkotlin_version = \"1.9.0\"";
    let v2 = "[versions]\nkotlin_version = \"2.0.0\"\nguava_version = \"33.0.0\"";
    let dir = create_test_repo(
        "gradle/libs.versions.toml",
        &[("v1.0.0", v1), ("v2.0.0", v2)],
    );
    let tags = vec!["v1.0.0".to_string(), "v2.0.0".to_string()];

    let payload =
        dependency_drift::extract_drift(dir.path(), &tags, "gradle/libs.versions.toml").unwrap();

    let guava = payload
        .dependencies
        .iter()
        .find(|d| d.name == "guava")
        .unwrap();
    assert_eq!(guava.versions, vec![None, Some("33.0.0".to_string())]);
}

#[test]
fn extract_drift_missing_catalog_at_tag() {
    // First tag has the catalog at a different path, so it's "missing" at the expected path.
    // Second tag has it at the expected path.
    let dir = create_test_repo("other.toml", &[("v1.0.0", "[versions]\na = \"1.0\"")]);

    // Create a second commit with the catalog at the expected path.
    let repo = Repository::open(dir.path()).unwrap();
    let sig = Signature::now("test", "test@test.com").unwrap();
    let catalog_path = dir.path().join("gradle/libs.versions.toml");
    std::fs::create_dir_all(catalog_path.parent().unwrap()).unwrap();
    std::fs::write(&catalog_path, "[versions]\nfoo = \"1.0.0\"").unwrap();

    let mut index = repo.index().unwrap();
    index
        .add_path(Path::new("gradle/libs.versions.toml"))
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "v2.0.0", &tree, &[&head])
        .unwrap();
    let obj = repo.find_object(commit_id, None).unwrap();
    repo.tag_lightweight("v2.0.0", &obj, false).unwrap();

    let tags = vec!["v1.0.0".to_string(), "v2.0.0".to_string()];
    let payload =
        dependency_drift::extract_drift(dir.path(), &tags, "gradle/libs.versions.toml").unwrap();

    // v1.0.0 has no catalog at that path → dep is None. v2.0.0 has it → Some.
    let foo = payload
        .dependencies
        .iter()
        .find(|d| d.name == "foo")
        .unwrap();
    assert_eq!(foo.versions, vec![None, Some("1.0.0".to_string())]);
}

#[test]
fn extract_drift_preserves_categories() {
    let catalog = "\
[versions]
# External dependencies
guava_version = \"33.0.0\"

# Test dependencies
junit_version = \"5.10.0\"
";
    let dir = create_test_repo("gradle/libs.versions.toml", &[("v1.0.0", catalog)]);
    let tags = vec!["v1.0.0".to_string()];

    let payload =
        dependency_drift::extract_drift(dir.path(), &tags, "gradle/libs.versions.toml").unwrap();

    let guava = payload
        .dependencies
        .iter()
        .find(|d| d.name == "guava")
        .unwrap();
    assert_eq!(guava.category.as_deref(), Some("external"));

    let junit = payload
        .dependencies
        .iter()
        .find(|d| d.name == "junit")
        .unwrap();
    assert_eq!(junit.category.as_deref(), Some("test"));
}

#[test]
fn render_html_contains_json_payload() {
    let payload = dependency_drift::DriftPayload {
        tags: vec!["v1.0.0".to_string()],
        dependencies: vec![dependency_drift::Dependency {
            name: "kotlin".to_string(),
            category: Some("internal".to_string()),
            versions: vec![Some("2.0.0".to_string())],
        }],
    };

    let html = dependency_drift::render_html(&payload).unwrap();

    // The placeholder should be gone.
    assert!(!html.contains("/*__DATA__*/null/*__END__*/"));
    // The JSON payload should be embedded.
    assert!(html.contains("\"kotlin\""));
    assert!(html.contains("\"v1.0.0\""));
    assert!(html.contains("\"2.0.0\""));
}

#[test]
fn render_html_escapes_closing_script_tag() {
    let payload = dependency_drift::DriftPayload {
        tags: vec!["v1.0.0".to_string()],
        dependencies: vec![dependency_drift::Dependency {
            name: "evil".to_string(),
            category: None,
            versions: vec![Some("</script><script>alert(1)</script>".to_string())],
        }],
    };

    let html = dependency_drift::render_html(&payload).unwrap();

    // The injected JSON should use the escaped form.
    assert!(html.contains("<\\/script>"));

    // The literal dangerous sequence must not appear within the DATA region.
    let data_start = html.find("var DATA = ").expect("DATA variable not found");
    let data_region = &html[data_start..data_start + 500];
    assert!(
        !data_region.contains("</script>"),
        "unescaped </script> found in DATA region"
    );
}
