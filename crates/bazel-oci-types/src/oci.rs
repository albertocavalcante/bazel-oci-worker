use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// OCI content descriptor.
///
/// Field names use camelCase to match the OCI spec JSON format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Descriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
    pub size: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

/// OCI image manifest.
///
/// Field names use camelCase to match the OCI spec JSON format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OciManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: i32,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    #[serde(
        rename = "artifactType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub artifact_type: Option<String>,
    pub config: Descriptor,
    pub layers: Vec<Descriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_camel_case_field_names() {
        let desc = Descriptor {
            media_type: "application/octet-stream".to_owned(),
            digest: "sha256:abcdef".to_owned(),
            size: 1234,
            annotations: None,
        };
        let json = serde_json::to_string(&desc).unwrap();
        // Exact JSON -- no annotations key when None
        assert_eq!(
            json,
            r#"{"mediaType":"application/octet-stream","digest":"sha256:abcdef","size":1234}"#
        );
    }

    #[test]
    fn descriptor_roundtrip_preserves_all_fields() {
        let desc = Descriptor {
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_owned(),
            digest: "sha256:abcdef".to_owned(),
            size: 1234,
            annotations: None,
        };
        let json = serde_json::to_string(&desc).unwrap();
        let parsed: Descriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(desc, parsed);
    }

    #[test]
    fn descriptor_zero_size() {
        let desc = Descriptor {
            media_type: "application/octet-stream".to_owned(),
            digest: "sha256:e3b0c44298fc1c149afbf4c8996fb924".to_owned(),
            size: 0,
            annotations: None,
        };
        let json = serde_json::to_string(&desc).unwrap();
        assert!(json.contains("\"size\":0"));
        let parsed: Descriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.size, 0);
    }

    #[test]
    fn descriptor_with_annotations_roundtrip() {
        let desc = Descriptor {
            media_type: "text/plain".to_owned(),
            digest: "sha256:abc".to_owned(),
            size: 10,
            annotations: Some(HashMap::from([
                (
                    "org.opencontainers.image.title".to_owned(),
                    "MODULE.bazel".to_owned(),
                ),
                ("custom.key".to_owned(), "custom.value".to_owned()),
            ])),
        };
        let json = serde_json::to_string(&desc).unwrap();
        let parsed: Descriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(desc, parsed);

        let ann = parsed.annotations.as_ref().unwrap();
        assert_eq!(
            ann.get("org.opencontainers.image.title").unwrap(),
            "MODULE.bazel"
        );
        assert_eq!(ann.get("custom.key").unwrap(), "custom.value");
    }

    #[test]
    fn descriptor_parses_from_go_json() {
        let go_json = r#"{"mediaType":"application/vnd.bazel.module.file.v1","digest":"sha256:def456","size":100,"annotations":{"org.opencontainers.image.title":"MODULE.bazel"}}"#;
        let desc: Descriptor = serde_json::from_str(go_json).unwrap();
        assert_eq!(desc.media_type, "application/vnd.bazel.module.file.v1");
        assert_eq!(desc.digest, "sha256:def456");
        assert_eq!(desc.size, 100);
        assert_eq!(
            desc.annotations
                .unwrap()
                .get("org.opencontainers.image.title")
                .unwrap(),
            "MODULE.bazel"
        );
    }

    #[test]
    fn manifest_camel_case_field_names() {
        let manifest = OciManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".to_owned(),
            artifact_type: Some("application/vnd.bazel.module.v1+json".to_owned()),
            config: Descriptor {
                media_type: "application/vnd.bazel.module.config.v1+json".to_owned(),
                digest: "sha256:abc123".to_owned(),
                size: 42,
                annotations: None,
            },
            layers: vec![],
            annotations: None,
        };
        let json = serde_json::to_string(&manifest).unwrap();

        // Exact key verification via deserializing to generic Value
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["schemaVersion"], 2);
        assert!(value.get("schema_version").is_none());
        assert_eq!(
            value["mediaType"],
            "application/vnd.oci.image.manifest.v1+json"
        );
        assert_eq!(
            value["artifactType"],
            "application/vnd.bazel.module.v1+json"
        );
        assert!(value.get("artifact_type").is_none());
    }

    #[test]
    fn manifest_roundtrip_with_layers() {
        let manifest = OciManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".to_owned(),
            artifact_type: Some("application/vnd.bazel.module.v1+json".to_owned()),
            config: Descriptor {
                media_type: "application/vnd.bazel.module.config.v1+json".to_owned(),
                digest: "sha256:abc123".to_owned(),
                size: 42,
                annotations: None,
            },
            layers: vec![Descriptor {
                media_type: "application/vnd.bazel.module.file.v1".to_owned(),
                digest: "sha256:def456".to_owned(),
                size: 100,
                annotations: Some(HashMap::from([(
                    "org.opencontainers.image.title".to_owned(),
                    "MODULE.bazel".to_owned(),
                )])),
            }],
            annotations: Some(HashMap::from([(
                "dev.bazel.module.name".to_owned(),
                "rules_go".to_owned(),
            )])),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: OciManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, parsed);
    }

    #[test]
    fn manifest_omits_optional_fields_when_none() {
        let manifest = OciManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".to_owned(),
            artifact_type: None,
            config: Descriptor {
                media_type: "application/vnd.bazel.module.config.v1+json".to_owned(),
                digest: "sha256:abc".to_owned(),
                size: 2,
                annotations: None,
            },
            layers: vec![],
            annotations: None,
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("artifactType").is_none());
        assert!(value.get("annotations").is_none());
    }

    #[test]
    fn manifest_empty_layers_serializes_as_array() {
        let manifest = OciManifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".to_owned(),
            artifact_type: None,
            config: Descriptor {
                media_type: "text/plain".to_owned(),
                digest: "sha256:x".to_owned(),
                size: 0,
                annotations: None,
            },
            layers: vec![],
            annotations: None,
        };
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert!(value["layers"].is_array());
        assert_eq!(value["layers"].as_array().unwrap().len(), 0);
    }
}
