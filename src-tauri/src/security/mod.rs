use crate::errors::{AppError, AppResult};

const SERVICE_NAME: &str = "slicer";
const API_KEY_USER: &str = "model_api_key";

/// Store API key in OS credential storage (keyring).
pub fn store_api_key(key: &str) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::new(
            "api_key_empty",
            "API key 不能为空。",
            "security",
            true,
        ));
    }

    let entry = keyring::Entry::new(SERVICE_NAME, API_KEY_USER).map_err(|err| {
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

/// Read API key from OS credential storage.
pub fn read_api_key() -> AppResult<Option<String>> {
    let entry = keyring::Entry::new(SERVICE_NAME, API_KEY_USER).map_err(|err| {
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
    let entry = keyring::Entry::new(SERVICE_NAME, API_KEY_USER).map_err(|err| {
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

/// Check if API key is configured (without reading the actual key).
pub fn has_api_key() -> bool {
    read_api_key().ok().flatten().is_some()
}
