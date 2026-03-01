pub mod metadata;
pub mod module_bazel;
pub mod overlays;
pub mod patches;
pub mod registry_json;
pub mod source_json;

use bazel_oci_types::annotations::ANNOTATION_TITLE;
use bazel_oci_types::oci::OciManifest;

use crate::error::{ErrorCode, RegistryError};
use crate::keys;
use crate::storage::Storage;

/// Fetch and parse an OCI manifest for a given name and reference.
async fn fetch_manifest(
    storage: &dyn Storage,
    name: &str,
    reference: &str,
) -> Result<OciManifest, RegistryError> {
    let key = keys::manifest(name, reference);
    let data = storage
        .get(&key)
        .await
        .map_err(RegistryError::Storage)?
        .ok_or(RegistryError::NotFound)?;
    serde_json::from_slice(&data).map_err(Into::into)
}

/// Find a layer by media type and return its blob content.
async fn get_layer_by_media_type(
    storage: &dyn Storage,
    name: &str,
    manifest: &OciManifest,
    media_type: &str,
) -> Result<Vec<u8>, RegistryError> {
    let layer = manifest
        .layers
        .iter()
        .find(|l| l.media_type == media_type)
        .ok_or_else(|| {
            RegistryError::oci(
                ErrorCode::ManifestBlobUnknown,
                format!("no layer with media type {media_type}"),
            )
        })?;

    let (algo, hex) = bazel_oci_types::digest::parse_digest(&layer.digest).ok_or_else(|| {
        RegistryError::oci(ErrorCode::DigestInvalid, "invalid layer digest format")
    })?;

    let key = keys::blob(name, algo, hex);
    storage
        .get(&key)
        .await
        .map_err(RegistryError::Storage)?
        .ok_or_else(|| RegistryError::oci(ErrorCode::BlobUnknown, "layer blob not found"))
}

/// Find all layers of a given media type, keyed by their title annotation.
async fn get_layers_by_media_type(
    storage: &dyn Storage,
    name: &str,
    manifest: &OciManifest,
    media_type: &str,
) -> Result<std::collections::HashMap<String, Vec<u8>>, RegistryError> {
    let mut result = std::collections::HashMap::new();

    for layer in &manifest.layers {
        if layer.media_type != media_type {
            continue;
        }

        let title = layer
            .annotations
            .as_ref()
            .and_then(|a| a.get(ANNOTATION_TITLE))
            .cloned()
            .unwrap_or_default();

        let (algo, hex) =
            bazel_oci_types::digest::parse_digest(&layer.digest).ok_or_else(|| {
                RegistryError::oci(ErrorCode::DigestInvalid, "invalid layer digest format")
            })?;

        let key = keys::blob(name, algo, hex);
        let data = storage
            .get(&key)
            .await
            .map_err(RegistryError::Storage)?
            .ok_or_else(|| RegistryError::oci(ErrorCode::BlobUnknown, "layer blob not found"))?;

        result.insert(title, data);
    }

    Ok(result)
}
