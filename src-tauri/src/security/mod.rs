#![cfg_attr(test, allow(unreachable_code))]

use crate::errors::{AppError, AppResult};

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

const SERVICE_NAME: &str = "slicer";
const API_KEY_USER: &str = "model_api_key";
const SILICONFLOW_API_KEY_USER: &str = "model_api_key_siliconflow";
const OPENAI_API_KEY_USER: &str = "model_api_key_openai";
const ANTHROPIC_API_KEY_USER: &str = "model_api_key_anthropic";
const API_KEY_USER_PREFIX: &str = "model_api_key:";

/// Store API key in OS credential storage (keyring).
pub fn store_api_key(key: &str) -> AppResult<()> {
    store_api_key_for_user(API_KEY_USER, key)
}

pub fn store_api_key_for_provider(provider: &str, key: &str) -> AppResult<()> {
    store_api_key_for_user(api_key_user_for_provider(provider), key)
}

pub fn store_api_key_for_id(key_id: &str, key: &str) -> AppResult<()> {
    store_api_key_for_user(&api_key_user_for_id(key_id), key)
}

fn store_api_key_for_user(user: &str, key: &str) -> AppResult<()> {
    let key = normalize_api_key_input(key);
    if key.is_empty() {
        return Err(AppError::new(
            "api_key_empty",
            "API key 不能为空。",
            "security",
            true,
        ));
    }

    if looks_like_url_value(&key) {
        return Err(api_key_looks_like_url_error());
    }

    #[cfg(test)]
    {
        test_keyring()
            .lock()
            .map_err(|_| keyring_poisoned_error())?
            .insert(user.to_string(), key);
        return Ok(());
    }

    let entry = keyring::Entry::new(SERVICE_NAME, user).map_err(|err| {
        AppError::new(
            "keyring_access_failed",
            "无法访问系统密钥存储，请检查系统凭据管理器。",
            "security",
            true,
        )
        .with_details(err.to_string())
    })?;
    entry.set_password(&key).map_err(|err| {
        AppError::new(
            "keyring_write_failed",
            "API key 保存失败，请重试。",
            "security",
            true,
        )
        .with_details(err.to_string())
    })?;
    tracing::info!(target: "security", "API key 已安全保存到系统密钥存储");
    Ok(())
}

pub fn read_api_key_for_provider(provider: &str) -> AppResult<Option<String>> {
    read_api_key_for_user(api_key_user_for_provider(provider))
}

pub fn read_api_key_for_id(key_id: &str) -> AppResult<Option<String>> {
    read_api_key_for_user(&api_key_user_for_id(key_id))
}

