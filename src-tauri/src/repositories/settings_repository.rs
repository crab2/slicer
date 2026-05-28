use crate::domain::settings::{ApiKeyListDto, ApiKeyRecordDto, AppSettingsDto};
use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct BootstrapSettings {
    pub last_workspace_path: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
struct AppSettingsFile {
    pub libreoffice_path: Option<String>,
}

#[derive(Clone)]
pub struct SettingsRepository {
    config_dir: PathBuf,
}

impl SettingsRepository {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub fn load_bootstrap(&self) -> AppResult<BootstrapSettings> {
        let path = self.bootstrap_path();
        if !path.exists() {
            return Ok(BootstrapSettings::default());
        }
        let text = fs::read_to_string(&path).map_err(|err| {
            AppError::io("restore", "bootstrap_read_failed", err)
                .with_details(path.display().to_string())
        })?;
        serde_json::from_str(&text).map_err(|err| {
            AppError::new(
                "bootstrap_parse_failed",
                "最近工作区配置无法读取，请重新选择工作区。",
                "restore",
                true,
            )
            .with_details(err.to_string())
        })
    }

    pub fn save_last_workspace(&self, workspace_path: String) -> AppResult<()> {
        fs::create_dir_all(&self.config_dir)
            .map_err(|err| AppError::io("restore", "bootstrap_dir_failed", err))?;
        self.write_json(
            self.bootstrap_path(),
            &BootstrapSettings {
                last_workspace_path: Some(workspace_path),
            },
        )
    }

    pub fn load_app_settings(&self) -> AppResult<AppSettingsDto> {
        let path = self.app_settings_path();
        if !path.exists() {
            return Ok(AppSettingsDto::default());
        }
        let text = fs::read_to_string(&path).map_err(|err| {
            AppError::io("settings", "settings_read_failed", err)
                .with_details(path.display().to_string())
        })?;
        serde_json::from_str(&text).map_err(|err| {
            AppError::new(
                "settings_parse_failed",
                "应用配置无法读取，已恢复默认设置。",
                "settings",
                true,
            )
            .with_details(err.to_string())
        })
    }

    pub fn save_app_settings(&self, settings: &AppSettingsDto) -> AppResult<()> {
        fs::create_dir_all(&self.config_dir)
            .map_err(|err| AppError::io("settings", "settings_dir_failed", err))?;
        self.write_json(
            self.app_settings_path(),
            &AppSettingsFile {
                libreoffice_path: settings.libreoffice_path.clone(),
            },
        )
    }

    pub fn load_api_key_list(&self) -> AppResult<ApiKeyListDto> {
        let path = self.api_keys_path();
        if !path.exists() {
            return Ok(ApiKeyListDto { keys: Vec::new() });
        }
        let text = fs::read_to_string(&path).map_err(|err| {
            AppError::io("settings", "api_keys_read_failed", err)
                .with_details(path.display().to_string())
        })?;
        serde_json::from_str(&text).map_err(|err| {
            AppError::new(
                "api_keys_parse_failed",
                "API key list could not be read.",
                "settings",
                true,
            )
            .with_details(err.to_string())
        })
    }

    pub fn save_api_key_list(&self, keys: &[ApiKeyRecordDto]) -> AppResult<()> {
        fs::create_dir_all(&self.config_dir)
            .map_err(|err| AppError::io("settings", "settings_dir_failed", err))?;
        self.write_json(
            self.api_keys_path(),
            &ApiKeyListDto {
                keys: keys.to_vec(),
            },
        )
    }

    fn bootstrap_path(&self) -> PathBuf {
        self.config_dir.join("bootstrap-workspace.json")
    }

    fn app_settings_path(&self) -> PathBuf {
        self.config_dir.join("app-settings.json")
    }

    fn api_keys_path(&self) -> PathBuf {
        self.config_dir.join("api-keys.json")
    }

    fn write_json<T: Serialize>(&self, path: PathBuf, value: &T) -> AppResult<()> {
        let tmp_path = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(value).map_err(|err| {
            AppError::new("serialize_failed", "配置写入失败。", "settings", true)
                .with_details(err.to_string())
        })?;
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp_path)
                .map_err(|err| AppError::io("settings", "settings_write_failed", err))?;
            file.write_all(json.as_bytes())
                .map_err(|err| AppError::io("settings", "settings_write_failed", err))?;
        }
        fs::rename(&tmp_path, &path)
            .map_err(|err| AppError::io("settings", "settings_replace_failed", err))
    }
}
