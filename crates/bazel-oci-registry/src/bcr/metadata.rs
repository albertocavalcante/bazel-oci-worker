use bazel_oci_types::media_types::MEDIA_TYPE_METADATA;
use bazel_oci_types::reference::{METADATA_TAG, module_repository};

use crate::error::RegistryError;
use crate::storage::Storage;

/// GET /modules/{name}/metadata.json
///
/// Fetches the `:metadata` manifest, finds the `MEDIA_TYPE_METADATA` layer,
/// and returns its raw JSON content.
pub async fn get_metadata(
    storage: &dyn Storage,
    namespace: &str,
    module: &str,
) -> Result<Vec<u8>, RegistryError> {
    let repo = module_repository(namespace, module);
    let manifest = super::fetch_manifest(storage, &repo, METADATA_TAG).await?;
    super::get_layer_by_media_type(storage, &repo, &manifest, MEDIA_TYPE_METADATA).await
}
