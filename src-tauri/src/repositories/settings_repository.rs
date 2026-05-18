use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct BootstrapSettings {
    pub last_workspace_path: Option<String>,
}

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

    fn bootstrap_path(&self) -> PathBuf {
        self.config_dir.join("bootstrap-workspace.json")
    }

    fn write_json<T: Serialize>(&self, path: PathBuf, value: &T) -> AppResult<()> {
        let tmp_path = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(value).map_err(|err| {
            AppError::new(
                "bootstrap_serialize_failed",
                "工作区配置写入失败。",
                "restore",
                true,
            )
            .with_details(err.to_string())
        })?;
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp_path)
                .map_err(|err| AppError::io("restore", "bootstrap_write_failed", err))?;
            file.write_all(json.as_bytes())
                .map_err(|err| AppError::io("restore", "bootstrap_write_failed", err))?;
        }
        fs::rename(&tmp_path, &path)
            .map_err(|err| AppError::io("restore", "bootstrap_replace_failed", err))
    }
}
