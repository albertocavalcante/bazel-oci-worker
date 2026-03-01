pub mod bcr;
pub mod error;
pub mod keys;
pub mod memory;
pub mod oci;
pub mod storage;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::cast_possible_wrap)]
mod tests {
    use std::collections::HashMap;

    use bazel_oci_types::annotations::{
        ANNOTATION_COMPATIBILITY_LEVEL, ANNOTATION_CREATED, ANNOTATION_MODULE_NAME,
        ANNOTATION_MODULE_VERSION, ANNOTATION_TITLE,
    };
    use bazel_oci_types::config::ModuleConfig;
    use bazel_oci_types::digest::sha256_digest;
    use bazel_oci_types::media_types::{
        MEDIA_TYPE_CONFIG, MEDIA_TYPE_METADATA, MEDIA_TYPE_MODULE_FILE, MEDIA_TYPE_OCI_MANIFEST,
        MEDIA_TYPE_OVERLAY, MEDIA_TYPE_PATCH, MEDIA_TYPE_SOURCE_JSON,
    };
    use bazel_oci_types::oci::{Descriptor, OciManifest};

    use crate::memory::InMemoryStorage;
    use crate::oci::{blobs, catalog, manifests, uploads};
    use crate::storage::Storage;

    fn make_descriptor(media_type: &str, data: &[u8], title: Option<&str>) -> Descriptor {
        let mut annotations =
            title.map(|t| HashMap::from([(ANNOTATION_TITLE.to_owned(), t.to_owned())]));
        if annotations.as_ref().is_some_and(HashMap::is_empty) {
            annotations = None;
        }
        Descriptor {
            media_type: media_type.to_owned(),
            digest: sha256_digest(data),
            size: data.len() as i64,
            annotations,
        }
    }

    /// Helper: store a blob and return its descriptor.
    async fn push_blob(
        storage: &dyn Storage,
        name: &str,
        media_type: &str,
        data: &[u8],
        title: Option<&str>,
    ) -> Descriptor {
        let digest = sha256_digest(data);
        blobs::store_blob(storage, name, &digest, data, None)
            .await
            .unwrap();
        make_descriptor(media_type, data, title)
    }

    /// Helper: build + store a full module version manifest via OCI APIs.
    async fn push_module(storage: &InMemoryStorage, namespace: &str, module: &str, version: &str) {
        let repo = bazel_oci_types::reference::module_repository(namespace, module);

        // Config blob
        let config = ModuleConfig {
            name: module.to_owned(),
            version: version.to_owned(),
            compatibility_level: 0,
            homepage: None,
            repository: None,
            yanked: None,
            yank_reason: None,
        };
        let config_data = serde_json::to_vec(&config).unwrap();
        let config_desc = push_blob(storage, &repo, MEDIA_TYPE_CONFIG, &config_data, None).await;

        // MODULE.bazel layer
        let module_bazel = format!("module(name = \"{module}\", version = \"{version}\")");
        let module_desc = push_blob(
            storage,
            &repo,
            MEDIA_TYPE_MODULE_FILE,
            module_bazel.as_bytes(),
            Some("MODULE.bazel"),
        )
        .await;

        // source.json layer
        let source_json = br#"{"type":"archive","url":"https://example.com/v1.tar.gz","integrity":"sha256-abc","strip_prefix":"mod-1.0"}"#;
        let source_desc = push_blob(
            storage,
            &repo,
            MEDIA_TYPE_SOURCE_JSON,
            source_json,
            Some("source.json"),
        )
        .await;

        // A patch layer
        let patch_data = b"--- a/file\n+++ b/file\n@@ -1 +1 @@\n-old\n+new\n";
        let patch_desc = push_blob(
            storage,
            &repo,
            MEDIA_TYPE_PATCH,
            patch_data,
            Some("patches/fix.patch"),
        )
        .await;

        // An overlay layer
        let overlay_data = b"# BUILD.bazel overlay\ncc_library(name = \"lib\")";
        let overlay_desc = push_blob(
            storage,
            &repo,
            MEDIA_TYPE_OVERLAY,
            overlay_data,
            Some("overlay/BUILD.bazel"),
        )
        .await;

        // Build manifest
        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: Some(bazel_oci_types::media_types::ARTIFACT_TYPE_MODULE.to_owned()),
            config: config_desc,
            layers: vec![module_desc, source_desc, patch_desc, overlay_desc],
            annotations: Some(HashMap::from([
                (
                    ANNOTATION_CREATED.to_owned(),
                    "2025-01-01T00:00:00Z".to_owned(),
                ),
                (ANNOTATION_MODULE_NAME.to_owned(), module.to_owned()),
                (ANNOTATION_MODULE_VERSION.to_owned(), version.to_owned()),
                (ANNOTATION_COMPATIBILITY_LEVEL.to_owned(), "0".to_owned()),
            ])),
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(storage, &repo, version, &manifest_json)
            .await
            .unwrap();
    }

