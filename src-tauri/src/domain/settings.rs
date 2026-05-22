use serde::{Deserialize, Serialize};

/// Non-sensitive settings persisted in workspace SQLite (`settings` table).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceSettingsRecord {
    pub model_provider: String,
    pub base_url: String,
    pub custom_endpoint: String,
    pub model_name: String,
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
            model_provider: "custom".to_string(),
            base_url: String::new(),
            custom_endpoint: String::new(),
            model_name: String::new(),
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
        self.default_image_dpi = record.default_image_dpi;
        self.conversion_concurrency = record.conversion_concurrency;
        self.analysis_concurrency = record.analysis_concurrency;
        self.api_enabled = record.api_enabled;
        self.api_bind_address = record.api_bind_address;
        self.api_port = record.api_port;
    }
}
