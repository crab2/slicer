use crate::domain::settings::{
    ApiKeyListDto, ApiKeyRecordDto, AppSettingsDto, ModelConfigurationStatusDto, ModelProfileDto,
    ModelProfileListDto, ModelProfileUpsertRequestDto, PrivacyNoticeStatusDto,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::workspace_settings_repository::WorkspaceSettingsRepository;
use crate::security;
use crate::services::api_server_service::ApiServerService;
use crate::services::workspace_service::WorkspaceService;
use chrono::Utc;
use uuid::Uuid;

pub struct SettingsService;

impl SettingsService {
    const MAX_MODEL_PROFILES: usize = 10;

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

        if status.status == "ready" {
            Self::normalize_model_profiles(workspace, &mut settings, true)?;
        }
        settings.model_provider = Self::normalized_model_provider(&settings);
        settings.workspace_path = status.workspace_path;
        if !settings
            .model_profiles
            .iter()
            .any(|profile| profile.is_active)
        {
            settings.api_key_configured =
                Self::active_api_key_configured_for_provider(workspace, &settings.model_provider)?;
        }
        Ok(settings)
    }

    pub fn save_settings(
        workspace: &WorkspaceService,
        api_server: &ApiServerService,
        settings: &AppSettingsDto,
    ) -> AppResult<()> {
        let mut normalized_settings = settings.clone();
        normalized_settings.model_provider = Self::normalized_model_provider(settings);
        Self::normalize_model_profiles(workspace, &mut normalized_settings, false)?;
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

    pub fn list_model_profiles(workspace: &WorkspaceService) -> AppResult<ModelProfileListDto> {
        let settings = Self::get_settings(workspace)?;
        Ok(ModelProfileListDto {
            profiles: settings.model_profiles,
            max_profiles: Self::MAX_MODEL_PROFILES,
        })
    }

    pub fn upsert_model_profile(
        workspace: &WorkspaceService,
        api_server: &ApiServerService,
        request: &ModelProfileUpsertRequestDto,
    ) -> AppResult<ModelProfileListDto> {
        let mut settings = Self::get_settings(workspace)?;
        let mut profiles = settings.model_profiles.clone();
        let provider = Self::normalized_provider_name(&request.provider);
        Self::validate_provider_name(&provider)?;

        let now = Utc::now().to_rfc3339();
        let existing_index = request.profile_id.as_ref().and_then(|id| {
            profiles
                .iter()
                .position(|profile| &profile.profile_id == id)
        });
        if existing_index.is_none() && profiles.len() >= Self::MAX_MODEL_PROFILES {
            return Err(AppError::new(
                "model_profile_limit_reached",
                "最多只能保存 10 个模型配置。",
                "settings",
                true,
            ));
        }

        let api_key = request.api_key.as_deref().unwrap_or("").trim();
        let existing = existing_index.and_then(|index| profiles.get(index).cloned());
        let mut key_id = existing.as_ref().and_then(|profile| profile.key_id.clone());
        if !api_key.is_empty() {
            let next_key_id = key_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            security::store_api_key_for_id(&next_key_id, api_key)?;
            key_id = Some(next_key_id);
        }

        if Self::provider_requires_api_key(&provider) && key_id.is_none() {
            return Err(AppError::new(
                "api_key_missing",
                "请为模型配置填写 API Key。",
                "settings",
                true,
            ));
        }

        let key_label = Self::normalized_profile_key_label(
            &request.api_key_label,
            &request.label,
            &request.model_name,
            &provider,
        );
        if let Some(id) = key_id.as_deref() {
            Self::upsert_profile_api_key_record(workspace, &provider, id, &key_label)?;
        }

        let label = Self::normalized_profile_label(&request.label, &request.model_name, &provider);
        let should_activate = request.activate
            || profiles.is_empty()
            || existing.as_ref().is_some_and(|profile| profile.is_active);
        let profile = ModelProfileDto {
            profile_id: existing
                .as_ref()
                .map(|profile| profile.profile_id.clone())
                .or_else(|| request.profile_id.clone())
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            label,
            provider,
            base_url: request.base_url.trim().to_string(),
            custom_endpoint: request.custom_endpoint.trim().to_string(),
            model_name: request.model_name.trim().to_string(),
            key_id,
            key_label: Some(key_label),
            api_key_configured: false,
            is_active: should_activate,
            created_at: existing
                .as_ref()
                .map(|profile| profile.created_at.clone())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        };

        if let Some(index) = existing_index {
            profiles[index] = profile;
        } else {
            profiles.push(profile);
        }
        if should_activate {
            let active_id = profiles
                .iter()
                .find(|candidate| candidate.is_active)
                .map(|candidate| candidate.profile_id.clone());
            if let Some(active_id) = active_id {
                for item in &mut profiles {
                    item.is_active = item.profile_id == active_id;
                }
            }
        }

        settings.model_profiles = profiles;
        Self::normalize_model_profiles(workspace, &mut settings, false)?;
        Self::save_settings(workspace, api_server, &settings)?;
        Self::list_model_profiles(workspace)
    }

    pub fn activate_model_profile(
        workspace: &WorkspaceService,
        api_server: &ApiServerService,
        profile_id: &str,
    ) -> AppResult<ModelProfileListDto> {
        let mut settings = Self::get_settings(workspace)?;
        if !settings
            .model_profiles
            .iter()
            .any(|profile| profile.profile_id == profile_id)
        {
            return Err(AppError::new(
                "model_profile_not_found",
                "模型配置不存在。",
                "settings",
                true,
            ));
        }
        for profile in &mut settings.model_profiles {
            profile.is_active = profile.profile_id == profile_id;
        }
        Self::normalize_model_profiles(workspace, &mut settings, false)?;
        Self::save_settings(workspace, api_server, &settings)?;
        Self::list_model_profiles(workspace)
    }

    pub fn delete_model_profile(
        workspace: &WorkspaceService,
        api_server: &ApiServerService,
        profile_id: &str,
    ) -> AppResult<ModelProfileListDto> {
        let mut settings = Self::get_settings(workspace)?;
        let removed = settings
            .model_profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "model_profile_not_found",
                    "模型配置不存在。",
                    "settings",
                    true,
                )
            })?;
        settings
            .model_profiles
            .retain(|profile| profile.profile_id != profile_id);
        if let Some(key_id) = removed.key_id.as_deref() {
            if !settings
                .model_profiles
                .iter()
                .any(|profile| profile.key_id.as_deref() == Some(key_id))
            {
                let _ = security::delete_api_key_for_id(key_id);
                Self::remove_api_key_record_by_id(workspace, key_id)?;
            }
        }
        if removed.is_active {
            if let Some(first) = settings.model_profiles.first_mut() {
                first.is_active = true;
            } else {
                settings.model_provider = "openai".to_string();
                settings.base_url.clear();
                settings.custom_endpoint.clear();
                settings.model_name.clear();
                settings.api_key_configured = false;
            }
        }
        Self::normalize_model_profiles(workspace, &mut settings, false)?;
        Self::save_settings(workspace, api_server, &settings)?;
        Self::list_model_profiles(workspace)
    }

    pub fn save_api_key(key: &str) -> AppResult<()> {
        security::store_api_key(key)
    }

    pub fn save_api_key_for_provider(provider: &str, key: &str) -> AppResult<()> {
        security::store_api_key_for_provider(provider, key)
    }

    pub fn add_api_key(
        workspace: &WorkspaceService,
        provider: &str,
        label: &str,
        key: &str,
        activate: bool,
    ) -> AppResult<ApiKeyListDto> {
        let provider = Self::normalized_provider_name(provider);
        Self::validate_provider_name(&provider)?;
        let key_id = Uuid::new_v4().to_string();
        security::store_api_key_for_id(&key_id, key)?;

        let mut list = workspace.settings_repository().load_api_key_list()?.keys;
        let now = Utc::now().to_rfc3339();
        let should_activate = activate || !list.iter().any(|item| item.provider == provider);
        if should_activate {
            for item in &mut list {
                if item.provider == provider {
                    item.is_active = false;
                    item.updated_at = now.clone();
                }
            }
            security::store_api_key_for_provider(&provider, key)?;
        }
        let normalized_label = label.trim();
        list.push(ApiKeyRecordDto {
            key_id,
            provider,
            label: if normalized_label.is_empty() {
                format!("API Key {}", list.len() + 1)
            } else {
                normalized_label.to_string()
            },
            is_active: should_activate,
            created_at: now.clone(),
            updated_at: now,
        });
        workspace.settings_repository().save_api_key_list(&list)?;
        Ok(ApiKeyListDto { keys: list })
    }

    pub fn list_api_keys(workspace: &WorkspaceService) -> AppResult<ApiKeyListDto> {
        Self::migrated_api_key_list(workspace)
    }

    pub fn activate_api_key(
        workspace: &WorkspaceService,
        provider: &str,
        key_id: &str,
    ) -> AppResult<ApiKeyListDto> {
        let provider = Self::normalized_provider_name(provider);
        Self::validate_provider_name(&provider)?;
        let mut list = Self::migrated_api_key_list(workspace)?.keys;
        if !list
            .iter()
            .any(|item| item.provider == provider && item.key_id == key_id)
        {
            return Err(
                AppError::new("api_key_not_found", "API key not found.", "settings", true)
                    .with_details(format!("provider={provider}; key_id={key_id}")),
            );
        }
        let active_secret = security::read_api_key_for_id(key_id)?.ok_or_else(|| {
            AppError::new(
                "api_key_secret_missing",
                "API key secret is missing from system credential storage.",
                "settings",
                true,
            )
            .with_details(format!("provider={provider}; key_id={key_id}"))
        })?;
        if active_secret.trim().is_empty() {
            return Err(AppError::new(
                "api_key_secret_missing",
                "API key secret is missing from system credential storage.",
                "settings",
                true,
            )
            .with_details(format!("provider={provider}; key_id={key_id}")));
        }
        security::store_api_key_for_provider(&provider, &active_secret)?;
        let now = Utc::now().to_rfc3339();
        for item in &mut list {
            if item.provider == provider {
                item.is_active = item.key_id == key_id;
                item.updated_at = now.clone();
            }
        }
        workspace.settings_repository().save_api_key_list(&list)?;
        Ok(ApiKeyListDto { keys: list })
    }

    pub fn delete_api_key_record(
        workspace: &WorkspaceService,
        provider: &str,
        key_id: &str,
    ) -> AppResult<ApiKeyListDto> {
        let provider = Self::normalized_provider_name(provider);
        Self::validate_provider_name(&provider)?;
        let mut list = Self::migrated_api_key_list(workspace)?.keys;
        let removed = list
            .iter()
            .find(|item| item.provider == provider && item.key_id == key_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new("api_key_not_found", "API key not found.", "settings", true)
                    .with_details(format!("provider={provider}; key_id={key_id}"))
            })?;
        list.retain(|item| !(item.provider == provider && item.key_id == key_id));
        security::delete_api_key_for_id(&removed.key_id)?;

        if removed.is_active {
            if let Some(next) = list.iter_mut().find(|item| item.provider == provider) {
                next.is_active = true;
                next.updated_at = Utc::now().to_rfc3339();
                if let Some(next_secret) = security::read_api_key_for_id(&next.key_id)? {
                    security::store_api_key_for_provider(&provider, &next_secret)?;
                } else {
                    security::delete_api_key_for_provider(&provider)?;
                }
            } else {
                security::delete_api_key_for_provider(&provider)?;
            }
        }

        workspace.settings_repository().save_api_key_list(&list)?;
        Ok(ApiKeyListDto { keys: list })
    }

    pub fn delete_api_key() -> AppResult<()> {
        security::delete_api_key()
    }

    pub fn delete_api_key_for_provider(provider: &str) -> AppResult<()> {
        security::delete_api_key_for_provider(provider)
    }

    pub fn read_active_api_key_for_provider(provider: &str) -> AppResult<Option<String>> {
        let provider = Self::normalized_provider_name(provider);
        security::read_api_key_for_provider(&provider)
    }

    pub fn read_active_api_key_for_provider_from_workspace(
        workspace: &WorkspaceService,
        provider: &str,
    ) -> AppResult<Option<String>> {
        let provider = Self::normalized_provider_name(provider);
        Self::validate_provider_name(&provider)?;

        if let Some(active) = Self::migrated_api_key_list(workspace)?
            .keys
            .into_iter()
            .find(|item| item.provider == provider && item.is_active)
        {
            let Some(secret) = security::read_api_key_for_id(&active.key_id)? else {
                return Err(AppError::new(
                    "api_key_secret_missing",
                    "当前启用的 API Key 密钥内容缺失，请删除后重新新增。",
                    "settings",
                    true,
                )
                .with_details(format!("provider={}; key_id={}", provider, active.key_id)));
            };
            security::store_api_key_for_provider(&provider, &secret)?;
            return Ok(Some(secret));
        }

        security::read_api_key_for_provider(&provider)
    }

    pub fn read_api_key_for_model_profile(
        workspace: &WorkspaceService,
        profile_id: &str,
    ) -> AppResult<Option<String>> {
        let settings = Self::get_settings(workspace)?;
        let Some(profile) = settings
            .model_profiles
            .iter()
            .find(|candidate| candidate.profile_id == profile_id)
        else {
            return Err(AppError::new(
                "model_profile_not_found",
                "模型配置不存在。",
                "settings",
                true,
            ));
        };
        let Some(key_id) = profile.key_id.as_deref() else {
            return Ok(None);
        };
        security::read_api_key_for_id(key_id)
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

    fn normalize_model_profiles(
        workspace: &WorkspaceService,
        settings: &mut AppSettingsDto,
        _persist_changes: bool,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        for profile in &mut settings.model_profiles {
            profile.provider = Self::normalized_provider_name(&profile.provider);
            profile.label = profile.label.trim().to_string();
            profile.base_url = profile.base_url.trim().to_string();
            profile.custom_endpoint = profile.custom_endpoint.trim().to_string();
            profile.model_name = profile.model_name.trim().to_string();
            if profile.profile_id.trim().is_empty() {
                profile.profile_id = Uuid::new_v4().to_string();
            }
            if profile.label.is_empty() {
                profile.label =
                    Self::normalized_profile_label("", &profile.model_name, &profile.provider);
            }
            if profile.created_at.is_empty() {
                profile.created_at = now.clone();
            }
            if profile.updated_at.is_empty() {
                profile.updated_at = profile.created_at.clone();
            }
            profile.api_key_configured =
                Self::profile_api_key_configured(profile.key_id.as_deref())?;
        }

        if settings.model_profiles.is_empty() && !settings.model_name.trim().is_empty() {
            settings
                .model_profiles
                .push(Self::legacy_profile_from_settings(workspace, settings)?);
        }

        let mut active_seen = false;
        for profile in &mut settings.model_profiles {
            if profile.is_active {
                if active_seen {
                    profile.is_active = false;
                } else {
                    active_seen = true;
                }
            }
        }
        if !active_seen {
            if let Some(first) = settings.model_profiles.first_mut() {
                first.is_active = true;
            }
        }

        if let Some(active) = settings
            .model_profiles
            .iter()
            .find(|profile| profile.is_active)
            .cloned()
        {
            settings.model_provider = active.provider.clone();
            settings.base_url = active.base_url.clone();
            settings.custom_endpoint = active.custom_endpoint.clone();
            settings.model_name = active.model_name.clone();
            settings.api_key_configured = active.api_key_configured;
            if let Some(key_id) = active.key_id.as_deref() {
                if let Some(secret) = security::read_api_key_for_id(key_id)? {
                    security::store_api_key_for_provider(&active.provider, &secret)?;
                }
                Self::set_active_api_key_record(workspace, &active.provider, key_id)?;
            }
        }

        Ok(())
    }

    fn legacy_profile_from_settings(
        workspace: &WorkspaceService,
        settings: &AppSettingsDto,
    ) -> AppResult<ModelProfileDto> {
        let provider = Self::normalized_model_provider(settings);
        let active_key = Self::migrated_api_key_list(workspace)?
            .keys
            .into_iter()
            .find(|key| key.provider == provider && key.is_active);
        let key_id = active_key.as_ref().map(|key| key.key_id.clone());
        let key_label = active_key.as_ref().map(|key| key.label.clone());
        let now = Utc::now().to_rfc3339();
        Ok(ModelProfileDto {
            profile_id: "legacy-active-model".to_string(),
            label: Self::normalized_profile_label("", &settings.model_name, &provider),
            provider,
            base_url: settings.base_url.trim().to_string(),
            custom_endpoint: settings.custom_endpoint.trim().to_string(),
            model_name: settings.model_name.trim().to_string(),
            api_key_configured: Self::profile_api_key_configured(key_id.as_deref())?,
            key_id,
            key_label,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    fn profile_api_key_configured(key_id: Option<&str>) -> AppResult<bool> {
        let Some(key_id) = key_id else {
            return Ok(false);
        };
        match security::read_api_key_for_id(key_id) {
            Ok(secret) => Ok(secret.is_some()),
            Err(err) if err.code == "api_key_looks_like_url" => Ok(false),
            Err(err) => Err(err),
        }
    }

    fn normalized_profile_label(label: &str, model_name: &str, provider: &str) -> String {
        let label = label.trim();
        if !label.is_empty() {
            return label.to_string();
        }
        let model_name = model_name.trim();
        if !model_name.is_empty() {
            return model_name.to_string();
        }
        format!("{} model", provider)
    }

    fn normalized_profile_key_label(
        key_label: &str,
        label: &str,
        model_name: &str,
        provider: &str,
    ) -> String {
        let key_label = key_label.trim();
        if !key_label.is_empty() {
            return key_label.to_string();
        }
        format!(
            "{} API Key",
            Self::normalized_profile_label(label, model_name, provider)
        )
    }

    fn upsert_profile_api_key_record(
        workspace: &WorkspaceService,
        provider: &str,
        key_id: &str,
        label: &str,
    ) -> AppResult<()> {
        let mut list = Self::migrated_api_key_list(workspace)?.keys;
        let now = Utc::now().to_rfc3339();
        if let Some(existing) = list.iter_mut().find(|item| item.key_id == key_id) {
            existing.provider = provider.to_string();
            existing.label = label.to_string();
            existing.updated_at = now;
        } else {
            list.push(ApiKeyRecordDto {
                key_id: key_id.to_string(),
                provider: provider.to_string(),
                label: label.to_string(),
                is_active: false,
                created_at: now.clone(),
                updated_at: now,
            });
        }
        workspace.settings_repository().save_api_key_list(&list)
    }

    fn remove_api_key_record_by_id(workspace: &WorkspaceService, key_id: &str) -> AppResult<()> {
        let mut list = Self::migrated_api_key_list(workspace)?.keys;
        list.retain(|item| item.key_id != key_id);
        workspace.settings_repository().save_api_key_list(&list)
    }

    fn set_active_api_key_record(
        workspace: &WorkspaceService,
        provider: &str,
        key_id: &str,
    ) -> AppResult<()> {
        let mut list = Self::migrated_api_key_list(workspace)?.keys;
        let now = Utc::now().to_rfc3339();
        for item in &mut list {
            if item.provider == provider {
                item.is_active = item.key_id == key_id;
                item.updated_at = now.clone();
            }
        }
        workspace.settings_repository().save_api_key_list(&list)
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
            || !matches!(
                global.model_provider.as_str(),
                "siliconflow" | "mimo" | "openai" | "anthropic"
            )
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
        #[cfg(test)]
        if Self::normalized_model_provider(settings) == "local_mock" {
            return false;
        }
        true
    }

    fn missing_configuration_fields(settings: &AppSettingsDto) -> Vec<String> {
        let mut missing = Vec::new();
        let model_provider = Self::normalized_model_provider(settings);
        if !Self::is_supported_provider(&model_provider) {
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
        if Self::provider_requires_api_key(&model_provider) && !settings.api_key_configured {
            missing.push("api_key".to_string());
        }
        missing
    }

    fn active_api_key_configured_for_provider(
        workspace: &WorkspaceService,
        provider: &str,
    ) -> AppResult<bool> {
        let provider = Self::normalized_provider_name(provider);
        if let Some(active) = Self::migrated_api_key_list(workspace)?
            .keys
            .into_iter()
            .find(|item| item.provider == provider && item.is_active)
        {
            return match security::read_api_key_for_id(&active.key_id) {
                Ok(secret) => Ok(secret.is_some()),
                Err(err) if err.code == "api_key_looks_like_url" => Ok(false),
                Err(err) => Err(err),
            };
        }
        match security::read_api_key_for_provider(&provider) {
            Ok(secret) => Ok(secret.is_some()),
            Err(err) if err.code == "api_key_looks_like_url" => Ok(false),
            Err(err) => Err(err),
        }
    }

    fn migrated_api_key_list(workspace: &WorkspaceService) -> AppResult<ApiKeyListDto> {
        let mut list = workspace.settings_repository().load_api_key_list()?.keys;
        let mut changed = false;
        for provider in ["siliconflow", "mimo"] {
            if list.iter().any(|item| item.provider == provider) {
                continue;
            }
            if let Some(legacy_key) = security::read_api_key_for_provider(provider)? {
                let key_id = Self::legacy_key_id_for_provider(provider);
                security::store_api_key_for_id(&key_id, &legacy_key)?;
                let now = Utc::now().to_rfc3339();
                list.push(ApiKeyRecordDto {
                    key_id,
                    provider: provider.to_string(),
                    label: format!("Legacy {provider} key"),
                    is_active: true,
                    created_at: now.clone(),
                    updated_at: now,
                });
                changed = true;
            }
        }
        Self::normalize_active_api_keys(&mut list);
        if changed {
            workspace.settings_repository().save_api_key_list(&list)?;
        }
        Ok(ApiKeyListDto { keys: list })
    }

    fn normalize_active_api_keys(list: &mut [ApiKeyRecordDto]) {
        for provider in ["siliconflow", "mimo", "openai", "anthropic"] {
            let mut first_for_provider = None;
            let mut active_seen = false;
            for (index, item) in list.iter_mut().enumerate() {
                if item.provider != provider {
                    continue;
                }
                if first_for_provider.is_none() {
                    first_for_provider = Some(index);
                }
                if item.is_active {
                    if active_seen {
                        item.is_active = false;
                    } else {
                        active_seen = true;
                    }
                }
            }
            if !active_seen {
                if let Some(index) = first_for_provider {
                    list[index].is_active = true;
                }
            }
        }
    }

    fn normalized_provider_name(provider: &str) -> String {
        let mut settings = AppSettingsDto::default();
        settings.model_provider = provider.trim().to_string();
        Self::normalized_model_provider(&settings)
    }

    fn validate_provider_name(provider: &str) -> AppResult<()> {
        if Self::is_supported_provider(provider) {
            Ok(())
        } else {
            Err(AppError::new(
                "settings_validation_failed",
                "Unsupported model provider.",
                "settings",
                true,
            )
            .with_details(format!("provider={provider}")))
        }
    }

    fn legacy_key_id_for_provider(provider: &str) -> String {
        format!("legacy-{provider}")
    }

    pub fn normalized_model_provider(settings: &AppSettingsDto) -> String {
        let provider = settings.model_provider.trim();
        if matches!(provider, "mimo" | "xiaomi" | "xiaomimimo")
            || (provider == "openai" && Self::looks_like_mimo_settings(settings))
        {
            "mimo".to_string()
        } else {
            provider.to_string()
        }
    }

    fn looks_like_mimo_settings(settings: &AppSettingsDto) -> bool {
        let endpoint = if settings.custom_endpoint.trim().is_empty() {
            settings.base_url.trim()
        } else {
            settings.custom_endpoint.trim()
        }
        .to_ascii_lowercase();
        endpoint.contains("xiaomimimo.com")
            || settings
                .model_name
                .trim()
                .to_ascii_lowercase()
                .starts_with("mimo-")
    }

    fn is_supported_provider(model_provider: &str) -> bool {
        #[cfg(test)]
        if model_provider == "local_mock" {
            return true;
        }
        matches!(
            model_provider,
            "siliconflow" | "mimo" | "openai" | "anthropic"
        )
    }

    fn provider_requires_api_key(model_provider: &str) -> bool {
        #[cfg(test)]
        if model_provider == "local_mock" {
            return false;
        }
        let _ = model_provider;
        true
    }

    fn provider_has_default_endpoint(model_provider: &str) -> bool {
        #[cfg(test)]
        if model_provider == "local_mock" {
            return true;
        }
        matches!(
            model_provider,
            "siliconflow" | "mimo" | "openai" | "anthropic"
        )
    }

    fn validate_settings(settings: &AppSettingsDto) -> AppResult<()> {
        let model_provider = Self::normalized_model_provider(settings);
        if !Self::is_supported_provider(&model_provider) {
            return Err(AppError::new(
                "settings_validation_failed",
                "模型 Provider 仅支持硅基流动、MiMo、OpenAI 和 Anthropic。",
                "settings",
                true,
            )
            .with_details(format!("provider={model_provider}")));
        }
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
            r#"{"model_provider":"openai","base_url":"https://legacy.example","custom_endpoint":"","model_name":"legacy-model","default_image_dpi":144,"conversion_concurrency":2,"analysis_concurrency":2,"api_enabled":false,"api_bind_address":"127.0.0.1","api_port":17321}"#,
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
        assert!(status.missing.contains(&"api_key".to_string()));

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
    fn unsupported_provider_is_missing_configuration() {
        let mut settings = AppSettingsDto::default();
        settings.model_provider = "custom".to_string();
        settings.model_name = "mock".to_string();
        settings.base_url = "http://localhost".to_string();
        settings.api_key_configured = true;
        let missing = SettingsService::missing_configuration_fields(&settings);
        assert!(missing.contains(&"model_provider".to_string()));
    }

    #[test]
    fn workspace_active_api_key_read_normalizes_bearer_prefix() {
        let (service, root) = test_workspace();
        let keys = SettingsService::add_api_key(
            &service,
            "openai",
            "test openai",
            "Authorization: Bearer sk-test-openai",
            true,
        )
        .expect("add key");
        assert!(keys
            .keys
            .iter()
            .any(|key| key.provider == "openai" && key.is_active));

        let secret =
            SettingsService::read_active_api_key_for_provider_from_workspace(&service, "openai")
                .expect("read active")
                .expect("secret");

        assert_eq!(secret, "sk-test-openai");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn model_profiles_can_be_added_activated_and_deleted() {
        let (service, root) = test_workspace();
        let api = ApiServerService::new(test_state(&root.join("config")));

        let first = SettingsService::upsert_model_profile(
            &service,
            &api,
            &ModelProfileUpsertRequestDto {
                profile_id: None,
                label: "first".to_string(),
                provider: "openai".to_string(),
                base_url: "https://api.one.example".to_string(),
                custom_endpoint: String::new(),
                model_name: "gpt-one".to_string(),
                api_key_label: "first key".to_string(),
                api_key: Some("sk-first".to_string()),
                activate: true,
            },
        )
        .expect("first profile");
        assert_eq!(first.profiles.len(), 1);
        assert!(first.profiles[0].is_active);

        let second = SettingsService::upsert_model_profile(
            &service,
            &api,
            &ModelProfileUpsertRequestDto {
                profile_id: None,
                label: "second".to_string(),
                provider: "openai".to_string(),
                base_url: "https://api.two.example".to_string(),
                custom_endpoint: String::new(),
                model_name: "gpt-two".to_string(),
                api_key_label: "second key".to_string(),
                api_key: Some("sk-second".to_string()),
                activate: false,
            },
        )
        .expect("second profile");
        assert_eq!(second.profiles.len(), 2);
        assert_eq!(
            second
                .profiles
                .iter()
                .find(|profile| profile.is_active)
                .expect("active")
                .label,
            "first"
        );

        let second_id = second
            .profiles
            .iter()
            .find(|profile| profile.label == "second")
            .expect("second")
            .profile_id
            .clone();
        let activated =
            SettingsService::activate_model_profile(&service, &api, &second_id).expect("activate");
        assert_eq!(
            activated
                .profiles
                .iter()
                .find(|profile| profile.is_active)
                .expect("active")
                .label,
            "second"
        );
        let settings = SettingsService::get_settings(&service).expect("settings");
        assert_eq!(settings.model_name, "gpt-two");
        assert_eq!(settings.base_url, "https://api.two.example");

        let remaining =
            SettingsService::delete_model_profile(&service, &api, &second_id).expect("delete");
        assert_eq!(remaining.profiles.len(), 1);
        assert!(remaining.profiles[0].is_active);
        assert_eq!(remaining.profiles[0].label, "first");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn privacy_notice_requirement_matches_provider_or_endpoint_rule() {
        let settings = AppSettingsDto::default();
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
        settings.model_name = "zai-org/GLM-4.6V".to_string();
        settings.api_port = 0;
        assert!(SettingsService::validate_settings(&settings).is_err());

        settings.api_port = 17321;
        settings.api_enabled = true;
        settings.api_bind_address = "0.0.0.0".to_string();
        assert!(SettingsService::validate_settings(&settings).is_err());
    }

    #[test]
    fn validate_settings_rejects_removed_model_provider() {
        let mut settings = AppSettingsDto::default();
        settings.model_provider = "custom".to_string();
        let err = SettingsService::validate_settings(&settings).expect_err("removed provider");
        assert_eq!(err.code, "settings_validation_failed");
        assert!(err.details.unwrap_or_default().contains("provider=custom"));
    }
}
