use bazel_oci_types::media_types::MEDIA_TYPE_OVERLAY;
use bazel_oci_types::reference::module_repository;

use crate::error::RegistryError;
use crate::storage::Storage;

/// GET /modules/{name}/{version}/overlay/{path}
///
/// Fetches the `:{version}` manifest, finds all `MEDIA_TYPE_OVERLAY` layers,
/// and returns the one whose title annotation matches `"overlay/{path}"`.
pub async fn get_overlay(
    storage: &dyn Storage,
    namespace: &str,
    module: &str,
    version: &str,
    overlay_path: &str,
) -> Result<Vec<u8>, RegistryError> {
    let repo = module_repository(namespace, module);
    let manifest = super::fetch_manifest(storage, &repo, version).await?;
    let overlays =
        super::get_layers_by_media_type(storage, &repo, &manifest, MEDIA_TYPE_OVERLAY).await?;

    // The title annotation is "overlay/{path}" as set by Go's BuildLayers
    let title = format!("overlay/{overlay_path}");
    overlays.get(&title).cloned().ok_or(RegistryError::NotFound)
}
