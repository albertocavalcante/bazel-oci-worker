use http::StatusCode;

/// Result of checking authorization on a request.
pub enum AuthResult {
    Allowed,
    Denied,
}

/// Validate a write request's Authorization header against the configured token.
///
/// Supports:
/// - `Bearer {token}` -- token matches directly
/// - `Basic {base64}` -- decodes to `{anything}:{token}`
///
/// Returns `AuthResult::Allowed` if `auth_token` is `None` (auth disabled).
///
// TODO(go-wasm): Go's `http.Handler` wrapping is more verbose than axum layers.
// Rust's axum middleware/extractor pattern naturally composes auth checks,
// while Go requires explicit HandlerFunc wrapping or middleware chains.
pub fn check_auth(auth_header: Option<&str>, auth_token: Option<&str>) -> AuthResult {
    let Some(expected) = auth_token else {
        return AuthResult::Allowed;
    };

    let Some(header) = auth_header else {
        return AuthResult::Denied;
    };

    if let Some(token) = header.strip_prefix("Bearer ") {
        if token == expected {
            return AuthResult::Allowed;
        }
    }

    if let Some(encoded) = header.strip_prefix("Basic ") {
        if let Some(decoded) = decode_base64(encoded) {
            // Format: {username}:{password} -- we only check the password
            if let Some((_user, pass)) = decoded.split_once(':') {
                if pass == expected {
                    return AuthResult::Allowed;
                }
            }
        }
    }

    AuthResult::Denied
}

/// WWW-Authenticate header value for 401 responses.
pub const WWW_AUTHENTICATE: &str = "Bearer realm=\"bazel-oci-registry\"";

/// Status code for unauthorized requests.
pub const UNAUTHORIZED: StatusCode = StatusCode::UNAUTHORIZED;

