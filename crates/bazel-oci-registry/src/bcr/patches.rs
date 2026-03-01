use bazel_oci_types::media_types::MEDIA_TYPE_PATCH;
use bazel_oci_types::reference::module_repository;

use crate::error::RegistryError;
use crate::storage::Storage;

/// GET /modules/{name}/{version}/patches/{filename}
///
/// Fetches the `:{version}` manifest, finds all `MEDIA_TYPE_PATCH` layers,
/// and returns the one whose title annotation matches `"patches/{filename}"`.
pub async fn get_patch(
    storage: &dyn Storage,
    namespace: &str,
    module: &str,
    version: &str,
    filename: &str,
) -> Result<Vec<u8>, RegistryError> {
    let repo = module_repository(namespace, module);
    let manifest = super::fetch_manifest(storage, &repo, version).await?;
    let patches =
        super::get_layers_by_media_type(storage, &repo, &manifest, MEDIA_TYPE_PATCH).await?;

    // The title annotation is "patches/{filename}" as set by Go's BuildLayers
    let title = format!("patches/{filename}");
    patches.get(&title).cloned().ok_or(RegistryError::NotFound)
}
