use bazel_oci_types::digest::sha256_digest;
use serde::{Deserialize, Serialize};

use crate::error::{ErrorCode, RegistryError};
use crate::keys;
use crate::oci::blobs::store_blob;
use crate::storage::Storage;

/// Upload session metadata.
#[derive(Debug, Serialize, Deserialize)]
struct UploadMeta {
    name: String,
    uuid: String,
    offset: u64,
}

/// Remove upload session artifacts from storage.
async fn cleanup_upload(storage: &dyn Storage, uuid: &str) {
    let _ = storage.delete(&keys::upload_data(uuid)).await;
    let _ = storage.delete(&keys::upload_meta(uuid)).await;
}

/// POST /v2/{name}/blobs/uploads/
///
/// Initiate a new upload session. Returns the upload UUID.
pub async fn initiate_upload(
    storage: &dyn Storage,
    name: &str,
    uuid: &str,
) -> Result<String, RegistryError> {
    let meta = UploadMeta {
        name: name.to_owned(),
        uuid: uuid.to_owned(),
        offset: 0,
    };
    let meta_json = serde_json::to_vec(&meta).map_err(|e| RegistryError::Storage(e.to_string()))?;

    let meta_key = keys::upload_meta(uuid);
    storage
        .put(&meta_key, &meta_json, Some("application/json"))
        .await
        .map_err(RegistryError::Storage)?;

    Ok(uuid.to_owned())
}

/// PATCH /v2/{name}/blobs/uploads/{uuid}
///
/// Append a chunk to an in-progress upload. Returns the new offset.
pub async fn upload_chunk(
    storage: &dyn Storage,
    uuid: &str,
    data: &[u8],
) -> Result<u64, RegistryError> {
    // Verify upload exists
    let meta_key = keys::upload_meta(uuid);
    let meta_bytes = storage
        .get(&meta_key)
        .await
        .map_err(RegistryError::Storage)?
        .ok_or_else(|| {
            RegistryError::oci(ErrorCode::BlobUploadUnknown, "upload session not found")
        })?;

    let mut meta: UploadMeta =
        serde_json::from_slice(&meta_bytes).map_err(|e| RegistryError::Storage(e.to_string()))?;

    // Append data
    let data_key = keys::upload_data(uuid);
    let new_size = storage
        .append(&data_key, data)
        .await
        .map_err(RegistryError::Storage)?;

    // Update offset
    meta.offset = new_size;
    let meta_json = serde_json::to_vec(&meta).map_err(|e| RegistryError::Storage(e.to_string()))?;
    storage
        .put(&meta_key, &meta_json, Some("application/json"))
        .await
        .map_err(RegistryError::Storage)?;

    Ok(new_size)
}

/// PUT /v2/{name}/blobs/uploads/{uuid}?digest=...
///
/// Complete an upload session with an optional final chunk.
/// Returns the computed digest.
pub async fn complete_upload(
    storage: &dyn Storage,
    name: &str,
    uuid: &str,
    expected_digest: &str,
    final_chunk: Option<&[u8]>,
) -> Result<String, RegistryError> {
    let data_key = keys::upload_data(uuid);

    // Get accumulated data
    let mut data = storage
        .get(&data_key)
        .await
        .map_err(RegistryError::Storage)?
        .unwrap_or_default();

    // Append final chunk if present
    if let Some(chunk) = final_chunk {
        data.extend_from_slice(chunk);
    }

    // Verify digest
    let computed = sha256_digest(&data);
    if computed != expected_digest {
        cleanup_upload(storage, uuid).await;
        return Err(RegistryError::oci(
            ErrorCode::DigestInvalid,
            format!("digest mismatch: computed {computed}, expected {expected_digest}"),
        ));
    }

    // Store the blob
    store_blob(storage, name, &computed, &data, None).await?;

    // Clean up upload session
    cleanup_upload(storage, uuid).await;

    Ok(computed)
}

/// Handle monolithic upload (POST with digest query param, or PUT with full body).
///
/// This is the single-request upload path where the entire blob is sent at once.
pub async fn monolithic_upload(
    storage: &dyn Storage,
    name: &str,
    expected_digest: &str,
    data: &[u8],
) -> Result<String, RegistryError> {
    let computed = sha256_digest(data);
    if computed != expected_digest {
        return Err(RegistryError::oci(
            ErrorCode::DigestInvalid,
            format!("digest mismatch: computed {computed}, expected {expected_digest}"),
        ));
    }

    store_blob(storage, name, &computed, data, None).await?;
    Ok(computed)
}
