/// OCI Distribution API version check response.
///
/// Returns 200 OK with the standard header.
pub struct PingResponse;

impl PingResponse {
    /// Header name for the OCI Distribution API version.
    pub const HEADER_NAME: &str = "Docker-Distribution-API-Version";
    /// Header value for registry/2.0.
    pub const HEADER_VALUE: &str = "registry/2.0";
}
