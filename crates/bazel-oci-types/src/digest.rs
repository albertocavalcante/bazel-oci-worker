use sha2::{Digest, Sha256};

/// Compute the sha256 digest of `data`, returning `"sha256:{hex}"`.
pub fn sha256_digest(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    format!("sha256:{}", hex::encode(hash))
}

/// Parse a digest string like `"sha256:abcdef..."` into `(algorithm, hex)`.
///
/// Returns `None` if the format is invalid.
pub fn parse_digest(s: &str) -> Option<(&str, &str)> {
    let (algo, hex) = s.split_once(':')?;
    if algo.is_empty() || hex.is_empty() {
        return None;
    }
    Some((algo, hex))
}

mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn sha256_digest_empty_input() {
        assert_eq!(
            sha256_digest(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_digest_known_value() {
        assert_eq!(
            sha256_digest(b"hello"),
            "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn sha256_digest_deterministic() {
        let a = sha256_digest(b"same input");
        let b = sha256_digest(b"same input");
        assert_eq!(a, b);
    }

    #[test]
    fn sha256_digest_different_inputs_differ() {
        assert_ne!(sha256_digest(b"a"), sha256_digest(b"b"));
    }

    #[test]
    fn sha256_digest_starts_with_prefix() {
        assert!(sha256_digest(b"any").starts_with("sha256:"));
    }

    #[test]
    fn sha256_digest_hex_length_is_64() {
        let d = sha256_digest(b"test");
        let hex_part = d.strip_prefix("sha256:").unwrap();
        assert_eq!(hex_part.len(), 64);
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parse_digest_valid() {
        let (algo, hex) = parse_digest("sha256:abcdef0123456789").unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(hex, "abcdef0123456789");
    }

    #[test]
    fn parse_digest_non_sha256_algo() {
        let (algo, hex) = parse_digest("sha512:deadbeef").unwrap();
        assert_eq!(algo, "sha512");
        assert_eq!(hex, "deadbeef");
    }

    #[test]
    fn parse_digest_multiple_colons_splits_at_first() {
        let (algo, hex) = parse_digest("sha256:abc:def").unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(hex, "abc:def");
    }

    #[test]
    fn parse_digest_no_colon_returns_none() {
        assert!(parse_digest("nocolon").is_none());
    }

    #[test]
    fn parse_digest_empty_algo_returns_none() {
        assert!(parse_digest(":abcdef").is_none());
    }

    #[test]
    fn parse_digest_empty_hex_returns_none() {
        assert!(parse_digest("sha256:").is_none());
    }

    #[test]
    fn parse_digest_empty_string_returns_none() {
        assert!(parse_digest("").is_none());
    }

    #[test]
    fn parse_digest_just_colon_returns_none() {
        assert!(parse_digest(":").is_none());
    }

    #[test]
    fn sha256_digest_roundtrips_through_parse() {
        let digest = sha256_digest(b"roundtrip");
        let (algo, hex) = parse_digest(&digest).unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(hex.len(), 64);
    }
}
