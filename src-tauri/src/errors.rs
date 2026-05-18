use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub stage: String,
    pub retryable: bool,
    pub details: Option<String>,
    pub correlation_id: String,
}

impl AppError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        stage: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            stage: stage.into(),
            retryable,
            details: None,
            correlation_id: Uuid::new_v4().to_string(),
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(redact_secrets(&details.into()));
        self
    }

    pub fn io(stage: impl Into<String>, code: impl Into<String>, err: std::io::Error) -> Self {
        Self::new(
            code,
            "文件系统操作失败，请检查目录权限后重试。",
            stage,
            true,
        )
        .with_details(err.to_string())
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

pub fn redact_secrets(input: &str) -> String {
    const SECRET_KEYS: [&str; 5] = ["api_key", "apikey", "authorization", "token", "secret"];
    let lower = input.to_lowercase();
    if SECRET_KEYS.iter().any(|key| lower.contains(key)) {
        return "[redacted]".to_string();
    }
    input.chars().take(800).collect()
}
