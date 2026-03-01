//! Centralized storage key builders for the R2 object layout.
//!
//! All R2 keys used by the registry go through this module, ensuring
//! consistency across OCI and BCR code paths.
//!
//! ## Key layout
//!
//! ```text
//! {name}/manifests/{reference}   — manifest by tag or digest
//! {name}/blobs/{algo}/{hex}      — content-addressable blob
//! _uploads/{uuid}/data           — in-progress upload data
//! _uploads/{uuid}/_meta          — upload session metadata
//! ```

/// Key for a manifest identified by name and reference (tag or digest).
pub fn manifest(name: &str, reference: &str) -> String {
    format!("{name}/manifests/{reference}")
}

/// Prefix for listing all manifests under a repository.
pub fn manifest_prefix(name: &str) -> String {
    format!("{name}/manifests/")
}

/// The path segment used to locate manifests inside a full key.
pub const MANIFESTS_SEGMENT: &str = "/manifests/";

/// Key for a content-addressable blob.
pub fn blob(name: &str, algo: &str, hex: &str) -> String {
    format!("{name}/blobs/{algo}/{hex}")
}

/// Key for in-progress upload data.
pub fn upload_data(uuid: &str) -> String {
    format!("_uploads/{uuid}/data")
}

/// Key for upload session metadata.
pub fn upload_meta(uuid: &str) -> String {
    format!("_uploads/{uuid}/_meta")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_key() {
        assert_eq!(manifest("myrepo", "latest"), "myrepo/manifests/latest");
        assert_eq!(
            manifest("ns/foo", "sha256:abc123"),
            "ns/foo/manifests/sha256:abc123"
        );
    }

    #[test]
    fn manifest_prefix_key() {
        assert_eq!(manifest_prefix("myrepo"), "myrepo/manifests/");
    }

    #[test]
    fn manifests_segment_value() {
        assert_eq!(MANIFESTS_SEGMENT, "/manifests/");
    }

    #[test]
    fn blob_key() {
        assert_eq!(blob("myrepo", "sha256", "abc"), "myrepo/blobs/sha256/abc");
    }

    #[test]
    fn upload_data_key() {
        assert_eq!(upload_data("uuid-1"), "_uploads/uuid-1/data");
    }

    #[test]
    fn upload_meta_key() {
        assert_eq!(upload_meta("uuid-1"), "_uploads/uuid-1/_meta");
    }
}
