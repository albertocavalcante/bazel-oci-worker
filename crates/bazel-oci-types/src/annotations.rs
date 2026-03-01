/// Standard OCI annotation: creation timestamp (RFC 3339).
pub const ANNOTATION_CREATED: &str = "org.opencontainers.image.created";

/// Standard OCI annotation: human-readable title.
pub const ANNOTATION_TITLE: &str = "org.opencontainers.image.title";

/// Bazel annotation: module name.
pub const ANNOTATION_MODULE_NAME: &str = "dev.bazel.module.name";

/// Bazel annotation: module version.
pub const ANNOTATION_MODULE_VERSION: &str = "dev.bazel.module.version";

/// Bazel annotation: compatibility level.
pub const ANNOTATION_COMPATIBILITY_LEVEL: &str = "dev.bazel.module.compatibility_level";
