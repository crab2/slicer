use crate::domain::settings::{
    AppSettingsDto, ModelConfigurationStatusDto, PrivacyNoticeStatusDto,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::workspace_settings_repository::WorkspaceSettingsRepository;
use crate::security;
use crate::services::api_server_service::ApiServerService;
use crate::services::workspace_service::WorkspaceService;

pub struct SettingsService;

impl SettingsService {
    pub fn get_settings(workspace: &WorkspaceService) -> AppResult<AppSettingsDto> {
        let status = workspace.get_workspace_status();
        if status.status == "error" {
            return Err(status.error.unwrap_or_else(|| {
                AppError::new(
                    "settings_workspace_error",
                    "工作区状态不可用。",
                    "settings",
                    true,
                )
            }));
        }

        let mut settings = if status.status == "ready" {
            Self::load_workspace_merged_settings(workspace)?
        } else {
            workspace.settings_repository().load_app_settings()?
        };

        settings.workspace_path = status.workspace_path;
        settings.api_key_configured = Self::is_api_key_configured();
        Ok(settings)
    }

    pub fn save_settings(
        workspace: &WorkspaceService,
        api_server: &ApiServerService,
        settings: &AppSettingsDto,
    ) -> AppResult<()> {
        let mut normalized_settings = settings.clone();
        normalized_settings.model_provider = Self::normalized_model_provider(settings);
        Self::validate_settings(&normalized_settings)?;

        let status = workspace.get_workspace_status();
        if status.status != "ready" {
            return Err(AppError::new(
                "workspace_not_ready",
                "请先选择可用的工作区后再保存设置。",
                "settings",
                true,
            ));
        }

        let layout = workspace.current_layout()?;
        let ws_repo = WorkspaceSettingsRepository::new(layout);
        ws_repo.save_workspace_settings(&normalized_settings.workspace_record())?;

        let mut global = AppSettingsDto::default();
        global.libreoffice_path = normalized_settings.libreoffice_path.clone();
        workspace.settings_repository().save_app_settings(&global)?;

        tracing::info!(target: "settings", "工作区与应用设置已保存");

        api_server.reconcile(&normalized_settings)?;
        Ok(())
    }

    pub fn save_api_key(key: &str) -> AppResult<()> {
        security::store_api_key(key)
    }

    pub fn delete_api_key() -> AppResult<()> {
        security::delete_api_key()
    }

    pub fn is_api_key_configured() -> bool {
        security::has_api_key()
    }

    pub fn get_libreoffice_path(workspace: &WorkspaceService) -> AppResult<String> {
        let settings = workspace.settings_repository().load_app_settings()?;
        settings.libreoffice_path.ok_or_else(|| {
            AppError::new(
                "libreoffice_not_configured",
                "请在设置中配置 LibreOffice 路径后再导入 Office 文档。",
                "conversion",
                true,
            )
        })
    }

    pub fn get_model_configuration_status(
        workspace: &WorkspaceService,
    ) -> AppResult<ModelConfigurationStatusDto> {
        let settings = Self::get_settings(workspace)?;
        let privacy_notice_accepted = Self::privacy_notice_accepted(workspace)?;
        let missing = Self::missing_configuration_fields(&settings);
        let configured = missing.is_empty();
        let requires_privacy_notice = Self::requires_privacy_notice(&settings);

        Ok(ModelConfigurationStatusDto {
            configured,
            missing,
            privacy_notice_accepted,
            requires_privacy_notice,
        })
    }

    pub fn get_privacy_notice_status(
        workspace: &WorkspaceService,
    ) -> AppResult<PrivacyNoticeStatusDto> {
        let settings = Self::get_settings(workspace)?;
        Ok(PrivacyNoticeStatusDto {
            accepted: Self::privacy_notice_accepted(workspace)?,
            requires_notice: Self::requires_privacy_notice(&settings),
        })
    }

    pub fn accept_privacy_notice(workspace: &WorkspaceService) -> AppResult<()> {
        let settings = Self::get_settings(workspace)?;
        if !Self::is_model_configuration_complete(workspace)? {
            let missing = Self::missing_configuration_fields(&settings);
            return Err(AppError::new(
                "model_configuration_incomplete",
                "请先完成模型配置后再确认隐私提示。",
                "settings",
                true,
            )
            .with_details(format!("missing={}", missing.join(","))));
        }

        if !Self::requires_privacy_notice(&settings) {
            return Ok(());
        }

        let layout = workspace.current_layout()?;
        WorkspaceSettingsRepository::new(layout).set_privacy_notice_accepted(true)
    }

    pub fn is_model_configuration_complete(workspace: &WorkspaceService) -> AppResult<bool> {
        Ok(Self::get_model_configuration_status(workspace)?.configured)
    }

    fn load_workspace_merged_settings(workspace: &WorkspaceService) -> AppResult<AppSettingsDto> {
        let layout = workspace.current_layout()?;
        let ws_repo = WorkspaceSettingsRepository::new(layout);
        Self::maybe_migrate_global_settings(workspace, &ws_repo)?;

        let record = ws_repo.load_workspace_settings()?;
        let global = workspace.settings_repository().load_app_settings()?;

        let mut settings = AppSettingsDto::default();
        settings.apply_workspace_record(record);
        settings.libreoffice_path = global.libreoffice_path;
        Ok(settings)
    }

    fn maybe_migrate_global_settings(
        workspace: &WorkspaceService,
        ws_repo: &WorkspaceSettingsRepository,
    ) -> AppResult<()> {
        if ws_repo.has_app_settings()? {
            return Ok(());
        }

        let global = workspace.settings_repository().load_app_settings()?;
        let has_legacy = !global.model_name.is_empty()
            || !global.base_url.is_empty()
            || !global.custom_endpoint.is_empty()
            || global.model_provider != "custom"
            || global.default_image_dpi != 144
            || global.api_enabled;

        if has_legacy {
            ws_repo.save_workspace_settings(&global.workspace_record())?;
            tracing::info!(target: "settings", "已从全局 app-settings.json 迁移工作区配置到 SQLite");
        }
        Ok(())
    }

    fn privacy_notice_accepted(workspace: &WorkspaceService) -> AppResult<bool> {
        if workspace.get_workspace_status().status != "ready" {
            return Ok(false);
        }
        let layout = workspace.current_layout()?;
        WorkspaceSettingsRepository::new(layout).get_privacy_notice_accepted()
    }

    pub fn requires_privacy_notice(settings: &AppSettingsDto) -> bool {
        Self::normalized_model_provider(settings) != "local_mock"
            || !settings.base_url.trim().is_empty()
            || !settings.custom_endpoint.trim().is_empty()
    }

    fn missing_configuration_fields(settings: &AppSettingsDto) -> Vec<String> {
        let mut missing = Vec::new();
        let model_provider = Self::normalized_model_provider(settings);
        if model_provider.is_empty() {
            missing.push("model_provider".to_string());
        }
        if settings.model_name.trim().is_empty() {
            missing.push("model_name".to_string());
        }
        if !Self::provider_has_default_endpoint(&model_provider)
            && settings.base_url.trim().is_empty()
            && settings.custom_endpoint.trim().is_empty()
        {
            missing.push("endpoint".to_string());
        }
        if model_provider != "local_mock" && !settings.api_key_configured {
            missing.push("api_key".to_string());
        }
        missing
    }

    fn normalized_model_provider(settings: &AppSettingsDto) -> String {
        settings.model_provider.trim().to_string()
    }

    fn provider_has_default_endpoint(model_provider: &str) -> bool {
        matches!(
            model_provider,
            "local_mock" | "openai" | "anthropic" | "siliconflow"
        )
    }

    fn validate_settings(settings: &AppSettingsDto) -> AppResult<()> {
        if settings.default_image_dpi < 72 || settings.default_image_dpi > 300 {
            return Err(AppError::new(
                "settings_validation_failed",
                "默认图片 DPI 须在 72 到 300 之间。",
                "settings",
                true,
            ));
        }
        if settings.conversion_concurrency < 1 || settings.conversion_concurrency > 8 {
            return Err(AppError::new(
                "settings_validation_failed",
                "转换并发数须在 1 到 8 之间。",
                "settings",
                true,
            ));
        }
        if settings.analysis_concurrency < 1 || settings.analysis_concurrency > 8 {
            return Err(AppError::new(
                "settings_validation_failed",
                "分析并发数须在 1 到 8 之间。",
                "settings",
                true,
            ));
        }
        if settings.api_port < 1024 {
            return Err(AppError::new(
                "settings_validation_failed",
                "localhost API 端口须在 1024 到 65535 之间。",
                "settings",
                true,
            ));
        }
        if settings.api_enabled && settings.api_bind_address.trim() != "127.0.0.1" {
            return Err(AppError::new(
                "settings_validation_failed",
                "localhost API 当前仅允许监听 127.0.0.1。",
                "settings",
                true,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::state::ApiAppState;
    use crate::repositories::workspace_settings_repository::WorkspaceSettingsRepository;
    use crate::services::workspace_service::WorkspaceService;
    use std::fs;
    use std::sync::Arc;

    fn test_state(config_dir: &std::path::Path) -> ApiAppState {
        ApiAppState::new(Arc::new(WorkspaceService::new(config_dir.to_path_buf())))
    }

    fn test_workspace() -> (WorkspaceService, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "slicer-settings-svc-{}-{}",
            std::process::id(),
            nonce
        ));
        let _ = fs::remove_dir_all(&root);
        let config = root.join("config");
        let workspace = root.join("workspace");
        fs::create_dir_all(&config).expect("config");
        let service = WorkspaceService::new(config);
        let api = ApiServerService::new(test_state(&root.join("config")));
        let selected = service.select_workspace(workspace.to_string_lossy().into_owned(), &api);
        assert_eq!(selected.status, "ready");
        (service, root)
    }

    #[test]
    fn migrates_global_json_into_workspace_sqlite() {
        let (service, root) = test_workspace();
        let config_dir = root.join("config");
        fs::write(
            config_dir.join("app-settings.json"),
            r#"{"model_provider":"custom","base_url":"https://legacy.example","custom_endpoint":"","model_name":"legacy-model","default_image_dpi":144,"conversion_concurrency":2,"analysis_concurrency":2,"api_enabled":false,"api_bind_address":"127.0.0.1","api_port":17321}"#,
        )
        .expect("legacy global settings");

        let loaded = SettingsService::get_settings(&service).expect("get");
        assert_eq!(loaded.model_name, "legacy-model");

        let layout = service.current_layout().expect("layout");
        assert!(WorkspaceSettingsRepository::new(layout)
            .has_app_settings()
            .expect("has settings"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_settings_keeps_model_fields_out_of_global_json() {
        let (service, root) = test_workspace();
        let mut settings = AppSettingsDto::default();
        settings.libreoffice_path =
            Some("C:/Program Files/LibreOffice/program/soffice.exe".to_string());
        settings.model_name = "sensitive-model-name".to_string();
        settings.base_url = "https://models.example.com".to_string();
        settings.custom_endpoint = "/v1/chat/completions".to_string();

        SettingsService::save_settings(
            &service,
            &ApiServerService::new(test_state(&root.join("config"))),
            &settings,
        )
        .expect("save");

        let global_text = fs::read_to_string(root.join("config").join("app-settings.json"))
            .expect("global settings file");
        assert!(global_text.contains("libreoffice_path"));
        assert!(!global_text.contains("sensitive-model-name"));
        assert!(!global_text.contains("models.example.com"));

        let layout = service.current_layout().expect("layout");
        let workspace_record = WorkspaceSettingsRepository::new(layout)
            .load_workspace_settings()
            .expect("workspace settings");
        let workspace_json = serde_json::to_string(&workspace_record).expect("serialize");
        for secret_word in ["api_key", "authorization", "token", "secret"] {
            assert!(!workspace_json.to_lowercase().contains(secret_word));
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn model_configuration_status_detects_missing_fields() {
        let (service, root) = test_workspace();
        let status = SettingsService::get_model_configuration_status(&service).expect("status");
        assert!(!status.configured);
        assert!(status.missing.contains(&"model_name".to_string()));
        assert!(status.missing.contains(&"endpoint".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn siliconflow_uses_default_endpoint_configuration() {
        let mut settings = AppSettingsDto::default();
        settings.model_provider = "siliconflow".to_string();
        settings.model_name = "zai-org/GLM-4.6V".to_string();
        settings.api_key_configured = true;
        settings.base_url.clear();
        settings.custom_endpoint.clear();

        let missing = SettingsService::missing_configuration_fields(&settings);

        assert!(!missing.contains(&"endpoint".to_string()));
    }

    #[test]
    fn local_mock_does_not_require_api_key() {
        let mut settings = AppSettingsDto::default();
        settings.model_provider = "local_mock".to_string();
        settings.model_name = "mock".to_string();
        settings.base_url = "http://localhost".to_string();
        settings.api_key_configured = false;
        let missing = SettingsService::missing_configuration_fields(&settings);
        assert!(!missing.contains(&"api_key".to_string()));
    }

    #[test]
    fn privacy_notice_requirement_matches_provider_or_endpoint_rule() {
        let mut settings = AppSettingsDto::default();
        settings.model_provider = "custom".to_string();
        settings.base_url.clear();
        settings.custom_endpoint.clear();
        assert!(SettingsService::requires_privacy_notice(&settings));

        settings.model_provider = "local_mock".to_string();
        assert!(!SettingsService::requires_privacy_notice(&settings));

        settings.base_url = "https://remote.example.com".to_string();
        assert!(SettingsService::requires_privacy_notice(&settings));
    }

    #[test]
    fn accept_privacy_notice_requires_complete_model_configuration() {
        let (service, root) = test_workspace();
        let err = SettingsService::accept_privacy_notice(&service).expect_err("incomplete config");
        assert_eq!(err.code, "model_configuration_incomplete");

        let layout = service.current_layout().expect("layout");
        assert!(!WorkspaceSettingsRepository::new(layout)
            .get_privacy_notice_accepted()
            .expect("privacy flag"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_settings_rejects_non_localhost_api_exposure() {
        let mut settings = AppSettingsDto::default();
        settings.api_port = 0;
        assert!(SettingsService::validate_settings(&settings).is_err());

        settings.api_port = 17321;
        settings.api_enabled = true;
        settings.api_bind_address = "0.0.0.0".to_string();
        assert!(SettingsService::validate_settings(&settings).is_err());
    }
}
