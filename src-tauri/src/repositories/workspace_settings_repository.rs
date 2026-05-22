use crate::artifacts::workspace_layout::WorkspaceLayout;
use crate::domain::settings::WorkspaceSettingsRecord;
use crate::errors::{AppError, AppResult};
use crate::repositories::db::{block_on_db, connect_workspace_db, database_error, run_migrations};
use chrono::Utc;
use sqlx::Row;

pub const APP_SETTINGS_KEY: &str = "app_settings";
pub const PRIVACY_NOTICE_KEY: &str = "privacy_notice_accepted";

pub struct WorkspaceSettingsRepository {
    layout: WorkspaceLayout,
}

impl WorkspaceSettingsRepository {
    pub fn new(layout: WorkspaceLayout) -> Self {
        Self { layout }
    }

    pub fn load_workspace_settings(&self) -> AppResult<WorkspaceSettingsRecord> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            match self.get_setting(&mut connection, APP_SETTINGS_KEY).await? {
                Some(value) => match serde_json::from_str(&value) {
                    Ok(settings) => Ok(settings),
                    Err(err) => {
                        tracing::warn!(
                            target: "settings",
                            error = %err,
                            "工作区配置无法解析，使用默认设置继续"
                        );
                        Ok(WorkspaceSettingsRecord::default())
                    }
                },
                None => Ok(WorkspaceSettingsRecord::default()),
            }
        })
    }

    pub fn save_workspace_settings(&self, settings: &WorkspaceSettingsRecord) -> AppResult<()> {
        let json = serde_json::to_string(settings).map_err(|err| {
            AppError::new(
                "settings_serialize_failed",
                "工作区配置写入失败。",
                "settings",
                true,
            )
            .with_details(err.to_string())
        })?;
        self.set_setting(APP_SETTINGS_KEY, &json)
    }

    pub fn get_privacy_notice_accepted(&self) -> AppResult<bool> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            match self
                .get_setting(&mut connection, PRIVACY_NOTICE_KEY)
                .await?
            {
                Some(value) => Ok(value == "true"),
                None => Ok(false),
            }
        })
    }

    pub fn set_privacy_notice_accepted(&self, accepted: bool) -> AppResult<()> {
        self.set_setting(PRIVACY_NOTICE_KEY, if accepted { "true" } else { "false" })
    }

    pub fn has_app_settings(&self) -> AppResult<bool> {
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            Ok(self
                .get_setting(&mut connection, APP_SETTINGS_KEY)
                .await?
                .is_some())
        })
    }

    fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        block_on_db(async {
            run_migrations(self.layout.app_db_path()).await?;
            let mut connection = connect_workspace_db(self.layout.app_db_path()).await?;
            sqlx::query(
                "INSERT INTO settings (key, value, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value)
            .bind(&now)
            .execute(&mut connection)
            .await
            .map_err(|err| database_error("settings", "settings_upsert_failed", err))?;
            Ok(())
        })
    }

    async fn get_setting(
        &self,
        connection: &mut sqlx::SqliteConnection,
        key: &str,
    ) -> AppResult<Option<String>> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(connection)
            .await
            .map_err(|err| database_error("settings", "settings_read_failed", err))?;
        Ok(row.map(|row| row.get::<String, _>("value")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::workspace_layout::WorkspaceLayout;
    use std::fs;

    fn test_repo(name: &str) -> (WorkspaceSettingsRepository, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("{name}-{}-{nonce}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let layout = WorkspaceLayout::from_root(root.clone());
        layout.ensure_base_layout().expect("layout");
        (WorkspaceSettingsRepository::new(layout), root)
    }

    #[test]
    fn workspace_settings_round_trip() {
        let (repo, root) = test_repo("slicer-ws-settings");
        let mut record = WorkspaceSettingsRecord::default();
        record.model_name = "gpt-4o".to_string();
        record.base_url = "https://api.example.com".to_string();
        repo.save_workspace_settings(&record).expect("save");
        let loaded = repo.load_workspace_settings().expect("load");
        assert_eq!(loaded.model_name, "gpt-4o");
        assert_eq!(loaded.base_url, "https://api.example.com");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn privacy_notice_persists() {
        let (repo, root) = test_repo("slicer-ws-privacy");
        assert!(!repo.get_privacy_notice_accepted().expect("read"));
        repo.set_privacy_notice_accepted(true).expect("write");
        assert!(repo.get_privacy_notice_accepted().expect("read again"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_workspace_settings_fall_back_to_defaults() {
        let (repo, root) = test_repo("slicer-ws-settings-invalid");
        repo.set_setting(APP_SETTINGS_KEY, "{not-json")
            .expect("seed");
        let loaded = repo.load_workspace_settings().expect("load default");
        assert_eq!(
            loaded.model_provider,
            WorkspaceSettingsRecord::default().model_provider
        );
        assert_eq!(loaded.model_name, "");
        let _ = fs::remove_dir_all(root);
    }
}
