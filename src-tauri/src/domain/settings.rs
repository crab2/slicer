use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
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
        Self {
            workspace_path: None,
            libreoffice_path: None,
            model_provider: "custom".to_string(),
            api_key_configured: false,
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