fn read_api_key_for_user(user: &str) -> AppResult<Option<String>> {
    #[cfg(test)]
    {
        let Some(key) = test_keyring()
            .lock()
            .map_err(|_| keyring_poisoned_error())?
            .get(user)
            .cloned()
        else {
            return Ok(None);
        };
        let normalized = normalize_api_key_input(&key);
        if normalized.is_empty() {
            return Ok(None);
        }
        if looks_like_url_value(&normalized) {
            return Err(api_key_looks_like_url_error());
        }
        return Ok(Some(normalized));
    }

    let entry = keyring::Entry::new(SERVICE_NAME, user).map_err(|err| {
        AppError::new(
            "keyring_access_failed",
            "无法访问系统密钥存储，请检查系统凭据管理器。",
            "security",
            true,
        )
        .with_details(err.to_string())
    })?;
    match entry.get_password() {
        Ok(key) => {
            let normalized = normalize_api_key_input(&key);
            if normalized.is_empty() {
                Ok(None)
            } else if looks_like_url_value(&normalized) {
                Err(api_key_looks_like_url_error())
            } else {
                Ok(Some(normalized))
            }
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(AppError::new(
            "keyring_read_failed",
            "API key 读取失败，请重试。",
            "security",
            true,
        )
        .with_details(err.to_string())),
    }
}

/// Delete API key from OS credential storage.
pub fn delete_api_key() -> AppResult<()> {
    delete_api_key_for_user(API_KEY_USER)
}

pub fn delete_api_key_for_provider(provider: &str) -> AppResult<()> {
    delete_api_key_for_user(api_key_user_for_provider(provider))
}

pub fn delete_api_key_for_id(key_id: &str) -> AppResult<()> {
    delete_api_key_for_user(&api_key_user_for_id(key_id))
}

fn delete_api_key_for_user(user: &str) -> AppResult<()> {
    #[cfg(test)]
    {
        test_keyring()
            .lock()
            .map_err(|_| keyring_poisoned_error())?
            .remove(user);
        return Ok(());
    }

    let entry = keyring::Entry::new(SERVICE_NAME, user).map_err(|err| {
        AppError::new(
            "keyring_access_failed",
            "无法访问系统密钥存储，请检查系统凭据管理器。",
            "security",
            true,
        )
        .with_details(err.to_string())
    })?;
    match entry.delete_credential() {
        Ok(()) => {
            tracing::info!(target: "security", "API key 已从系统密钥存储中删除");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(AppError::new(
            "keyring_delete_failed",
            "API key 删除失败，请重试。",
            "security",
            true,
        )
        .with_details(err.to_string())),
    }
}

fn api_key_user_for_provider(provider: &str) -> &'static str {
    match provider.trim().to_ascii_lowercase().as_str() {
        "siliconflow" => SILICONFLOW_API_KEY_USER,
        "openai" => OPENAI_API_KEY_USER,
        "anthropic" => ANTHROPIC_API_KEY_USER,
        _ => API_KEY_USER,
    }
}

fn api_key_user_for_id(key_id: &str) -> String {
    format!("{API_KEY_USER_PREFIX}{}", key_id.trim())
}

fn normalize_api_key_input(key: &str) -> String {
    let mut candidate = key
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .trim();

    if let Some((name, value)) = candidate.split_once(':') {
        if name.trim().eq_ignore_ascii_case("authorization") {
            candidate = value.trim();
        }
    }

    if candidate.len() >= 2 {
        candidate = candidate
            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
            .trim();
    }

    let mut parts = candidate.split_whitespace();
    let first = parts.next();
    let second = parts.next();
    if matches!(first, Some(value) if value.eq_ignore_ascii_case("bearer")) {
        candidate = second.unwrap_or("").trim();
    }

    candidate
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .trim()
        .to_string()
}

fn looks_like_url_value(key: &str) -> bool {
    let lower = key.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn api_key_looks_like_url_error() -> AppError {
    AppError::new(
        "api_key_looks_like_url",
        "API Key 看起来填成了 Base URL。请把 https://... 填到 Base URL，把真实密钥填到 API Key。",
        "security",
        false,
    )
    .with_details("summary=api_key_value_starts_with_url_scheme")
}

#[cfg(test)]
fn test_keyring() -> &'static Mutex<HashMap<String, String>> {
    static KEYRING: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    KEYRING.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn keyring_poisoned_error() -> AppError {
    AppError::new(
        "keyring_access_failed",
        "API key test storage is unavailable.",
        "security",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        api_key_user_for_id, api_key_user_for_provider, looks_like_url_value,
        normalize_api_key_input, store_api_key_for_id,
    };

    #[test]
    fn siliconflow_uses_dedicated_keyring_user() {
        assert_eq!(
            api_key_user_for_provider("siliconflow"),
            "model_api_key_siliconflow"
        );
        assert_eq!(api_key_user_for_provider("mimo"), "model_api_key");
        assert_eq!(api_key_user_for_provider("openai"), "model_api_key_openai");
        assert_eq!(
            api_key_user_for_provider("anthropic"),
            "model_api_key_anthropic"
        );
    }

    #[test]
    fn key_id_uses_dedicated_keyring_user() {
        assert_eq!(
            api_key_user_for_id("abc-123"),
            "model_api_key:abc-123".to_string()
        );
    }

    #[test]
    fn normalizes_api_key_input_before_storage_or_use() {
        assert_eq!(normalize_api_key_input(" sk-test "), "sk-test");
        assert_eq!(normalize_api_key_input("Bearer sk-test"), "sk-test");
        assert_eq!(
            normalize_api_key_input("Authorization: Bearer sk-test"),
            "sk-test"
        );
        assert_eq!(
            normalize_api_key_input("authorization: bearer sk-test"),
            "sk-test"
        );
        assert_eq!(
            normalize_api_key_input("Authorization: Bearer \"sk-test\"\nAccept: application/json"),
            "sk-test"
        );
        assert_eq!(normalize_api_key_input("'Bearer sk-test'"), "sk-test");
    }

    #[test]
    fn rejects_url_values_as_api_keys() {
        assert!(looks_like_url_value("https://www.su8.codes/codex/v1"));
        let err = store_api_key_for_id("url-key-test", "https://www.su8.codes/codex/v1")
            .expect_err("url should not be stored as api key");
        assert_eq!(err.code, "api_key_looks_like_url");
    }
}
