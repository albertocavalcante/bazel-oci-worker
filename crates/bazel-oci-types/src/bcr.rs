use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Module-level metadata (metadata.json).
///
/// Field names use snake_case to match the Go `go-bcr` JSON format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metadata {
    pub versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yanked_versions: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintainers: Option<Vec<Maintainer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<Vec<String>>,
}

/// Module maintainer information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Maintainer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_user_id: Option<i64>,
}

/// Source description for a module version (source.json).
///
/// Field names use snake_case to match the Go `go-bcr` JSON format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub source_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patches: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_strip: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_type: Option<String>,
    // Git repository fields
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shallow_since: Option<String>,
    // Local path fields
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Registry-level configuration (bazel_registry.json).
///
/// Returned as `{}` for our registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryConfig {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn metadata_from_go_json_with_all_fields() {
        let json = r#"{"versions":["0.50.1"],"yanked_versions":{},"maintainers":[],"homepage":"","repository":[]}"#;
        let meta: Metadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.versions, vec!["0.50.1"]);
        assert_eq!(meta.yanked_versions, Some(HashMap::new()));
        assert_eq!(meta.maintainers, Some(vec![]));
        assert_eq!(meta.homepage, Some(String::new()));
        assert_eq!(meta.repository, Some(vec![]));
    }

    #[test]
    fn metadata_minimal_omits_none_fields() {
        let meta = Metadata {
            versions: vec!["1.0.0".to_owned()],
            yanked_versions: None,
            maintainers: None,
            homepage: None,
            repository: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert_eq!(json, r#"{"versions":["1.0.0"]}"#);
    }

    #[test]
    fn metadata_empty_versions() {
        let meta = Metadata {
            versions: vec![],
            yanked_versions: None,
            maintainers: None,
            homepage: None,
            repository: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert_eq!(json, r#"{"versions":[]}"#);
    }

    #[test]
    fn metadata_with_yanked_versions() {
        let meta = Metadata {
            versions: vec!["1.0.0".to_owned(), "1.1.0".to_owned()],
            yanked_versions: Some(HashMap::from([(
                "1.0.0".to_owned(),
                "security vulnerability".to_owned(),
            )])),
            maintainers: None,
            homepage: None,
            repository: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: Metadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.versions.len(), 2);
        let yanked = parsed.yanked_versions.unwrap();
        assert_eq!(yanked.get("1.0.0").unwrap(), "security vulnerability");
    }

    #[test]
    fn metadata_roundtrip_preserves_all_fields() {
        let meta = Metadata {
            versions: vec!["0.1.0".to_owned()],
            yanked_versions: Some(HashMap::new()),
            maintainers: Some(vec![Maintainer {
                name: Some("Alice".to_owned()),
                email: None,
                github: Some("alice".to_owned()),
                github_user_id: Some(42),
            }]),
            homepage: Some("https://example.com".to_owned()),
            repository: Some(vec!["github:org/repo".to_owned()]),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: Metadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, parsed);
    }

    #[test]
    fn source_archive_type_roundtrip() {
        let json = r#"{"type":"archive","url":"https://example.com/v1.tar.gz","integrity":"sha256-abc","strip_prefix":"mod-1.0","patches":{},"patch_strip":0}"#;
        let source: Source = serde_json::from_str(json).unwrap();
        assert_eq!(source.source_type.as_deref(), Some("archive"));
        assert_eq!(source.url.as_deref(), Some("https://example.com/v1.tar.gz"));
        assert_eq!(source.integrity.as_deref(), Some("sha256-abc"));
        assert_eq!(source.strip_prefix.as_deref(), Some("mod-1.0"));
        assert_eq!(source.patch_strip, Some(0));
    }

    #[test]
    fn source_type_field_serializes_as_type_not_source_type() {
        let source = Source {
            source_type: Some("archive".to_owned()),
            url: None,
            integrity: None,
            strip_prefix: None,
            patches: None,
            patch_strip: None,
            archive_type: None,
            remote: None,
            commit: None,
            shallow_since: None,
            path: None,
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"archive\""));
        assert!(!json.contains("source_type"));
    }

    #[test]
    fn source_git_type_roundtrip() {
        let json = r#"{"type":"git_repository","remote":"https://github.com/org/repo.git","commit":"abc123def","shallow_since":"2024-01-01"}"#;
        let source: Source = serde_json::from_str(json).unwrap();
        assert_eq!(source.source_type.as_deref(), Some("git_repository"));
        assert_eq!(
            source.remote.as_deref(),
            Some("https://github.com/org/repo.git")
        );
        assert_eq!(source.commit.as_deref(), Some("abc123def"));
        assert_eq!(source.shallow_since.as_deref(), Some("2024-01-01"));
        assert!(source.url.is_none());

        let reserialized = serde_json::to_string(&source).unwrap();
        let reparsed: Source = serde_json::from_str(&reserialized).unwrap();
        assert_eq!(source, reparsed);
    }

    #[test]
    fn source_local_path_type() {
        let json = r#"{"type":"local_path","path":"/home/user/modules/foo"}"#;
        let source: Source = serde_json::from_str(json).unwrap();
        assert_eq!(source.source_type.as_deref(), Some("local_path"));
        assert_eq!(source.path.as_deref(), Some("/home/user/modules/foo"));
    }

    #[test]
    fn source_minimal_omits_all_none_fields() {
        let source = Source {
            source_type: None,
            url: None,
            integrity: None,
            strip_prefix: None,
            patches: None,
            patch_strip: None,
            archive_type: None,
            remote: None,
            commit: None,
            shallow_since: None,
            path: None,
        };
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn maintainer_full_roundtrip() {
        let m = Maintainer {
            name: Some("Alice".to_owned()),
            email: Some("alice@example.com".to_owned()),
            github: Some("alice".to_owned()),
            github_user_id: Some(12345),
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: Maintainer = serde_json::from_str(&json).unwrap();
        assert_eq!(m, parsed);
    }

    #[test]
    fn maintainer_minimal() {
        let m = Maintainer {
            name: None,
            email: None,
            github: None,
            github_user_id: None,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn maintainer_parses_from_go_json() {
        let json = r#"{"name":"Alice","email":"alice@example.com","github":"alice","github_user_id":12345}"#;
        let m: Maintainer = serde_json::from_str(json).unwrap();
        assert_eq!(m.name.as_deref(), Some("Alice"));
        assert_eq!(m.email.as_deref(), Some("alice@example.com"));
        assert_eq!(m.github.as_deref(), Some("alice"));
        assert_eq!(m.github_user_id, Some(12345));
    }

    #[test]
    fn registry_config_serializes_to_empty_object() {
        let cfg = RegistryConfig {};
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn registry_config_roundtrip() {
        let json = "{}";
        let cfg: RegistryConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg, RegistryConfig {});
    }
}
