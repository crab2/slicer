use crate::errors::{AppError, AppResult};

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
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::new(
            "api_key_empty",
            "API key 不能为空。",
            "security",
            true,
        ));
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
    entry.set_password(key).map_err(|err| {
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
        Ok(key) if key.trim().is_empty() => Ok(None),
        Ok(key) => Ok(Some(key)),
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

#[cfg(test)]
mod tests {
    use super::{api_key_user_for_id, api_key_user_for_provider};

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
}