    /// Helper: build + store a metadata manifest.
    async fn push_metadata(
        storage: &InMemoryStorage,
        namespace: &str,
        module: &str,
        versions: &[&str],
    ) {
        let repo = bazel_oci_types::reference::module_repository(namespace, module);

        // Empty config blob (metadata manifests use "{}" as config)
        let config_data = b"{}";
        let config_desc = push_blob(storage, &repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        // Metadata layer
        let metadata = bazel_oci_types::bcr::Metadata {
            versions: versions.iter().map(|v| (*v).to_owned()).collect(),
            yanked_versions: None,
            maintainers: None,
            homepage: None,
            repository: None,
        };
        let metadata_json = serde_json::to_vec(&metadata).unwrap();
        let metadata_desc = push_blob(
            storage,
            &repo,
            MEDIA_TYPE_METADATA,
            &metadata_json,
            Some("metadata.json"),
        )
        .await;

        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![metadata_desc],
            annotations: Some(HashMap::from([(
                ANNOTATION_MODULE_NAME.to_owned(),
                module.to_owned(),
            )])),
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(storage, &repo, "metadata", &manifest_json)
            .await
            .unwrap();
    }

    // === OCI Blob tests ===

    #[tokio::test]
    async fn test_blob_upload_and_get() {
        let storage = InMemoryStorage::new();
        let data = b"hello world";
        let digest = sha256_digest(data);

        // Upload via monolithic
        let result = uploads::monolithic_upload(&storage, "test/repo", &digest, data)
            .await
            .unwrap();
        assert_eq!(result, digest);

        // GET
        let got = blobs::get_blob(&storage, "test/repo", &digest)
            .await
            .unwrap();
        assert_eq!(got, data);

        // HEAD
        let size = blobs::head_blob(&storage, "test/repo", &digest)
            .await
            .unwrap();
        assert_eq!(size, data.len() as u64);
    }

    #[tokio::test]
    async fn test_blob_chunked_upload() {
        let storage = InMemoryStorage::new();
        let uuid = "test-uuid-123";

        uploads::initiate_upload(&storage, "test/repo", uuid)
            .await
            .unwrap();

        let chunk1 = b"hello ";
        let offset = uploads::upload_chunk(&storage, uuid, chunk1).await.unwrap();
        assert_eq!(offset, 6);

        let chunk2 = b"world";
        let offset = uploads::upload_chunk(&storage, uuid, chunk2).await.unwrap();
        assert_eq!(offset, 11);

        let full_data = b"hello world";
        let expected_digest = sha256_digest(full_data);

        let digest = uploads::complete_upload(&storage, "test/repo", uuid, &expected_digest, None)
            .await
            .unwrap();
        assert_eq!(digest, expected_digest);

        // Verify blob is stored
        let got = blobs::get_blob(&storage, "test/repo", &digest)
            .await
            .unwrap();
        assert_eq!(got, full_data);
    }

    #[tokio::test]
    async fn test_blob_delete() {
        let storage = InMemoryStorage::new();
        let data = b"test";
        let digest = sha256_digest(data);

        uploads::monolithic_upload(&storage, "test/repo", &digest, data)
            .await
            .unwrap();
        blobs::delete_blob(&storage, "test/repo", &digest)
            .await
            .unwrap();

        let result = blobs::get_blob(&storage, "test/repo", &digest).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_blob_digest_mismatch() {
        let storage = InMemoryStorage::new();
        let result =
            uploads::monolithic_upload(&storage, "test/repo", "sha256:wrong", b"data").await;
        assert!(result.is_err());
    }

    // === OCI Manifest tests ===

    #[tokio::test]
    async fn test_manifest_put_get() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let repo = "ns/bazel/rules_go";

        // GET by tag
        let (data, digest) = manifests::get_manifest(&storage, repo, "0.50.1")
            .await
            .unwrap();
        assert!(!data.is_empty());
        assert!(digest.starts_with("sha256:"));

        // GET by digest
        let (data2, _) = manifests::get_manifest(&storage, repo, &digest)
            .await
            .unwrap();
        assert_eq!(data, data2);

        // HEAD
        let (size, head_digest) = manifests::head_manifest(&storage, repo, "0.50.1")
            .await
            .unwrap();
        assert_eq!(size, data.len() as u64);
        assert_eq!(head_digest, digest);
    }

    #[tokio::test]
    async fn test_manifest_delete() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let repo = "ns/bazel/rules_go";
        manifests::delete_manifest(&storage, repo, "0.50.1")
            .await
            .unwrap();

        let result = manifests::get_manifest(&storage, repo, "0.50.1").await;
        assert!(result.is_err());
    }

