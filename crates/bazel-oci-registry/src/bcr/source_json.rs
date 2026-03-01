use bazel_oci_types::media_types::MEDIA_TYPE_SOURCE_JSON;
use bazel_oci_types::reference::module_repository;

use crate::error::RegistryError;
use crate::storage::Storage;

/// GET /modules/{name}/{version}/source.json
///
/// Fetches the `:{version}` manifest, finds the `MEDIA_TYPE_SOURCE_JSON` layer,
/// and returns its raw JSON content.
pub async fn get_source_json(
    storage: &dyn Storage,
    namespace: &str,
    module: &str,
    version: &str,
) -> Result<Vec<u8>, RegistryError> {
    let repo = module_repository(namespace, module);
    let manifest = super::fetch_manifest(storage, &repo, version).await?;
    super::get_layer_by_media_type(storage, &repo, &manifest, MEDIA_TYPE_SOURCE_JSON).await
}
