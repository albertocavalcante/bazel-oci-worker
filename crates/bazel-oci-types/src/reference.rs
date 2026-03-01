use thiserror::Error;

const BAZEL_PREFIX: &str = "bazel";
const REGISTRY_REPO_NAME: &str = "_registry";

/// Well-known OCI tag for module-level metadata.
pub const METADATA_TAG: &str = "metadata";

/// Well-known OCI tag for registry configuration.
pub const REGISTRY_CONFIG_TAG: &str = "config";

#[derive(Debug, Error)]
pub enum ReferenceError {
    #[error("reference {0:?} has no tag")]
    NoTag(String),
    #[error("reference {0:?} has empty tag")]
    EmptyTag(String),
    #[error("reference {0:?} does not contain /bazel/ segment")]
    NoBazelSegment(String),
    #[error("reference {0:?} has empty module name")]
    EmptyModule(String),
    #[error("reference {0:?} has no registry")]
    NoRegistry(String),
    #[error("reference {0:?} has no namespace")]
    NoNamespace(String),
}

/// Parsed components of an OCI reference for a Bazel module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedReference {
    pub registry: String,
    pub namespace: String,
    pub module: String,
    pub tag: String,
}

impl ParsedReference {
    /// Returns true if this reference points to module metadata.
    pub fn is_metadata(&self) -> bool {
        self.tag == METADATA_TAG
    }

    /// Returns true if this reference points to the registry config.
    pub fn is_registry_config(&self) -> bool {
        self.module == REGISTRY_REPO_NAME && self.tag == REGISTRY_CONFIG_TAG
    }
}

/// Build the OCI reference for a specific module version.
///
/// Format: `{registry}/{namespace}/bazel/{module}:{version}`
pub fn module_reference(registry: &str, namespace: &str, module: &str, version: &str) -> String {
    format!("{registry}/{namespace}/{BAZEL_PREFIX}/{module}:{version}")
}

/// Build the OCI reference for a module's metadata.
///
/// Format: `{registry}/{namespace}/bazel/{module}:metadata`
pub fn metadata_reference(registry: &str, namespace: &str, module: &str) -> String {
    format!("{registry}/{namespace}/{BAZEL_PREFIX}/{module}:{METADATA_TAG}")
}

/// Build the OCI reference for the registry config.
///
/// Format: `{registry}/{namespace}/bazel/_registry:config`
pub fn registry_config_reference(registry: &str, namespace: &str) -> String {
    format!("{registry}/{namespace}/{BAZEL_PREFIX}/{REGISTRY_REPO_NAME}:{REGISTRY_CONFIG_TAG}")
}

/// Return the OCI repository path for a module (without tag).
///
/// Format: `{namespace}/bazel/{module}`
pub fn module_repository(namespace: &str, module: &str) -> String {
    format!("{namespace}/{BAZEL_PREFIX}/{module}")
}

/// Return the OCI repository path for registry config.
///
/// Format: `{namespace}/bazel/_registry`
pub fn registry_config_repository(namespace: &str) -> String {
    format!("{namespace}/{BAZEL_PREFIX}/{REGISTRY_REPO_NAME}")
}

