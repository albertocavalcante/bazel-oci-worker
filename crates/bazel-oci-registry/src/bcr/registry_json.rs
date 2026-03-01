/// GET /bazel_registry.json
///
/// Returns `{}` -- the minimal registry config that tells Bazel this is a valid registry.
pub const fn registry_json() -> &'static [u8] {
    b"{}"
}
