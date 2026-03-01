use serde::{Deserialize, Serialize};
use thiserror::Error;

/// OCI Distribution error codes per the spec.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    BlobUnknown,
    BlobUploadInvalid,
    BlobUploadUnknown,
    DigestInvalid,
    ManifestBlobUnknown,
    ManifestInvalid,
    ManifestUnknown,
    NameInvalid,
    NameUnknown,
    SizeInvalid,
    Unauthorized,
    Denied,
    Unsupported,
}

impl ErrorCode {
    pub const fn status_code(self) -> u16 {
        match self {
            Self::Unauthorized => 401,
            Self::Denied => 403,
            Self::BlobUnknown | Self::ManifestUnknown | Self::NameUnknown => 404,
            Self::BlobUploadUnknown => 404,
            Self::Unsupported => 405,
            Self::BlobUploadInvalid
            | Self::DigestInvalid
            | Self::ManifestBlobUnknown
            | Self::ManifestInvalid
            | Self::NameInvalid
            | Self::SizeInvalid => 400,
        }
    }
}

/// OCI error response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub errors: Vec<ErrorEntry>,
}

/// Single error entry in an OCI error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Registry error type.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("OCI error: {code:?} -- {message}")]
    Oci {
        code: ErrorCode,
        message: String,
        detail: Option<String>,
    },
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not found")]
    NotFound,
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl RegistryError {
    pub fn oci(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Oci {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub const fn status_code(&self) -> u16 {
        match self {
            Self::Oci { code, .. } => code.status_code(),
            Self::Storage(_) | Self::Json(_) => 500,
            Self::NotFound => 404,
        }
    }

    pub fn to_error_response(&self) -> ErrorResponse {
        match self {
            Self::Oci {
                code,
                message,
                detail,
            } => ErrorResponse {
                errors: vec![ErrorEntry {
                    code: *code,
                    message: message.clone(),
                    detail: detail.clone(),
                }],
            },
            Self::Storage(msg) => ErrorResponse {
                errors: vec![ErrorEntry {
                    code: ErrorCode::BlobUnknown,
                    message: msg.clone(),
                    detail: None,
                }],
            },
            Self::NotFound => ErrorResponse {
                errors: vec![ErrorEntry {
                    code: ErrorCode::NameUnknown,
                    message: "not found".to_owned(),
                    detail: None,
                }],
            },
            Self::Json(e) => ErrorResponse {
                errors: vec![ErrorEntry {
                    code: ErrorCode::ManifestInvalid,
                    message: e.to_string(),
                    detail: None,
                }],
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn error_code_status_codes_all_variants() {
        assert_eq!(ErrorCode::Unauthorized.status_code(), 401);
        assert_eq!(ErrorCode::Denied.status_code(), 403);
        assert_eq!(ErrorCode::BlobUnknown.status_code(), 404);
        assert_eq!(ErrorCode::ManifestUnknown.status_code(), 404);
        assert_eq!(ErrorCode::NameUnknown.status_code(), 404);
        assert_eq!(ErrorCode::BlobUploadUnknown.status_code(), 404);
        assert_eq!(ErrorCode::Unsupported.status_code(), 405);
        assert_eq!(ErrorCode::BlobUploadInvalid.status_code(), 400);
        assert_eq!(ErrorCode::DigestInvalid.status_code(), 400);
        assert_eq!(ErrorCode::ManifestBlobUnknown.status_code(), 400);
        assert_eq!(ErrorCode::ManifestInvalid.status_code(), 400);
        assert_eq!(ErrorCode::NameInvalid.status_code(), 400);
        assert_eq!(ErrorCode::SizeInvalid.status_code(), 400);
    }

    #[test]
    fn error_code_serializes_screaming_snake_case() {
        let json = serde_json::to_string(&ErrorCode::BlobUnknown).unwrap();
        assert_eq!(json, "\"BLOB_UNKNOWN\"");

        let json = serde_json::to_string(&ErrorCode::BlobUploadInvalid).unwrap();
        assert_eq!(json, "\"BLOB_UPLOAD_INVALID\"");

        let json = serde_json::to_string(&ErrorCode::ManifestBlobUnknown).unwrap();
        assert_eq!(json, "\"MANIFEST_BLOB_UNKNOWN\"");

        let json = serde_json::to_string(&ErrorCode::DigestInvalid).unwrap();
        assert_eq!(json, "\"DIGEST_INVALID\"");
    }

    #[test]
    fn error_code_deserializes_screaming_snake_case() {
        let code: ErrorCode = serde_json::from_str("\"BLOB_UNKNOWN\"").unwrap();
        assert_eq!(code, ErrorCode::BlobUnknown);

        let code: ErrorCode = serde_json::from_str("\"UNAUTHORIZED\"").unwrap();
        assert_eq!(code, ErrorCode::Unauthorized);
    }

    #[test]
    fn registry_error_oci_status_code() {
        let err = RegistryError::oci(ErrorCode::DigestInvalid, "bad digest");
        assert_eq!(err.status_code(), 400);

        let err = RegistryError::oci(ErrorCode::ManifestUnknown, "not found");
        assert_eq!(err.status_code(), 404);

        let err = RegistryError::oci(ErrorCode::Unauthorized, "no auth");
        assert_eq!(err.status_code(), 401);
    }

    #[test]
    fn registry_error_storage_status_code() {
        let err = RegistryError::Storage("backend error".to_owned());
        assert_eq!(err.status_code(), 500);
    }

    #[test]
    fn registry_error_not_found_status_code() {
        assert_eq!(RegistryError::NotFound.status_code(), 404);
    }

    #[test]
    fn registry_error_json_status_code() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = RegistryError::Json(json_err);
        assert_eq!(err.status_code(), 500);
    }

    #[test]
    fn oci_error_response_serializes_to_valid_json() {
        let err = RegistryError::Oci {
            code: ErrorCode::DigestInvalid,
            message: "bad digest".to_owned(),
            detail: Some("expected sha256:abc, got sha256:def".to_owned()),
        };
        let resp = err.to_error_response();
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.errors.len(), 1);
        assert_eq!(parsed.errors[0].code, ErrorCode::DigestInvalid);
        assert_eq!(parsed.errors[0].message, "bad digest");
        assert_eq!(
            parsed.errors[0].detail.as_deref(),
            Some("expected sha256:abc, got sha256:def")
        );
    }

    #[test]
    fn oci_error_response_without_detail_omits_field() {
        let err = RegistryError::oci(ErrorCode::BlobUnknown, "blob not found");
        let resp = err.to_error_response();
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("\"detail\""));
    }

    #[test]
    fn storage_error_response_uses_blob_unknown_code() {
        let err = RegistryError::Storage("r2 timeout".to_owned());
        let resp = err.to_error_response();
        assert_eq!(resp.errors.len(), 1);
        assert_eq!(resp.errors[0].code, ErrorCode::BlobUnknown);
        assert_eq!(resp.errors[0].message, "r2 timeout");
        assert!(resp.errors[0].detail.is_none());
    }

    #[test]
    fn not_found_error_response_uses_name_unknown_code() {
        let resp = RegistryError::NotFound.to_error_response();
        assert_eq!(resp.errors.len(), 1);
        assert_eq!(resp.errors[0].code, ErrorCode::NameUnknown);
        assert_eq!(resp.errors[0].message, "not found");
    }

    #[test]
    fn json_error_response_uses_manifest_invalid_code() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = RegistryError::Json(json_err);
        let resp = err.to_error_response();
        assert_eq!(resp.errors.len(), 1);
        assert_eq!(resp.errors[0].code, ErrorCode::ManifestInvalid);
        assert!(!resp.errors[0].message.is_empty());
    }

    #[test]
    fn registry_error_oci_convenience_sets_detail_to_none() {
        let err = RegistryError::oci(ErrorCode::BlobUnknown, "test");
        match err {
            RegistryError::Oci { detail, .. } => assert!(detail.is_none()),
            _ => panic!("expected Oci variant"),
        }
    }

    #[test]
    fn registry_error_display_messages() {
        let err = RegistryError::oci(ErrorCode::BlobUnknown, "not found");
        assert!(err.to_string().contains("not found"));

        let err = RegistryError::Storage("backend failed".to_owned());
        assert!(err.to_string().contains("backend failed"));

        assert_eq!(RegistryError::NotFound.to_string(), "not found");

        let json_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        let err = RegistryError::Json(json_err);
        assert!(err.to_string().contains("JSON error"));
    }
}