/// Parse an OCI reference into its components.
///
/// Expected format: `{registry}/{namespace}/bazel/{module}:{tag}`
///
/// The namespace may contain multiple path segments. When multiple `/bazel/`
/// segments exist, the last one is used (matching Go `strings.LastIndex` behavior).
pub fn parse_reference(reference: &str) -> Result<ParsedReference, ReferenceError> {
    // Split off tag (last colon)
    let tag_idx = reference
        .rfind(':')
        .ok_or_else(|| ReferenceError::NoTag(reference.to_owned()))?;
    let tag = &reference[tag_idx + 1..];
    if tag.is_empty() {
        return Err(ReferenceError::EmptyTag(reference.to_owned()));
    }
    let path = &reference[..tag_idx];

    // Find /bazel/ segment (last occurrence)
    let bazel_seg = format!("/{BAZEL_PREFIX}/");
    let bazel_idx = path
        .rfind(&bazel_seg)
        .ok_or_else(|| ReferenceError::NoBazelSegment(reference.to_owned()))?;

    let module = &path[bazel_idx + bazel_seg.len()..];
    if module.is_empty() {
        return Err(ReferenceError::EmptyModule(reference.to_owned()));
    }

    let prefix = &path[..bazel_idx];

    // Split prefix into registry and namespace at first '/'
    let slash_idx = prefix
        .find('/')
        .ok_or_else(|| ReferenceError::NoRegistry(reference.to_owned()))?;
    let registry = &prefix[..slash_idx];
    if registry.is_empty() {
        return Err(ReferenceError::NoRegistry(reference.to_owned()));
    }
    let namespace = &prefix[slash_idx + 1..];
    if namespace.is_empty() {
        return Err(ReferenceError::NoNamespace(reference.to_owned()));
    }

    Ok(ParsedReference {
        registry: registry.to_owned(),
        namespace: namespace.to_owned(),
        module: module.to_owned(),
        tag: tag.to_owned(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_module_reference() {
        assert_eq!(
            module_reference("ghcr.io", "myorg", "rules_go", "0.50.1"),
            "ghcr.io/myorg/bazel/rules_go:0.50.1"
        );
        assert_eq!(
            module_reference("localhost:5000", "test", "protobuf", "28.3"),
            "localhost:5000/test/bazel/protobuf:28.3"
        );
    }

    #[test]
    fn test_metadata_reference() {
        assert_eq!(
            metadata_reference("ghcr.io", "myorg", "rules_go"),
            "ghcr.io/myorg/bazel/rules_go:metadata"
        );
    }

    #[test]
    fn test_registry_config_reference() {
        assert_eq!(
            registry_config_reference("ghcr.io", "myorg"),
            "ghcr.io/myorg/bazel/_registry:config"
        );
    }

    #[test]
    fn test_module_repository() {
        assert_eq!(
            module_repository("myorg", "rules_go"),
            "myorg/bazel/rules_go"
        );
    }

    #[test]
    fn test_registry_config_repository() {
        assert_eq!(registry_config_repository("myorg"), "myorg/bazel/_registry");
    }

    // All ParseReference test cases ported from Go reference_test.go

    #[test]
    fn test_parse_module_version() {
        let r = parse_reference("ghcr.io/myorg/bazel/rules_go:0.50.1").unwrap();
        assert_eq!(r.registry, "ghcr.io");
        assert_eq!(r.namespace, "myorg");
        assert_eq!(r.module, "rules_go");
        assert_eq!(r.tag, "0.50.1");
    }

    #[test]
    fn test_parse_metadata_tag() {
        let r = parse_reference("ghcr.io/myorg/bazel/rules_go:metadata").unwrap();
        assert_eq!(r.tag, "metadata");
        assert!(r.is_metadata());
    }

    #[test]
    fn test_parse_registry_config() {
        let r = parse_reference("ghcr.io/myorg/bazel/_registry:config").unwrap();
        assert_eq!(r.module, "_registry");
        assert_eq!(r.tag, "config");
        assert!(r.is_registry_config());
    }

    #[test]
    fn test_parse_nested_namespace() {
        let r = parse_reference("localhost:5000/deep/nested/ns/bazel/protobuf:28.3").unwrap();
        assert_eq!(r.registry, "localhost:5000");
        assert_eq!(r.namespace, "deep/nested/ns");
        assert_eq!(r.module, "protobuf");
        assert_eq!(r.tag, "28.3");
    }

    #[test]
    fn test_parse_no_tag() {
        assert!(parse_reference("no-tag-ref").is_err());
    }

    #[test]
    fn test_parse_empty_tag() {
        assert!(parse_reference("ghcr.io/myorg/bazel/rules_go:").is_err());
    }

    #[test]
    fn test_parse_no_bazel_segment() {
        assert!(parse_reference("registry/no-bazel-segment:tag").is_err());
    }

    #[test]
    fn test_parse_port_in_registry() {
        let r = parse_reference("ghcr.io:443/myorg/bazel/rules_go:0.50.1").unwrap();
        assert_eq!(r.registry, "ghcr.io:443");
        assert_eq!(r.namespace, "myorg");
        assert_eq!(r.module, "rules_go");
        assert_eq!(r.tag, "0.50.1");
    }

    #[test]
    fn test_parse_module_name_with_dots() {
        let r = parse_reference("ghcr.io/myorg/bazel/rules_cc.experimental:1.0.0").unwrap();
        assert_eq!(r.module, "rules_cc.experimental");
        assert_eq!(r.tag, "1.0.0");
    }

    #[test]
    fn test_parse_single_char_segments() {
        let r = parse_reference("r/n/bazel/m:t").unwrap();
        assert_eq!(r.registry, "r");
        assert_eq!(r.namespace, "n");
        assert_eq!(r.module, "m");
        assert_eq!(r.tag, "t");
    }

    #[test]
    fn test_parse_tag_with_plus() {
        let r = parse_reference("ghcr.io/myorg/bazel/mod:1.0.0+build").unwrap();
        assert_eq!(r.tag, "1.0.0+build");
    }

    #[test]
    fn test_parse_multiple_bazel_segments() {
        let r = parse_reference("ghcr.io/ns/bazel/inner/bazel/mod:1.0").unwrap();
        assert_eq!(r.registry, "ghcr.io");
        assert_eq!(r.namespace, "ns/bazel/inner");
        assert_eq!(r.module, "mod");
        assert_eq!(r.tag, "1.0");
    }

    #[test]
    fn test_parse_empty_module_after_bazel() {
        assert!(parse_reference("ghcr.io/myorg/bazel/:1.0").is_err());
    }

    #[test]
    fn test_parse_no_namespace() {
        assert!(parse_reference("ghcr.io/bazel/mod:1.0").is_err());
    }

    #[test]
    fn test_parse_double_slash_empty_namespace() {
        assert!(parse_reference("ghcr.io//bazel/mod:1.0").is_err());
    }

    #[test]
    fn test_is_metadata() {
        let r = ParsedReference {
            registry: String::new(),
            namespace: String::new(),
            module: String::new(),
            tag: "metadata".to_owned(),
        };
        assert!(r.is_metadata());

        let r2 = ParsedReference {
            registry: String::new(),
            namespace: String::new(),
            module: String::new(),
            tag: "0.50.1".to_owned(),
        };
        assert!(!r2.is_metadata());
    }

    #[test]
    fn test_is_registry_config() {
        let r = ParsedReference {
            registry: String::new(),
            namespace: String::new(),
            module: "_registry".to_owned(),
            tag: "config".to_owned(),
        };
        assert!(r.is_registry_config());

        let r2 = ParsedReference {
            registry: String::new(),
            namespace: String::new(),
            module: "rules_go".to_owned(),
            tag: "0.50.1".to_owned(),
        };
        assert!(!r2.is_registry_config());
    }

    #[test]
    fn test_roundtrip() {
        let reference = module_reference("ghcr.io", "myorg", "rules_go", "0.50.1");
        let parsed = parse_reference(&reference).unwrap();
        assert_eq!(parsed.registry, "ghcr.io");
        assert_eq!(parsed.namespace, "myorg");
        assert_eq!(parsed.module, "rules_go");
        assert_eq!(parsed.tag, "0.50.1");
    }

    #[test]
    fn test_roundtrip_nested_namespace() {
        let reference = module_reference("ghcr.io", "org/team/sub", "rules_go", "1.0");
        let parsed = parse_reference(&reference).unwrap();
        assert_eq!(parsed.registry, "ghcr.io");
        assert_eq!(parsed.namespace, "org/team/sub");
        assert_eq!(parsed.module, "rules_go");
        assert_eq!(parsed.tag, "1.0");
    }

    #[test]
    fn test_roundtrip_metadata() {
        let reference = metadata_reference("ghcr.io", "ns", "rules_go");
        let parsed = parse_reference(&reference).unwrap();
        assert!(parsed.is_metadata());
        assert_eq!(parsed.module, "rules_go");
    }

    #[test]
    fn test_roundtrip_registry_config() {
        let reference = registry_config_reference("ghcr.io", "ns");
        let parsed = parse_reference(&reference).unwrap();
        assert!(parsed.is_registry_config());
    }

    #[test]
    fn test_is_registry_config_wrong_tag() {
        let r = ParsedReference {
            registry: "r".to_owned(),
            namespace: "n".to_owned(),
            module: "_registry".to_owned(),
            tag: "v1".to_owned(), // wrong tag
        };
        assert!(!r.is_registry_config());
    }

    #[test]
    fn test_is_registry_config_wrong_module() {
        let r = ParsedReference {
            registry: "r".to_owned(),
            namespace: "n".to_owned(),
            module: "not_registry".to_owned(),
            tag: "config".to_owned(),
        };
        assert!(!r.is_registry_config());
    }

    #[test]
    fn test_is_metadata_wrong_tag() {
        let r = ParsedReference {
            registry: "r".to_owned(),
            namespace: "n".to_owned(),
            module: "mod".to_owned(),
            tag: "not-metadata".to_owned(),
        };
        assert!(!r.is_metadata());
    }

    #[test]
    fn test_reference_builders_special_chars() {
        let ref_str = module_reference(
            "localhost:5000",
            "my/org/team",
            "rules_cc.experimental",
            "1.0.0+build.123",
        );
        assert_eq!(
            ref_str,
            "localhost:5000/my/org/team/bazel/rules_cc.experimental:1.0.0+build.123"
        );
        let parsed = parse_reference(&ref_str).unwrap();
        assert_eq!(parsed.registry, "localhost:5000");
        assert_eq!(parsed.namespace, "my/org/team");
        assert_eq!(parsed.module, "rules_cc.experimental");
        assert_eq!(parsed.tag, "1.0.0+build.123");
    }

    #[test]
    fn test_parse_error_variants_are_descriptive() {
        // NoTag
        let err = parse_reference("no-colon").unwrap_err();
        assert!(err.to_string().contains("no tag"));

        // EmptyTag
        let err = parse_reference("ghcr.io/ns/bazel/mod:").unwrap_err();
        assert!(err.to_string().contains("empty tag"));

        // NoBazelSegment
        let err = parse_reference("reg/ns/mod:tag").unwrap_err();
        assert!(err.to_string().contains("/bazel/"));

        // EmptyModule
        let err = parse_reference("reg/ns/bazel/:tag").unwrap_err();
        assert!(err.to_string().contains("empty module"));

        // NoRegistry -- prefix before /bazel/ has no slash
        let err = parse_reference("reg/bazel/mod:tag").unwrap_err();
        assert!(err.to_string().contains("no registry"));

        // NoNamespace -- "ghcr.io//bazel/mod:tag" → prefix is "ghcr.io/" → namespace is ""
        let err = parse_reference("ghcr.io//bazel/mod:tag").unwrap_err();
        assert!(err.to_string().contains("no namespace"));
    }
}
