use async_trait::async_trait;

/// Metadata returned by a HEAD/stat operation.
#[derive(Debug, Clone)]
pub struct ObjectMeta {
    pub size: u64,
    pub content_type: Option<String>,
}

/// Abstract storage backend for the OCI registry.
///
/// Implementations include `InMemoryStorage` (for tests) and `R2Storage` (Cloudflare).
///
// TODO(go-wasm): Go WASI has no networking; `context.Context` doesn't map to JS promises.
// The async Storage trait relies on Rust's zero-cost Future model which compiles cleanly
// to wasm32. Go would need raw `syscall/js` callback chains or a WASI preview2 reactor,
// neither of which is production-ready as of Go 1.23.
#[async_trait(?Send)]
pub trait Storage {
    /// Retrieve an object by key. Returns `None` if not found.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;

    /// Store an object at the given key, overwriting if it exists.
    async fn put(&self, key: &str, data: &[u8], content_type: Option<&str>) -> Result<(), String>;

    /// Check if an object exists and return its metadata.
    async fn head(&self, key: &str) -> Result<Option<ObjectMeta>, String>;

    /// Delete an object by key.
    async fn delete(&self, key: &str) -> Result<(), String>;

    /// List keys matching a prefix.
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, String>;

    /// Append data to an existing object (or create if it doesn't exist).
    ///
    // TODO(go-wasm): Go's `io.Reader` streaming is broken in wasm32; must buffer all
    // data in memory before writing. Rust's `&[u8]` approach naturally buffers, so
    // append is just read-concat-write with no streaming assumption.
    async fn append(&self, key: &str, data: &[u8]) -> Result<u64, String>;
}