/// Minimal base64 decoder (avoids pulling in a crate for this one use).
fn decode_base64(input: &str) -> Option<String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn val(c: u8) -> Option<u8> {
        TABLE.iter().position(|&b| b == c).map(|p| p as u8)
    }

    let input = input.trim_end_matches('=');
    let bytes: Vec<u8> = input.bytes().collect();
    let mut output = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        let mut buf = [0u8; 4];
        let mut count = 0;
        for (i, &b) in chunk.iter().enumerate() {
            buf[i] = val(b)?;
            count += 1;
        }

        if count >= 2 {
            output.push((buf[0] << 2) | (buf[1] >> 4));
        }
        if count >= 3 {
            output.push((buf[1] << 4) | (buf[2] >> 2));
        }
        if count >= 4 {
            output.push((buf[2] << 6) | buf[3]);
        }
    }

    String::from_utf8(output).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn bearer_correct_token_allowed() {
        assert!(matches!(
            check_auth(Some("Bearer secret123"), Some("secret123")),
            AuthResult::Allowed
        ));
    }

    #[test]
    fn bearer_wrong_token_denied() {
        assert!(matches!(
            check_auth(Some("Bearer wrong"), Some("secret123")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn bearer_empty_token_denied() {
        assert!(matches!(
            check_auth(Some("Bearer "), Some("secret123")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn bearer_case_sensitive_scheme() {
        // "bearer" (lowercase) should not match -- OCI spec uses "Bearer"
        assert!(matches!(
            check_auth(Some("bearer secret123"), Some("secret123")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn basic_correct_password_allowed() {
        // base64("user:secret123") = "dXNlcjpzZWNyZXQxMjM="
        assert!(matches!(
            check_auth(Some("Basic dXNlcjpzZWNyZXQxMjM="), Some("secret123")),
            AuthResult::Allowed
        ));
    }

    #[test]
    fn basic_wrong_password_denied() {
        // base64("user:wrongpass") = "dXNlcjp3cm9uZ3Bhc3M="
        assert!(matches!(
            check_auth(Some("Basic dXNlcjp3cm9uZ3Bhc3M="), Some("secret123")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn basic_empty_username_with_correct_password() {
        // base64(":secret123") = "OnNlY3JldDEyMw=="
        assert!(matches!(
            check_auth(Some("Basic OnNlY3JldDEyMw=="), Some("secret123")),
            AuthResult::Allowed
        ));
    }

    #[test]
    fn basic_missing_colon_denied() {
        // base64("nocolon") = "bm9jb2xvbg=="
        assert!(matches!(
            check_auth(Some("Basic bm9jb2xvbg=="), Some("nocolon")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn basic_invalid_base64_denied() {
        assert!(matches!(
            check_auth(Some("Basic !!!invalid!!!"), Some("secret")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn no_auth_configured_always_allows() {
        assert!(matches!(check_auth(None, None), AuthResult::Allowed));
        assert!(matches!(
            check_auth(Some("Bearer anything"), None),
            AuthResult::Allowed
        ));
        assert!(matches!(
            check_auth(Some("garbage"), None),
            AuthResult::Allowed
        ));
    }

    #[test]
    fn missing_header_when_auth_required_denied() {
        assert!(matches!(
            check_auth(None, Some("secret")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn unrecognized_scheme_denied() {
        assert!(matches!(
            check_auth(Some("Digest abc123"), Some("abc123")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn empty_header_denied() {
        assert!(matches!(
            check_auth(Some(""), Some("secret")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn decode_base64_known_values() {
        assert_eq!(
            decode_base64("dXNlcjpzZWNyZXQxMjM=").unwrap(),
            "user:secret123"
        );
        assert_eq!(decode_base64("OnNlY3JldDEyMw==").unwrap(), ":secret123");
        assert_eq!(decode_base64("bm9jb2xvbg==").unwrap(), "nocolon");
        assert_eq!(decode_base64("").unwrap(), "");
    }

    #[test]
    fn decode_base64_without_padding() {
        // Same as "dXNlcjpzZWNyZXQxMjM=" but without trailing '='
        assert_eq!(
            decode_base64("dXNlcjpzZWNyZXQxMjM").unwrap(),
            "user:secret123"
        );
    }

    #[test]
    fn decode_base64_invalid_chars_returns_none() {
        assert!(decode_base64("!!!").is_none());
    }

    #[test]
    fn bearer_scheme_only_no_space_denied() {
        // "Bearer" without trailing space and token
        assert!(matches!(
            check_auth(Some("Bearer"), Some("token")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn bearer_double_space_denied() {
        // "Bearer  token" (double space) -- the token becomes " token" which won't match
        assert!(matches!(
            check_auth(Some("Bearer  token"), Some("token")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn basic_scheme_only_no_space_denied() {
        assert!(matches!(
            check_auth(Some("Basic"), Some("secret")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn basic_empty_encoded_value() {
        // "Basic " with empty base64 → decodes to "" → no colon → denied
        assert!(matches!(
            check_auth(Some("Basic "), Some("secret")),
            AuthResult::Denied
        ));
    }

    #[test]
    fn decode_base64_various_padding() {
        // 1 byte: "a" → "YQ==" or "YQ"
        assert_eq!(decode_base64("YQ==").unwrap(), "a");
        assert_eq!(decode_base64("YQ").unwrap(), "a");

        // 2 bytes: "ab" → "YWI=" or "YWI"
        assert_eq!(decode_base64("YWI=").unwrap(), "ab");
        assert_eq!(decode_base64("YWI").unwrap(), "ab");

        // 3 bytes: "abc" → "YWJj" (no padding needed)
        assert_eq!(decode_base64("YWJj").unwrap(), "abc");

        // 4 bytes: "abcd" → "YWJjZA=="
        assert_eq!(decode_base64("YWJjZA==").unwrap(), "abcd");
        assert_eq!(decode_base64("YWJjZA").unwrap(), "abcd");
    }

    #[test]
    fn decode_base64_invalid_in_middle() {
        assert!(decode_base64("YW!!!Jj").is_none());
    }

    #[test]
    fn decode_base64_single_valid_char() {
        // A single base64 char produces partial data (only 6 bits → 0 complete bytes)
        // chunks(4) gives one chunk of size 1, count=1, which is < 2 → no output
        assert_eq!(decode_base64("Y").unwrap(), "");
    }

    #[test]
    fn bearer_token_with_special_chars() {
        let token = "abc123!@#$%^&*()_+-=";
        assert!(matches!(
            check_auth(Some(&format!("Bearer {token}")), Some(token)),
            AuthResult::Allowed
        ));
    }

    #[test]
    fn basic_password_with_colons() {
        // base64("user:pass:with:colons") = "dXNlcjpwYXNzOndpdGg6Y29sb25z"
        // split_once(':') → ("user", "pass:with:colons")
        assert!(matches!(
            check_auth(
                Some("Basic dXNlcjpwYXNzOndpdGg6Y29sb25z"),
                Some("pass:with:colons")
            ),
            AuthResult::Allowed
        ));
    }

    #[test]
    fn www_authenticate_header_value() {
        assert_eq!(WWW_AUTHENTICATE, "Bearer realm=\"bazel-oci-registry\"");
    }

    #[test]
    fn unauthorized_status_code() {
        assert_eq!(UNAUTHORIZED, http::StatusCode::UNAUTHORIZED);
    }
}
