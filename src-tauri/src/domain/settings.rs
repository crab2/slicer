use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Non-sensitive settings persisted in workspace SQLite (`settings` table).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WorkspaceSettingsRecord {
    pub model_provider: String,
    pub base_url: String,
    pub custom_endpoint: String,
    pub model_name: String,
    #[serde(
        default,
        serialize_with = "serialize_workspace_model_profiles",
        deserialize_with = "deserialize_workspace_model_profiles"
    )]
    pub model_profiles: Vec<ModelProfileDto>,
    pub default_image_dpi: u16,
    pub conversion_concurrency: u8,
    pub analysis_concurrency: u8,
    pub api_enabled: bool,
    pub api_bind_address: String,
    pub api_port: u16,
}

impl Default for WorkspaceSettingsRecord {
    fn default() -> Self {
        Self {
            model_provider: "siliconflow".to_string(),
            base_url: String::new(),
            custom_endpoint: String::new(),
            model_name: String::new(),
            model_profiles: Vec::new(),
            default_image_dpi: 144,
            conversion_concurrency: 2,
            analysis_concurrency: 2,
            api_enabled: false,
            api_bind_address: "127.0.0.1".to_string(),
            api_port: 17321,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfigurationStatusDto {
    pub configured: bool,
    pub missing: Vec<String>,
    pub privacy_notice_accepted: bool,
    pub requires_privacy_notice: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModelInfoDto {
    pub id: String,
    pub display_name: Option<String>,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModelListDto {
    pub provider: String,
    pub models: Vec<ModelInfoDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModelProfileDto {
    pub profile_id: String,
    pub label: String,
    pub provider: String,
    pub base_url: String,
    pub custom_endpoint: String,
    pub model_name: String,
    pub key_id: Option<String>,
    pub key_label: Option<String>,
    #[serde(alias = "key_configured")]
    pub api_key_configured: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for ModelProfileDto {
    fn default() -> Self {
        Self {
            profile_id: String::new(),
            label: String::new(),
            provider: "openai".to_string(),
            base_url: String::new(),
            custom_endpoint: String::new(),
            model_name: String::new(),
            key_id: None,
            key_label: None,
            api_key_configured: false,
            is_active: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
struct WorkspaceModelProfileRecord {
    pub profile_id: String,
    pub label: String,
    pub provider: String,
    pub base_url: String,
    pub custom_endpoint: String,
    pub model_name: String,
    pub key_id: Option<String>,
    pub key_label: Option<String>,
    #[serde(rename = "key_configured", alias = "api_key_configured")]
    pub api_key_configured: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for WorkspaceModelProfileRecord {
    fn default() -> Self {
        Self::from(ModelProfileDto::default())
    }
}

impl From<ModelProfileDto> for WorkspaceModelProfileRecord {
    fn from(profile: ModelProfileDto) -> Self {
        Self {
            profile_id: profile.profile_id,
            label: profile.label,
            provider: profile.provider,
            base_url: profile.base_url,
            custom_endpoint: profile.custom_endpoint,
            model_name: profile.model_name,
            key_id: profile.key_id,
            key_label: profile.key_label,
            api_key_configured: profile.api_key_configured,
            is_active: profile.is_active,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        }
    }
}

impl From<WorkspaceModelProfileRecord> for ModelProfileDto {
    fn from(profile: WorkspaceModelProfileRecord) -> Self {
        Self {
            profile_id: profile.profile_id,
            label: profile.label,
            provider: profile.provider,
            base_url: profile.base_url,
            custom_endpoint: profile.custom_endpoint,
            model_name: profile.model_name,
            key_id: profile.key_id,
            key_label: profile.key_label,
            api_key_configured: profile.api_key_configured,
            is_active: profile.is_active,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        }
    }
}

fn serialize_workspace_model_profiles<S>(
    profiles: &Vec<ModelProfileDto>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    profiles
        .iter()
        .cloned()
        .map(WorkspaceModelProfileRecord::from)
        .collect::<Vec<_>>()
        .serialize(serializer)
}

fn deserialize_workspace_model_profiles<'de, D>(
    deserializer: D,
) -> Result<Vec<ModelProfileDto>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(
        Vec::<WorkspaceModelProfileRecord>::deserialize(deserializer)?
            .into_iter()
            .map(ModelProfileDto::from)
            .collect(),
    )
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModelProfileListDto {
    pub profiles: Vec<ModelProfileDto>,
    pub max_profiles: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelProfileUpsertRequestDto {
    pub profile_id: Option<String>,
    pub label: String,
    pub provider: String,
    pub base_url: String,
    pub custom_endpoint: String,
    pub model_name: String,
    pub api_key_label: String,
    pub api_key: Option<String>,
    pub activate: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrivacyNoticeStatusDto {
    pub accepted: bool,
    pub requires_notice: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiServerStatusDto {
    pub runtime_status: String,
    pub bind_address: String,
    pub port: u16,
    pub enabled: bool,
    pub last_error: Option<crate::errors::AppError>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ApiKeyRecordDto {
    pub key_id: String,
    pub provider: String,
    pub label: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ApiKeyListDto {
    pub keys: Vec<ApiKeyRecordDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppSettingsDto {
    pub workspace_path: Option<String>,
    pub libreoffice_path: Option<String>,
    pub model_provider: String,
    pub api_key_configured: bool,
    pub base_url: String,
    pub custom_endpoint: String,
    pub model_name: String,
    pub model_profiles: Vec<ModelProfileDto>,
    pub default_image_dpi: u16,
    pub conversion_concurrency: u8,
    pub analysis_concurrency: u8,
    pub api_enabled: bool,
    pub api_bind_address: String,
    pub api_port: u16,
}

impl Default for AppSettingsDto {
    fn default() -> Self {
        let workspace = WorkspaceSettingsRecord::default();
        Self {
            workspace_path: None,
            libreoffice_path: None,
            model_provider: workspace.model_provider,
            api_key_configured: false,
            base_url: workspace.base_url,
            custom_endpoint: workspace.custom_endpoint,
            model_name: workspace.model_name,
            model_profiles: workspace.model_profiles,
            default_image_dpi: workspace.default_image_dpi,
            conversion_concurrency: workspace.conversion_concurrency,
            analysis_concurrency: workspace.analysis_concurrency,
            api_enabled: workspace.api_enabled,
            api_bind_address: workspace.api_bind_address,
            api_port: workspace.api_port,
        }
    }
}

impl AppSettingsDto {
    pub fn workspace_record(&self) -> WorkspaceSettingsRecord {
        WorkspaceSettingsRecord {
            model_provider: self.model_provider.clone(),
            base_url: self.base_url.clone(),
            custom_endpoint: self.custom_endpoint.clone(),
            model_name: self.model_name.clone(),
            model_profiles: self.model_profiles.clone(),
            default_image_dpi: self.default_image_dpi,
            conversion_concurrency: self.conversion_concurrency,
            analysis_concurrency: self.analysis_concurrency,
            api_enabled: self.api_enabled,
            api_bind_address: self.api_bind_address.clone(),
            api_port: self.api_port,
        }
    }

    pub fn apply_workspace_record(&mut self, record: WorkspaceSettingsRecord) {
        self.model_provider = record.model_provider;
        self.base_url = record.base_url;
        self.custom_endpoint = record.custom_endpoint;
        self.model_name = record.model_name;
        self.model_profiles = record.model_profiles;
        self.default_image_dpi = record.default_image_dpi;
        self.conversion_concurrency = record.conversion_concurrency;
        self.analysis_concurrency = record.analysis_concurrency;
        self.api_enabled = record.api_enabled;
        self.api_bind_address = record.api_bind_address;
        self.api_port = record.api_port;
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelProfileDto, WorkspaceSettingsRecord};

    #[test]
    fn model_profile_serializes_frontend_api_key_configured_field() {
        let profile = ModelProfileDto {
            api_key_configured: true,
            ..ModelProfileDto::default()
        };

        let json = serde_json::to_value(&profile).expect("serialize profile");

        assert_eq!(json["api_key_configured"], true);
        assert!(json.get("key_configured").is_none());
    }

    #[test]
    fn model_profile_reads_legacy_key_configured_field() {
        let profile: ModelProfileDto = serde_json::from_value(serde_json::json!({
            "profile_id": "profile-1",
            "label": "legacy",
            "provider": "openai",
            "base_url": "",
            "custom_endpoint": "",
            "model_name": "gpt",
            "key_configured": true,
            "is_active": true,
            "created_at": "2026-06-25T00:00:00Z",
            "updated_at": "2026-06-25T00:00:00Z"
        }))
        .expect("deserialize legacy profile");

        assert!(profile.api_key_configured);
    }

    #[test]
    fn workspace_settings_serializes_profiles_without_api_key_word() {
        let mut record = WorkspaceSettingsRecord::default();
        record.model_profiles.push(ModelProfileDto {
            api_key_configured: true,
            ..ModelProfileDto::default()
        });

        let json = serde_json::to_value(&record).expect("serialize workspace settings");

        assert_eq!(json["model_profiles"][0]["key_configured"], true);
        assert!(json["model_profiles"][0].get("api_key_configured").is_none());
    }
}