    // === Catalog + Tags tests ===

    #[tokio::test]
    async fn test_tags_list() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;
        push_module(&storage, "ns", "rules_go", "0.51.0").await;

        let result = catalog::tags_list(&storage, "ns/bazel/rules_go")
            .await
            .unwrap();
        assert_eq!(result.name, "ns/bazel/rules_go");
        assert!(result.tags.contains(&"0.50.1".to_owned()));
        assert!(result.tags.contains(&"0.51.0".to_owned()));
    }

    #[tokio::test]
    async fn test_catalog() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;
        push_module(&storage, "ns", "protobuf", "28.3").await;

        let result = catalog::catalog(&storage).await.unwrap();
        assert!(
            result
                .repositories
                .contains(&"ns/bazel/rules_go".to_owned())
        );
        assert!(
            result
                .repositories
                .contains(&"ns/bazel/protobuf".to_owned())
        );
    }

    // === BCR endpoint tests ===

    #[tokio::test]
    async fn test_bcr_registry_json() {
        let data = crate::bcr::registry_json::registry_json();
        assert_eq!(data, b"{}");
    }

    #[tokio::test]
    async fn test_bcr_metadata() {
        let storage = InMemoryStorage::new();
        push_metadata(&storage, "ns", "rules_go", &["0.50.1", "0.51.0"]).await;

        let data = crate::bcr::metadata::get_metadata(&storage, "ns", "rules_go")
            .await
            .unwrap();
        let meta: bazel_oci_types::bcr::Metadata = serde_json::from_slice(&data).unwrap();
        assert_eq!(meta.versions, vec!["0.50.1", "0.51.0"]);
    }

    #[tokio::test]
    async fn test_bcr_module_bazel() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let data = crate::bcr::module_bazel::get_module_bazel(&storage, "ns", "rules_go", "0.50.1")
            .await
            .unwrap();
        let content = String::from_utf8(data).unwrap();
        assert!(content.contains("rules_go"));
        assert!(content.contains("0.50.1"));
    }

    #[tokio::test]
    async fn test_bcr_source_json() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let data = crate::bcr::source_json::get_source_json(&storage, "ns", "rules_go", "0.50.1")
            .await
            .unwrap();
        let source: bazel_oci_types::bcr::Source = serde_json::from_slice(&data).unwrap();
        assert_eq!(source.source_type.as_deref(), Some("archive"));
    }

    #[tokio::test]
    async fn test_bcr_patch() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let data =
            crate::bcr::patches::get_patch(&storage, "ns", "rules_go", "0.50.1", "fix.patch")
                .await
                .unwrap();
        let content = String::from_utf8(data).unwrap();
        assert!(content.contains("--- a/file"));
    }

    #[tokio::test]
    async fn test_bcr_overlay() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let data =
            crate::bcr::overlays::get_overlay(&storage, "ns", "rules_go", "0.50.1", "BUILD.bazel")
                .await
                .unwrap();
        let content = String::from_utf8(data).unwrap();
        assert!(content.contains("cc_library"));
    }

    #[tokio::test]
    async fn test_bcr_not_found() {
        let storage = InMemoryStorage::new();
        let result = crate::bcr::metadata::get_metadata(&storage, "ns", "nonexistent").await;
        assert!(result.is_err());
    }

    // === Additional error path tests ===

    #[tokio::test]
    async fn chunked_upload_digest_mismatch_on_complete() {
        let storage = InMemoryStorage::new();
        let uuid = "mismatch-uuid";

        uploads::initiate_upload(&storage, "test/repo", uuid)
            .await
            .unwrap();
        uploads::upload_chunk(&storage, uuid, b"some data")
            .await
            .unwrap();

        let result =
            uploads::complete_upload(&storage, "test/repo", uuid, "sha256:definitelywrong", None)
                .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn upload_cleanup_on_digest_mismatch() {
        let storage = InMemoryStorage::new();
        let uuid = "cleanup-test-uuid";

        uploads::initiate_upload(&storage, "test/repo", uuid)
            .await
            .unwrap();
        uploads::upload_chunk(&storage, uuid, b"some data")
            .await
            .unwrap();

        // Verify upload artifacts exist before the failed complete
        let data_key = crate::keys::upload_data(uuid);
        let meta_key = crate::keys::upload_meta(uuid);
        assert!(storage.get(&data_key).await.unwrap().is_some());
        assert!(storage.get(&meta_key).await.unwrap().is_some());

        let result =
            uploads::complete_upload(&storage, "test/repo", uuid, "sha256:definitelywrong", None)
                .await;
        assert!(result.is_err());

        // Verify upload artifacts are cleaned up even on error
        assert!(storage.get(&data_key).await.unwrap().is_none());
        assert!(storage.get(&meta_key).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn upload_chunk_to_nonexistent_session_fails() {
        let storage = InMemoryStorage::new();
        let result = uploads::upload_chunk(&storage, "no-such-uuid", b"data").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_manifest_nonexistent_returns_error() {
        let storage = InMemoryStorage::new();
        let result = manifests::get_manifest(&storage, "no/repo", "no-tag").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn head_manifest_nonexistent_returns_error() {
        let storage = InMemoryStorage::new();
        let result = manifests::head_manifest(&storage, "no/repo", "no-tag").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_blob_nonexistent_returns_error() {
        let storage = InMemoryStorage::new();
        let result = blobs::get_blob(&storage, "no/repo", "sha256:deadbeef").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn head_blob_nonexistent_returns_error() {
        let storage = InMemoryStorage::new();
        let result = blobs::head_blob(&storage, "no/repo", "sha256:deadbeef").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_digest_format_returns_error() {
        let storage = InMemoryStorage::new();
        let result = blobs::get_blob(&storage, "test/repo", "not-a-digest").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn empty_catalog_returns_empty_list() {
        let storage = InMemoryStorage::new();
        let result = catalog::catalog(&storage).await.unwrap();
        assert!(result.repositories.is_empty());
    }

    #[tokio::test]
    async fn tags_list_empty_repo_returns_empty() {
        let storage = InMemoryStorage::new();
        let result = catalog::tags_list(&storage, "empty/repo").await.unwrap();
        assert!(result.tags.is_empty());
        assert_eq!(result.name, "empty/repo");
    }

    #[tokio::test]
    async fn tags_list_excludes_digest_references() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let result = catalog::tags_list(&storage, "ns/bazel/rules_go")
            .await
            .unwrap();
        // Should contain "0.50.1" but not any "sha256:..." entries
        assert!(result.tags.contains(&"0.50.1".to_owned()));
        assert!(!result.tags.iter().any(|t| t.starts_with("sha256:")));
    }

    #[tokio::test]
    async fn bcr_patch_not_found_for_missing_file() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let result = crate::bcr::patches::get_patch(
            &storage,
            "ns",
            "rules_go",
            "0.50.1",
            "nonexistent.patch",
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bcr_overlay_not_found_for_missing_file() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let result = crate::bcr::overlays::get_overlay(
            &storage,
            "ns",
            "rules_go",
            "0.50.1",
            "nonexistent.bzl",
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bcr_module_bazel_not_found_for_missing_version() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let result =
            crate::bcr::module_bazel::get_module_bazel(&storage, "ns", "rules_go", "9.9.9").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bcr_source_json_not_found_for_missing_version() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;

        let result =
            crate::bcr::source_json::get_source_json(&storage, "ns", "rules_go", "9.9.9").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn put_manifest_rejects_invalid_json() {
        let storage = InMemoryStorage::new();
        let result =
            manifests::put_manifest(&storage, "test/repo", "latest", b"not valid json").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn put_manifest_rejects_schema_version_not_2() {
        let storage = InMemoryStorage::new();
        let manifest = OciManifest {
            schema_version: 1,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: Descriptor {
                media_type: "text/plain".to_owned(),
                digest: "sha256:abc".to_owned(),
                size: 2,
                annotations: None,
            },
            layers: vec![],
            annotations: None,
        };
        let body = serde_json::to_vec(&manifest).unwrap();
        let result = manifests::put_manifest(&storage, "test/repo", "latest", &body).await;
        assert!(result.is_err());
    }

    // === Upload: complete_upload with final chunk ===

    #[tokio::test]
    async fn complete_upload_with_final_chunk() {
        let storage = InMemoryStorage::new();
        let uuid = "final-chunk-uuid";

        uploads::initiate_upload(&storage, "test/repo", uuid)
            .await
            .unwrap();
        uploads::upload_chunk(&storage, uuid, b"hello ")
            .await
            .unwrap();

        let full_data = b"hello world";
        let expected_digest = sha256_digest(full_data);

        let digest = uploads::complete_upload(
            &storage,
            "test/repo",
            uuid,
            &expected_digest,
            Some(b"world"),
        )
        .await
        .unwrap();
        assert_eq!(digest, expected_digest);

        let got = blobs::get_blob(&storage, "test/repo", &digest)
            .await
            .unwrap();
        assert_eq!(got, full_data);
    }

    #[tokio::test]
    async fn complete_upload_no_prior_chunks_only_final() {
        let storage = InMemoryStorage::new();
        let uuid = "only-final-uuid";

        uploads::initiate_upload(&storage, "test/repo", uuid)
            .await
            .unwrap();

        // Skip all chunks, provide entire body as final chunk
        let data = b"entire blob in final chunk";
        let expected_digest = sha256_digest(data);

        let digest = uploads::complete_upload(
            &storage,
            "test/repo",
            uuid,
            &expected_digest,
            Some(data.as_slice()),
        )
        .await
        .unwrap();
        assert_eq!(digest, expected_digest);

        let got = blobs::get_blob(&storage, "test/repo", &digest)
            .await
            .unwrap();
        assert_eq!(got, data);
    }

    #[tokio::test]
    async fn complete_upload_nonexistent_session_with_no_data() {
        let storage = InMemoryStorage::new();
        // No initiate_upload, so no _meta and no data key exist.
        // complete_upload calls storage.get() for data_key which returns None → unwrap_or_default()
        // Then computes digest of empty data, which won't match "sha256:wrong".
        let result =
            uploads::complete_upload(&storage, "test/repo", "no-session", "sha256:wrong", None)
                .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn upload_chunk_with_corrupted_metadata() {
        let storage = InMemoryStorage::new();
        // Manually store invalid JSON as upload metadata
        storage
            .put(
                "_uploads/corrupt-uuid/_meta",
                b"not valid json",
                Some("application/json"),
            )
            .await
            .unwrap();

        let result = uploads::upload_chunk(&storage, "corrupt-uuid", b"data").await;
        assert!(result.is_err());
    }

    // === Manifest: dedup when reference is a digest ===

    #[tokio::test]
    async fn put_manifest_with_digest_as_reference_no_duplicate() {
        let storage = InMemoryStorage::new();
        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: Descriptor {
                media_type: "text/plain".to_owned(),
                digest: "sha256:abc".to_owned(),
                size: 2,
                annotations: None,
            },
            layers: vec![],
            annotations: None,
        };
        let body = serde_json::to_vec(&manifest).unwrap();
        let digest = sha256_digest(&body);

        // Use the digest itself as the reference
        let result = manifests::put_manifest(&storage, "test/repo", &digest, &body)
            .await
            .unwrap();
        assert_eq!(result, digest);

        // Should only have one key (not duplicated)
        let keys = storage.list_keys("test/repo/manifests/").await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], format!("test/repo/manifests/{digest}"));
    }

    #[tokio::test]
    async fn delete_nonexistent_manifest_succeeds() {
        let storage = InMemoryStorage::new();
        let result = manifests::delete_manifest(&storage, "no/repo", "no-tag").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn manifest_stored_at_both_tag_and_digest_keys() {
        let storage = InMemoryStorage::new();
        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: Descriptor {
                media_type: "text/plain".to_owned(),
                digest: "sha256:abc".to_owned(),
                size: 2,
                annotations: None,
            },
            layers: vec![],
            annotations: None,
        };
        let body = serde_json::to_vec(&manifest).unwrap();
        let digest = manifests::put_manifest(&storage, "test/repo", "v1.0", &body)
            .await
            .unwrap();

        // Must have two keys: one for tag, one for digest
        let keys = storage.list_keys("test/repo/manifests/").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"test/repo/manifests/v1.0".to_owned()));
        assert!(keys.contains(&format!("test/repo/manifests/{digest}")));

        // Both return identical data
        let (by_tag, tag_digest) = manifests::get_manifest(&storage, "test/repo", "v1.0")
            .await
            .unwrap();
        let (by_digest, digest_digest) = manifests::get_manifest(&storage, "test/repo", &digest)
            .await
            .unwrap();
        assert_eq!(by_tag, by_digest);
        assert_eq!(tag_digest, digest_digest);
    }

    // === Blob: empty blob lifecycle ===

    #[tokio::test]
    async fn empty_blob_lifecycle() {
        let storage = InMemoryStorage::new();
        let empty_digest = sha256_digest(b"");

        uploads::monolithic_upload(&storage, "test/repo", &empty_digest, b"")
            .await
            .unwrap();

        let got = blobs::get_blob(&storage, "test/repo", &empty_digest)
            .await
            .unwrap();
        assert!(got.is_empty());

        let size = blobs::head_blob(&storage, "test/repo", &empty_digest)
            .await
            .unwrap();
        assert_eq!(size, 0);

        blobs::delete_blob(&storage, "test/repo", &empty_digest)
            .await
            .unwrap();
        assert!(
            blobs::get_blob(&storage, "test/repo", &empty_digest)
                .await
                .is_err()
        );
    }

    // === Blob: invalid digest in various operations ===

    #[tokio::test]
    async fn blob_operations_reject_invalid_digests() {
        let storage = InMemoryStorage::new();
        let invalid_digests = &["no-colon", ":empty-algo", "sha256:", ":", ""];

        for &bad in invalid_digests {
            assert!(
                blobs::get_blob(&storage, "repo", bad).await.is_err(),
                "get_blob should reject {bad:?}"
            );
            assert!(
                blobs::head_blob(&storage, "repo", bad).await.is_err(),
                "head_blob should reject {bad:?}"
            );
            assert!(
                blobs::delete_blob(&storage, "repo", bad).await.is_err(),
                "delete_blob should reject {bad:?}"
            );
            assert!(
                blobs::store_blob(&storage, "repo", bad, b"data", None)
                    .await
                    .is_err(),
                "store_blob should reject {bad:?}"
            );
        }
    }

    // === Blob: store with content_type ===

    #[tokio::test]
    async fn store_blob_preserves_content_type() {
        let storage = InMemoryStorage::new();
        let data = b"json data";
        let digest = sha256_digest(data);

        blobs::store_blob(
            &storage,
            "test/repo",
            &digest,
            data,
            Some("application/json"),
        )
        .await
        .unwrap();

        // Verify we can retrieve it
        let got = blobs::get_blob(&storage, "test/repo", &digest)
            .await
            .unwrap();
        assert_eq!(got, data);

        // Verify content_type via head
        let (algo, hex) = bazel_oci_types::digest::parse_digest(&digest).unwrap();
        let meta = storage
            .head(&format!("test/repo/blobs/{algo}/{hex}"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(meta.content_type.as_deref(), Some("application/json"));
    }

    // === Catalog: ignores non-manifest keys ===

    #[tokio::test]
    async fn catalog_ignores_non_manifest_keys() {
        let storage = InMemoryStorage::new();

        // Store blob and upload keys that should NOT appear in catalog
        storage
            .put("repo/blobs/sha256/abc", b"data", None)
            .await
            .unwrap();
        storage
            .put("_uploads/uuid/_meta", b"data", None)
            .await
            .unwrap();
        storage.put("orphaned-key", b"data", None).await.unwrap();

        let result = catalog::catalog(&storage).await.unwrap();
        assert!(result.repositories.is_empty());
    }

    #[tokio::test]
    async fn catalog_deduplicates_repos_with_multiple_tags() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;
        push_module(&storage, "ns", "rules_go", "0.51.0").await;
        push_metadata(&storage, "ns", "rules_go", &["0.50.1", "0.51.0"]).await;

        let result = catalog::catalog(&storage).await.unwrap();
        // "ns/bazel/rules_go" should appear exactly once despite multiple tags + digest keys
        let count = result
            .repositories
            .iter()
            .filter(|r| r.as_str() == "ns/bazel/rules_go")
            .count();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn catalog_separates_prefix_similar_repos() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "foo", "1.0").await;
        push_module(&storage, "ns", "foo_bar", "1.0").await;

        let result = catalog::catalog(&storage).await.unwrap();
        assert!(result.repositories.contains(&"ns/bazel/foo".to_owned()));
        assert!(result.repositories.contains(&"ns/bazel/foo_bar".to_owned()));
        assert_eq!(result.repositories.len(), 2);
    }

    #[tokio::test]
    async fn tags_list_multiple_tags_and_digests_mixed() {
        let storage = InMemoryStorage::new();
        push_module(&storage, "ns", "rules_go", "0.50.1").await;
        push_module(&storage, "ns", "rules_go", "0.51.0").await;
        push_metadata(&storage, "ns", "rules_go", &["0.50.1", "0.51.0"]).await;

        let result = catalog::tags_list(&storage, "ns/bazel/rules_go")
            .await
            .unwrap();
        // Tags should include version tags and "metadata"
        assert!(result.tags.contains(&"0.50.1".to_owned()));
        assert!(result.tags.contains(&"0.51.0".to_owned()));
        assert!(result.tags.contains(&"metadata".to_owned()));
        // No sha256: digest references
        assert!(!result.tags.iter().any(|t| t.starts_with("sha256:")));
    }

    // === BCR: missing layer type in manifest ===

    #[tokio::test]
    async fn bcr_metadata_manifest_missing_metadata_layer() {
        let storage = InMemoryStorage::new();
        let repo = "ns/bazel/rules_go";

        // Create a manifest with no MEDIA_TYPE_METADATA layer (only a module file)
        let module_data = b"module(name = \"rules_go\")";
        let module_desc =
            push_blob(&storage, repo, MEDIA_TYPE_MODULE_FILE, module_data, None).await;
        let config_data = b"{}";
        let config_desc = push_blob(&storage, repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![module_desc], // No metadata layer
            annotations: None,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(&storage, repo, "metadata", &manifest_json)
            .await
            .unwrap();

        let result = crate::bcr::metadata::get_metadata(&storage, "ns", "rules_go").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bcr_module_bazel_manifest_missing_module_layer() {
        let storage = InMemoryStorage::new();
        let repo = "ns/bazel/rules_go";

        // Create a manifest with no MEDIA_TYPE_MODULE_FILE layer (only source.json)
        let source_data = br#"{"type":"archive"}"#;
        let source_desc =
            push_blob(&storage, repo, MEDIA_TYPE_SOURCE_JSON, source_data, None).await;
        let config_data = b"{}";
        let config_desc = push_blob(&storage, repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![source_desc], // No module file layer
            annotations: None,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(&storage, repo, "1.0.0", &manifest_json)
            .await
            .unwrap();

        let result =
            crate::bcr::module_bazel::get_module_bazel(&storage, "ns", "rules_go", "1.0.0").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bcr_source_json_manifest_missing_source_layer() {
        let storage = InMemoryStorage::new();
        let repo = "ns/bazel/rules_go";

        // Create a manifest with no MEDIA_TYPE_SOURCE_JSON layer
        let module_data = b"module()";
        let module_desc =
            push_blob(&storage, repo, MEDIA_TYPE_MODULE_FILE, module_data, None).await;
        let config_data = b"{}";
        let config_desc = push_blob(&storage, repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![module_desc], // No source.json layer
            annotations: None,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(&storage, repo, "1.0.0", &manifest_json)
            .await
            .unwrap();

        let result =
            crate::bcr::source_json::get_source_json(&storage, "ns", "rules_go", "1.0.0").await;
        assert!(result.is_err());
    }

    // === BCR: layer blob missing from storage ===

    #[tokio::test]
    async fn bcr_layer_blob_missing_from_storage() {
        let storage = InMemoryStorage::new();
        let repo = "ns/bazel/rules_go";

        // Create a manifest that references a blob digest that doesn't exist in storage
        let config_data = b"{}";
        let config_desc = push_blob(&storage, repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        let phantom_digest = sha256_digest(b"this blob was never stored");
        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![Descriptor {
                media_type: MEDIA_TYPE_MODULE_FILE.to_owned(),
                digest: phantom_digest,
                size: 100,
                annotations: None,
            }],
            annotations: None,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(&storage, repo, "1.0.0", &manifest_json)
            .await
            .unwrap();

        let result =
            crate::bcr::module_bazel::get_module_bazel(&storage, "ns", "rules_go", "1.0.0").await;
        assert!(result.is_err());
    }

    // === BCR: layer with invalid digest format ===

    #[tokio::test]
    async fn bcr_layer_with_invalid_digest_format() {
        let storage = InMemoryStorage::new();
        let repo = "ns/bazel/rules_go";

        let config_data = b"{}";
        let config_desc = push_blob(&storage, repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![Descriptor {
                media_type: MEDIA_TYPE_MODULE_FILE.to_owned(),
                digest: "invalid-no-colon".to_owned(), // Bad digest format
                size: 10,
                annotations: None,
            }],
            annotations: None,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(&storage, repo, "1.0.0", &manifest_json)
            .await
            .unwrap();

        let result =
            crate::bcr::module_bazel::get_module_bazel(&storage, "ns", "rules_go", "1.0.0").await;
        assert!(result.is_err());
    }

    // === BCR: multiple patches/overlays ===

    #[tokio::test]
    async fn bcr_multiple_patches() {
        let storage = InMemoryStorage::new();
        let repo = "ns/bazel/rules_go";

        let config_data = b"{}";
        let config_desc = push_blob(&storage, repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        let patch1_data = b"--- a/file1\n+++ b/file1\n";
        let patch1_desc = push_blob(
            &storage,
            repo,
            MEDIA_TYPE_PATCH,
            patch1_data,
            Some("patches/fix1.patch"),
        )
        .await;

        let patch2_data = b"--- a/file2\n+++ b/file2\n";
        let patch2_desc = push_blob(
            &storage,
            repo,
            MEDIA_TYPE_PATCH,
            patch2_data,
            Some("patches/fix2.patch"),
        )
        .await;

        let module_data = b"module()";
        let module_desc = push_blob(
            &storage,
            repo,
            MEDIA_TYPE_MODULE_FILE,
            module_data,
            Some("MODULE.bazel"),
        )
        .await;

        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![module_desc, patch1_desc, patch2_desc],
            annotations: None,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(&storage, repo, "1.0.0", &manifest_json)
            .await
            .unwrap();

        let p1 = crate::bcr::patches::get_patch(&storage, "ns", "rules_go", "1.0.0", "fix1.patch")
            .await
            .unwrap();
        assert_eq!(p1, patch1_data);

        let p2 = crate::bcr::patches::get_patch(&storage, "ns", "rules_go", "1.0.0", "fix2.patch")
            .await
            .unwrap();
        assert_eq!(p2, patch2_data);

        // Non-existent patch still fails
        let missing =
            crate::bcr::patches::get_patch(&storage, "ns", "rules_go", "1.0.0", "nope.patch").await;
        assert!(missing.is_err());
    }

    #[tokio::test]
    async fn bcr_multiple_overlays() {
        let storage = InMemoryStorage::new();
        let repo = "ns/bazel/rules_go";

        let config_data = b"{}";
        let config_desc = push_blob(&storage, repo, MEDIA_TYPE_CONFIG, config_data, None).await;

        let overlay1_data = b"cc_library(name = \"a\")";
        let overlay1_desc = push_blob(
            &storage,
            repo,
            MEDIA_TYPE_OVERLAY,
            overlay1_data,
            Some("overlay/BUILD.bazel"),
        )
        .await;

        let overlay2_data = b"go_library(name = \"b\")";
        let overlay2_desc = push_blob(
            &storage,
            repo,
            MEDIA_TYPE_OVERLAY,
            overlay2_data,
            Some("overlay/sub/BUILD.bazel"),
        )
        .await;

        let module_data = b"module()";
        let module_desc = push_blob(
            &storage,
            repo,
            MEDIA_TYPE_MODULE_FILE,
            module_data,
            Some("MODULE.bazel"),
        )
        .await;

        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_owned(),
            artifact_type: None,
            config: config_desc,
            layers: vec![module_desc, overlay1_desc, overlay2_desc],
            annotations: None,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        manifests::put_manifest(&storage, repo, "1.0.0", &manifest_json)
            .await
            .unwrap();

        let o1 =
            crate::bcr::overlays::get_overlay(&storage, "ns", "rules_go", "1.0.0", "BUILD.bazel")
                .await
                .unwrap();
        assert_eq!(o1, overlay1_data);

        let o2 = crate::bcr::overlays::get_overlay(
            &storage,
            "ns",
            "rules_go",
            "1.0.0",
            "sub/BUILD.bazel",
        )
        .await
        .unwrap();
        assert_eq!(o2, overlay2_data);
    }

    // === BCR: manifest with corrupted JSON in storage ===

    #[tokio::test]
    async fn bcr_corrupted_manifest_in_storage() {
        let storage = InMemoryStorage::new();
        // Manually store invalid JSON as a manifest
        storage
            .put(
                "ns/bazel/rules_go/manifests/metadata",
                b"not json at all",
                None,
            )
            .await
            .unwrap();

        let result = crate::bcr::metadata::get_metadata(&storage, "ns", "rules_go").await;
        assert!(result.is_err());
    }

    // === Integration: push via OCI, read via BCR ===

    #[tokio::test]
    async fn test_full_roundtrip() {
        let storage = InMemoryStorage::new();

        // Push metadata + version via OCI handlers
        push_metadata(&storage, "myns", "rules_go", &["0.50.1"]).await;
        push_module(&storage, "myns", "rules_go", "0.50.1").await;

        // Read via BCR endpoints
        let meta_data = crate::bcr::metadata::get_metadata(&storage, "myns", "rules_go")
            .await
            .unwrap();
        let meta: bazel_oci_types::bcr::Metadata = serde_json::from_slice(&meta_data).unwrap();
        assert_eq!(meta.versions, vec!["0.50.1"]);

        let module_data =
            crate::bcr::module_bazel::get_module_bazel(&storage, "myns", "rules_go", "0.50.1")
                .await
                .unwrap();
        assert!(String::from_utf8(module_data).unwrap().contains("rules_go"));

        let source_data =
            crate::bcr::source_json::get_source_json(&storage, "myns", "rules_go", "0.50.1")
                .await
                .unwrap();
        assert!(String::from_utf8(source_data).unwrap().contains("archive"));

        let patch_data =
            crate::bcr::patches::get_patch(&storage, "myns", "rules_go", "0.50.1", "fix.patch")
                .await
                .unwrap();
        assert!(!patch_data.is_empty());

        let overlay_data = crate::bcr::overlays::get_overlay(
            &storage,
            "myns",
            "rules_go",
            "0.50.1",
            "BUILD.bazel",
        )
        .await
        .unwrap();
        assert!(!overlay_data.is_empty());

        // Verify catalog sees the repo
        let cat = crate::oci::catalog::catalog(&storage).await.unwrap();
        assert!(cat.repositories.contains(&"myns/bazel/rules_go".to_owned()));

        // Verify tags list
        let tags = crate::oci::catalog::tags_list(&storage, "myns/bazel/rules_go")
            .await
            .unwrap();
        assert!(tags.tags.contains(&"0.50.1".to_owned()));
        assert!(tags.tags.contains(&"metadata".to_owned()));
    }
}
