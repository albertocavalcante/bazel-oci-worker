use bazel_oci_types::digest::parse_digest;

use crate::error::{ErrorCode, RegistryError};
use crate::keys;
use crate::storage::Storage;

/// GET /v2/{name}/blobs/{digest}
///
/// Returns the blob content for the given digest.
pub async fn get_blob(
    storage: &dyn Storage,
    name: &str,
    digest: &str,
) -> Result<Vec<u8>, RegistryError> {
    let (algo, hex) = parse_digest(digest)
        .ok_or_else(|| RegistryError::oci(ErrorCode::DigestInvalid, "invalid digest format"))?;
    let key = keys::blob(name, algo, hex);
    storage
        .get(&key)
        .await
        .map_err(RegistryError::Storage)?
        .ok_or_else(|| RegistryError::oci(ErrorCode::BlobUnknown, "blob not found"))
}

/// HEAD /v2/{name}/blobs/{digest}
///
/// Returns the size if the blob exists.
pub async fn head_blob(
    storage: &dyn Storage,
    name: &str,
    digest: &str,
) -> Result<u64, RegistryError> {
    let (algo, hex) = parse_digest(digest)
        .ok_or_else(|| RegistryError::oci(ErrorCode::DigestInvalid, "invalid digest format"))?;
    let key = keys::blob(name, algo, hex);
    let meta = storage
        .head(&key)
        .await
        .map_err(RegistryError::Storage)?
        .ok_or_else(|| RegistryError::oci(ErrorCode::BlobUnknown, "blob not found"))?;
    Ok(meta.size)
}

/// DELETE /v2/{name}/blobs/{digest}
pub async fn delete_blob(
    storage: &dyn Storage,
    name: &str,
    digest: &str,
) -> Result<(), RegistryError> {
    let (algo, hex) = parse_digest(digest)
        .ok_or_else(|| RegistryError::oci(ErrorCode::DigestInvalid, "invalid digest format"))?;
    let key = keys::blob(name, algo, hex);
    storage.delete(&key).await.map_err(RegistryError::Storage)
}

/// Store a blob directly (used internally by upload completion and manifest PUT).
pub async fn store_blob(
    storage: &dyn Storage,
    name: &str,
    digest: &str,
    data: &[u8],
    content_type: Option<&str>,
) -> Result<(), RegistryError> {
    let (algo, hex) = parse_digest(digest)
        .ok_or_else(|| RegistryError::oci(ErrorCode::DigestInvalid, "invalid digest format"))?;
    let key = keys::blob(name, algo, hex);
    storage
        .put(&key, data, content_type)
        .await
        .map_err(RegistryError::Storage)
}
