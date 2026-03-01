use serde::Serialize;

use crate::error::RegistryError;
use crate::keys;
use crate::storage::Storage;

/// Response for GET /v2/_catalog.
#[derive(Debug, Serialize)]
pub struct CatalogResponse {
    pub repositories: Vec<String>,
}

/// Response for GET /v2/{name}/tags/list.
#[derive(Debug, Serialize)]
pub struct TagsListResponse {
    pub name: String,
    pub tags: Vec<String>,
}

/// GET /v2/_catalog
///
/// List all repository names. Extracts unique repo names from manifest keys.
pub async fn catalog(storage: &dyn Storage) -> Result<CatalogResponse, RegistryError> {
    // List all keys that contain /manifests/ and extract the repo name portion
    let all_keys = storage
        .list_keys("")
        .await
        .map_err(RegistryError::Storage)?;

    let mut repos = std::collections::BTreeSet::new();
    for key in &all_keys {
        if let Some(idx) = key.find(keys::MANIFESTS_SEGMENT) {
            repos.insert(key[..idx].to_owned());
        }
    }

    Ok(CatalogResponse {
        repositories: repos.into_iter().collect(),
    })
}

/// GET /v2/{name}/tags/list
///
/// List all tags for a given repository.
pub async fn tags_list(
    storage: &dyn Storage,
    name: &str,
) -> Result<TagsListResponse, RegistryError> {
    let prefix = keys::manifest_prefix(name);
    let keys = storage
        .list_keys(&prefix)
        .await
        .map_err(RegistryError::Storage)?;

    let tags: Vec<String> = keys
        .iter()
        .filter_map(|k| {
            let tag = k.strip_prefix(&prefix)?;
            // Skip digest references (sha256:...)
            if tag.starts_with("sha256:") {
                None
            } else {
                Some(tag.to_owned())
            }
        })
        .collect();

    Ok(TagsListResponse {
        name: name.to_owned(),
        tags,
    })
}
