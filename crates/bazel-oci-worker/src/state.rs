use std::ops::Deref;

use bazel_oci_registry::storage::Storage;

/// Wraps an authentication token to prevent accidental logging.
pub struct SecretToken(String);

impl SecretToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }
}

impl std::fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

impl Deref for SecretToken {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

/// Application state shared across all request handlers.
pub struct AppState<S: Storage> {
    pub storage: S,
    pub namespace: String,
    pub auth_token: Option<SecretToken>,
}
