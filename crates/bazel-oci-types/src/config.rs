use serde::{Deserialize, Serialize};

/// OCI config blob for a Bazel module version.
///
/// Field names use camelCase to match the Go `bcroci.ModuleConfig` JSON format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleConfig {
    pub name: String,
    pub version: String,
    #[serde(rename = "compatibilityLevel")]
    pub compatibility_level: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yanked: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "yankReason"
    )]
    pub yank_reason: Option<String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_module_config_roundtrip_matches_go() {
        // This is the exact JSON format produced by Go's json.Marshal of bcroci.ModuleConfig
        let go_json = r#"{"name":"rules_go","version":"0.50.1","compatibilityLevel":0,"homepage":"https://github.com/bazelbuild/rules_go","repository":["github:bazelbuild/rules_go"]}"#;

        let cfg: ModuleConfig = serde_json::from_str(go_json).unwrap();
        assert_eq!(cfg.name, "rules_go");
        assert_eq!(cfg.version, "0.50.1");
        assert_eq!(cfg.compatibility_level, 0);
        assert_eq!(
            cfg.homepage.as_deref(),
            Some("https://github.com/bazelbuild/rules_go")
        );
        assert_eq!(
            cfg.repository.as_ref().unwrap(),
            &vec!["github:bazelbuild/rules_go".to_owned()]
        );
        assert!(cfg.yanked.is_none());
        assert!(cfg.yank_reason.is_none());

        // Re-serialize and verify camelCase
        let reserialized = serde_json::to_string(&cfg).unwrap();
        assert!(reserialized.contains("\"compatibilityLevel\":0"));
        assert!(!reserialized.contains("compatibility_level"));
        assert!(!reserialized.contains("yanked"));
        assert!(!reserialized.contains("yankReason"));
    }

    #[test]
    fn test_module_config_with_yank() {
        let cfg = ModuleConfig {
            name: "rules_go".to_owned(),
            version: "0.49.0".to_owned(),
            compatibility_level: 0,
            homepage: None,
            repository: None,
            yanked: Some(true),
            yank_reason: Some("security issue".to_owned()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"yanked\":true"));
        assert!(json.contains("\"yankReason\":\"security issue\""));
    }

    #[test]
    fn test_module_config_minimal() {
        let cfg = ModuleConfig {
            name: "mod".to_owned(),
            version: "1.0".to_owned(),
            compatibility_level: 0,
            homepage: None,
            repository: None,
            yanked: None,
            yank_reason: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(
            json,
            r#"{"name":"mod","version":"1.0","compatibilityLevel":0}"#
        );
    }

    #[test]
    fn test_module_config_with_only_homepage() {
        let cfg = ModuleConfig {
            name: "mod".to_owned(),
            version: "1.0".to_owned(),
            compatibility_level: 1,
            homepage: Some("https://example.com".to_owned()),
            repository: None,
            yanked: None,
            yank_reason: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"homepage\":\"https://example.com\""));
        assert!(!json.contains("\"repository\""));
        assert!(!json.contains("\"yanked\""));
        assert!(!json.contains("\"yankReason\""));
    }

    #[test]
    fn test_module_config_negative_compatibility_level() {
        let cfg = ModuleConfig {
            name: "mod".to_owned(),
            version: "1.0".to_owned(),
            compatibility_level: -1,
            homepage: None,
            repository: None,
            yanked: None,
            yank_reason: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: ModuleConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.compatibility_level, -1);
    }

    #[test]
    fn test_module_config_yanked_false_serializes() {
        let cfg = ModuleConfig {
            name: "mod".to_owned(),
            version: "1.0".to_owned(),
            compatibility_level: 0,
            homepage: None,
            repository: None,
            yanked: Some(false),
            yank_reason: Some(String::new()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"yanked\":false"));
        assert!(json.contains("\"yankReason\":\"\""));
    }

    #[test]
    fn test_module_config_multiple_repositories() {
        let cfg = ModuleConfig {
            name: "mod".to_owned(),
            version: "1.0".to_owned(),
            compatibility_level: 0,
            homepage: None,
            repository: Some(vec![
                "github:org/repo".to_owned(),
                "gitlab:org/repo".to_owned(),
            ]),
            yanked: None,
            yank_reason: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: ModuleConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.repository.unwrap().len(), 2);
    }

    #[test]
    fn test_module_config_roundtrip_all_fields() {
        let cfg = ModuleConfig {
            name: "rules_go".to_owned(),
            version: "0.50.1".to_owned(),
            compatibility_level: 3,
            homepage: Some("https://example.com".to_owned()),
            repository: Some(vec!["github:org/repo".to_owned()]),
            yanked: Some(true),
            yank_reason: Some("critical bug".to_owned()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: ModuleConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn test_module_config_go_json_with_zero_values() {
        // Go marshals zero-value booleans and empty strings
        let go_json = r#"{"name":"mod","version":"1.0","compatibilityLevel":0,"homepage":"","repository":[],"yanked":false,"yankReason":""}"#;
        let cfg: ModuleConfig = serde_json::from_str(go_json).unwrap();
        assert_eq!(cfg.name, "mod");
        assert_eq!(cfg.compatibility_level, 0);
        assert_eq!(cfg.homepage, Some(String::new()));
        assert_eq!(cfg.repository, Some(vec![]));
        assert_eq!(cfg.yanked, Some(false));
        assert_eq!(cfg.yank_reason, Some(String::new()));
    }
}
