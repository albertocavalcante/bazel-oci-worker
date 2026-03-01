use bazel_oci_types::digest::sha256_digest;
use bazel_oci_types::media_types::MEDIA_TYPE_OCI_MANIFEST;
use bazel_oci_types::oci::OciManifest;

use crate::error::{ErrorCode, RegistryError};
use crate::keys;
use crate::storage::Storage;

/// GET /v2/{name}/manifests/{reference}
///
/// The reference can be a tag or a digest.
pub async fn get_manifest(
    storage: &dyn Storage,
    name: &str,
    reference: &str,
) -> Result<(Vec<u8>, String), RegistryError> {
    let key = keys::manifest(name, reference);
    let data = storage
        .get(&key)
        .await
        .map_err(RegistryError::Storage)?
        .ok_or_else(|| RegistryError::oci(ErrorCode::ManifestUnknown, "manifest not found"))?;
    let digest = sha256_digest(&data);
    Ok((data, digest))
}

/// HEAD /v2/{name}/manifests/{reference}
///
/// Returns (size, digest) if the manifest exists.
pub async fn head_manifest(
    storage: &dyn Storage,
    name: &str,
    reference: &str,
) -> Result<(u64, String), RegistryError> {
    let key = keys::manifest(name, reference);
    let data = storage
        .get(&key)
        .await
        .map_err(RegistryError::Storage)?
        .ok_or_else(|| RegistryError::oci(ErrorCode::ManifestUnknown, "manifest not found"))?;
    let digest = sha256_digest(&data);
    let size = data.len() as u64;
    Ok((size, digest))
}

/// PUT /v2/{name}/manifests/{reference}
///
/// Stores the manifest at both the tag key and the digest key.
pub async fn put_manifest(
    storage: &dyn Storage,
    name: &str,
    reference: &str,
    body: &[u8],
) -> Result<String, RegistryError> {
    // Validate it parses as a manifest
    let manifest: OciManifest = serde_json::from_slice(body)?;

    if manifest.schema_version != 2 {
        return Err(RegistryError::oci(
            ErrorCode::ManifestInvalid,
            format!(
                "unsupported schema version: {}, expected 2",
                manifest.schema_version
            ),
        ));
    }

    let digest = sha256_digest(body);

    // Store by tag
    let tag_key = keys::manifest(name, reference);
    storage
        .put(&tag_key, body, Some(MEDIA_TYPE_OCI_MANIFEST))
        .await
        .map_err(RegistryError::Storage)?;

    // Also store by digest (manifests are small, duplication is fine)
    let digest_key = keys::manifest(name, &digest);
    if digest_key != tag_key {
        storage
            .put(&digest_key, body, Some(MEDIA_TYPE_OCI_MANIFEST))
            .await
            .map_err(RegistryError::Storage)?;
    }

    Ok(digest)
}

/// DELETE /v2/{name}/manifests/{reference}
pub async fn delete_manifest(
    storage: &dyn Storage,
    name: &str,
    reference: &str,
) -> Result<(), RegistryError> {
    let key = keys::manifest(name, reference);
    storage.delete(&key).await.map_err(RegistryError::Storage)
}
