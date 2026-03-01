/// OCI artifact type for a Bazel module version manifest.
pub const ARTIFACT_TYPE_MODULE: &str = "application/vnd.bazel.module.v1+json";

/// Media type for the module config blob.
pub const MEDIA_TYPE_CONFIG: &str = "application/vnd.bazel.module.config.v1+json";

/// Media type for MODULE.bazel content.
pub const MEDIA_TYPE_MODULE_FILE: &str = "application/vnd.bazel.module.file.v1";

/// Media type for source.json content.
pub const MEDIA_TYPE_SOURCE_JSON: &str = "application/vnd.bazel.module.source.v1+json";

/// Media type for patch files.
pub const MEDIA_TYPE_PATCH: &str = "application/vnd.bazel.module.patch.v1";

/// Media type for overlay files (e.g., BUILD.bazel).
pub const MEDIA_TYPE_OVERLAY: &str = "application/vnd.bazel.module.overlay.v1";

/// Media type for module-level metadata.json.
pub const MEDIA_TYPE_METADATA: &str = "application/vnd.bazel.module.metadata.v1+json";

/// Standard OCI image manifest media type.
pub const MEDIA_TYPE_OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
