//! Error shape returned to the frontend.

use serde::Serialize;

/// Every command fails with this structure so the UI can branch on `code`
/// (for example `INCORRECT_PASSWORD`) and still show `message` verbatim.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<lycoris_core::Error> for ApiError {
    fn from(value: lycoris_core::Error) -> Self {
        Self {
            code: value.code().to_string(),
            message: value.to_string(),
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(value: std::io::Error) -> Self {
        Self::new("IO", value.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        Self::new("JSON", value.to_string())
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;
