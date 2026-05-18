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

#[cfg(test)]
mod tests {
    use super::redact_secrets;

    #[test]
    fn redact_secrets_removes_api_key_patterns() {
        assert_eq!(redact_secrets("api_key=sk-12345"), "[redacted]");
        assert_eq!(redact_secrets("apikey: my-secret-value"), "[redacted]");
        assert_eq!(redact_secrets("API_KEY: sk-abc"), "[redacted]");
    }

    #[test]
    fn redact_secrets_removes_authorization_headers() {
        assert_eq!(
            redact_secrets("Authorization: Bearer sk-12345"),
            "[redacted]"
        );
        assert_eq!(redact_secrets("bearer token_here"), "[redacted]");
    }

    #[test]
    fn redact_secrets_removes_token_and_secret() {
        assert_eq!(redact_secrets("token=abc123"), "[redacted]");
        assert_eq!(redact_secrets("secret: value"), "[redacted]");
    }

    #[test]
    fn redact_secrets_preserves_safe_content() {
        assert_eq!(redact_secrets("normal log message"), "normal log message");
        assert_eq!(
            redact_secrets("workspace initialized at /path"),
            "workspace initialized at /path"
        );
    }

    #[test]
    fn redact_secrets_truncates_long_safe_content() {
        let long = "a".repeat(1000);
        assert_eq!(redact_secrets(&long).len(), 800);
    }
}

pub fn redact_secrets(input: &str) -> String {
    const SECRET_KEYS: [&str; 5] = ["api_key", "apikey", "authorization", "token", "secret"];
    let lower = input.to_lowercase();
    if SECRET_KEYS.iter().any(|key| lower.contains(key)) {
        // Check for Authorization header pattern (case-insensitive)
        if lower.contains("authorization:") || lower.contains("bearer ") {
            return "[redacted]".to_string();
        }
        // Check for key=value or key:value patterns with actual secret content
        for key in &SECRET_KEYS {
            if let Some(pos) = lower.find(key) {
                let after = &input[pos + key.len()..];
                let after_trimmed = after.trim_start();
                if after_trimmed.starts_with(':') || after_trimmed.starts_with('=') {
                    return "[redacted]".to_string();
                }
            }
        }
        return "[redacted]".to_string();
    }
    input.chars().take(800).collect()
}
