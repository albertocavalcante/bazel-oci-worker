use bazel_oci_types::media_types::MEDIA_TYPE_MODULE_FILE;
use bazel_oci_types::reference::module_repository;

use crate::error::RegistryError;
use crate::storage::Storage;

/// GET /modules/{name}/{version}/MODULE.bazel
///
/// Fetches the `:{version}` manifest, finds the `MEDIA_TYPE_MODULE_FILE` layer,
/// and returns its content.
pub async fn get_module_bazel(
    storage: &dyn Storage,
    namespace: &str,
    module: &str,
    version: &str,
) -> Result<Vec<u8>, RegistryError> {
    let repo = module_repository(namespace, module);
    let manifest = super::fetch_manifest(storage, &repo, version).await?;
    super::get_layer_by_media_type(storage, &repo, &manifest, MEDIA_TYPE_MODULE_FILE).await
}
