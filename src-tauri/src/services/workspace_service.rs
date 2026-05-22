use crate::artifacts::workspace_layout::WorkspaceLayout;
use crate::domain::settings::AppSettingsDto;
use crate::domain::workspace::{CurrentWorkspace, WorkspaceStatus, WorkspaceStatusDto};
use crate::errors::{AppError, AppResult};
use crate::repositories::db::{block_on_db, connect_workspace_db, run_migrations};
use crate::repositories::ledger_repository::LedgerRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::workspace_settings_repository::WorkspaceSettingsRepository;
use crate::services::api_server_service::ApiServerService;
use sqlx::SqliteConnection;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct WorkspaceService {
    settings: SettingsRepository,
    current: Mutex<Option<CurrentWorkspace>>,
}

impl Clone for WorkspaceService {
    fn clone(&self) -> Self {
        let current = self.current.lock().ok().and_then(|guard| guard.clone());
        Self {
            settings: self.settings.clone(),
            current: Mutex::new(current),
        }
    }
}

impl WorkspaceService {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            settings: SettingsRepository::new(config_dir),
            current: Mutex::new(None),
        }
    }

    pub fn get_workspace_status(&self) -> WorkspaceStatusDto {
        match self.current.lock() {
            Ok(guard) => {
                if let Some(current) = guard.as_ref() {
                    return self.status_for_path(current.root.clone(), "restore");
                }
            }
            Err(_) => {
                return status_with_error(AppError::new(
                    "workspace_state_poisoned",
                    "工作区状态暂时不可用，请重启应用后重试。",
                    "restore",
                    true,
                ));
            }
        }

        match self.settings.load_bootstrap() {
            Ok(settings) => {
                if let Some(path) = settings.last_workspace_path {
                    self.status_for_path(PathBuf::from(path), "restore")
                } else {
                    WorkspaceStatusDto {
                        status: WorkspaceStatus::NotSelected.as_str().to_string(),
                        workspace_path: None,
                        error: None,
                    }
                }
            }
            Err(error) => status_with_error(error),
        }
    }

    pub fn select_workspace(
        &self,
        path: String,
        api_server: &ApiServerService,
    ) -> WorkspaceStatusDto {
        let requested_root = PathBuf::from(&path);
        match self.initialize_workspace(requested_root.clone()) {
            Ok(layout) => {
                let root = layout.root().to_path_buf();
                if let Ok(mut guard) = self.current.lock() {
                    *guard = Some(CurrentWorkspace { root: root.clone() });
                }

                if let Err(err) = Self::reconcile_api_for_new_workspace(&layout, api_server) {
                    tracing::warn!(
                        target: "api",
                        code = %err.code,
                        "切换工作区后协调 localhost API 失败"
                    );
                }

                WorkspaceStatusDto {
                    status: WorkspaceStatus::Ready.as_str().to_string(),
                    workspace_path: Some(path_to_string(&root)),
                    error: None,
                }
            }
            Err(error) => selection_status_with_error(requested_root, error),
        }
    }

    fn reconcile_api_for_new_workspace(
        layout: &WorkspaceLayout,
        api_server: &ApiServerService,
    ) -> AppResult<()> {
        let record = WorkspaceSettingsRepository::new(layout.clone()).load_workspace_settings()?;
        let mut settings = AppSettingsDto::default();
        settings.apply_workspace_record(record);
        api_server.reconcile_for_new_workspace(&settings)
    }

    pub fn settings_repository(&self) -> &SettingsRepository {
        &self.settings
    }

    pub fn workspace_layout(&self) -> AppResult<WorkspaceLayout> {
        self.current_layout()
    }

    pub fn get_db_connection(&self) -> AppResult<SqliteConnection> {
        let layout = self.current_layout()?;
        let db_path = layout.app_db_path();
        block_on_db(async {
            run_migrations(db_path.clone()).await?;
            connect_workspace_db(db_path).await
        })
    }

    pub fn current_layout(&self) -> AppResult<WorkspaceLayout> {
        let status = self.get_workspace_status();
        if status.status != WorkspaceStatus::Ready.as_str() {
            return Err(status.error.unwrap_or_else(|| {
                AppError::new(
                    "workspace_not_ready",
                    "请先选择可用的工作区。",
                    "workspace",
                    true,
                )
            }));
        }
        let path = status.workspace_path.ok_or_else(|| {
            AppError::new(
                "workspace_path_missing",
                "工作区路径缺失，请重新选择工作区。",
                "workspace",
                true,
            )
        })?;
        Ok(WorkspaceLayout::from_root(PathBuf::from(path)))
    }

    fn initialize_workspace(&self, path: PathBuf) -> AppResult<WorkspaceLayout> {
        let root = validate_workspace_path(path)?;
        let layout = WorkspaceLayout::from_root(root.clone());
        layout.ensure_base_layout()?;
        LedgerRepository::new(layout.clone()).run_initial_migrations()?;
        self.settings.save_last_workspace(path_to_string(&root))?;
        Ok(layout)
    }

    fn status_for_path(&self, path: PathBuf, stage: &str) -> WorkspaceStatusDto {
        if !path.exists() {
            return WorkspaceStatusDto {
                status: WorkspaceStatus::Missing.as_str().to_string(),
                workspace_path: Some(path_to_string(&path)),
                error: Some(AppError::new(
                    "workspace_missing",
                    "最近使用的工作区不存在，请重新选择。",
                    stage,
                    true,
                )),
            };
        }

        if !path.is_dir() {
            return WorkspaceStatusDto {
                status: WorkspaceStatus::Invalid.as_str().to_string(),
                workspace_path: Some(path_to_string(&path)),
                error: Some(AppError::new(
                    "workspace_not_directory",
                    "选择的位置不是文件夹，请重新选择本地目录。",
                    stage,
                    true,
                )),
            };
        }

        let layout = WorkspaceLayout::from_root(path.clone());
        match layout
            .ensure_base_layout()
            .and_then(|_| LedgerRepository::new(layout).run_initial_migrations())
        {
            Ok(()) => WorkspaceStatusDto {
                status: WorkspaceStatus::Ready.as_str().to_string(),
                workspace_path: Some(path_to_string(&path)),
                error: None,
            },
            Err(error) => status_with_error(error),
        }
    }
}

fn validate_workspace_path(path: PathBuf) -> AppResult<PathBuf> {
    let root = if path.exists() {
        fs::canonicalize(&path).map_err(|err| {
            AppError::io("validate", "workspace_canonicalize_failed", err)
                .with_details(path.display().to_string())
        })?
    } else {
        path
    };

    if root.exists() && !root.is_dir() {
        return Err(AppError::new(
            "workspace_not_directory",
            "选择的位置不是文件夹，请重新选择本地目录。",
            "validate",
            true,
        )
        .with_details(root.display().to_string()));
    }

    fs::create_dir_all(&root)
        .map_err(|err| AppError::io("validate", "workspace_create_root_failed", err))?;

    let probe = root.join(".slicer-write-test");
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&probe)
        .map_err(|err| AppError::io("validate", "workspace_not_writable", err))?;
    let _ = fs::remove_file(probe);

    fs::canonicalize(&root).map_err(|err| {
        AppError::io("validate", "workspace_canonicalize_failed", err)
            .with_details(root.display().to_string())
    })
}

fn status_with_error(error: AppError) -> WorkspaceStatusDto {
    WorkspaceStatusDto {
        status: WorkspaceStatus::Error.as_str().to_string(),
        workspace_path: None,
        error: Some(error),
    }
}

fn path_to_string(path: impl AsRef<std::path::Path>) -> String {
    normalize_display_path(&path.as_ref().to_string_lossy())
}

fn normalize_display_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{stripped}");
    }
    if let Some(stripped) = path.strip_prefix(r"\\?\") {
        return stripped.to_string();
    }
    path.to_string()
}

fn selection_status_with_error(path: PathBuf, error: AppError) -> WorkspaceStatusDto {
    let status = match error.code.as_str() {
        "workspace_not_directory" => WorkspaceStatus::Invalid,
        _ => WorkspaceStatus::Error,
    };

    WorkspaceStatusDto {
        status: status.as_str().to_string(),
        workspace_path: Some(path_to_string(path)),
        error: Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_display_path, WorkspaceService};
    use crate::api::state::ApiAppState;
    use crate::services::api_server_service::ApiServerService;
    use std::fs;
    use std::sync::Arc;

    fn test_state(config_dir: &std::path::Path) -> ApiAppState {
        ApiAppState::new(Arc::new(WorkspaceService::new(config_dir.to_path_buf())))
    }

    #[test]
    fn select_workspace_creates_layout_and_restores() {
        let base =
            std::env::temp_dir().join(format!("slicer-service-测试 工作区-{}", std::process::id()));
        let config = base.join("config");
        let workspace = base.join("workspace");
        let _ = fs::remove_dir_all(&base);

        let service = WorkspaceService::new(config.clone());
        let api = ApiServerService::new(test_state(&config));
        let selected = service.select_workspace(workspace.to_string_lossy().into_owned(), &api);
        assert_eq!(selected.status, "ready");
        assert!(!selected
            .workspace_path
            .as_deref()
            .unwrap_or_default()
            .starts_with(r"\\?\"));
        assert!(workspace.join("originals").is_dir());
        assert!(workspace.join("indexes").join("bm25").is_dir());
        assert!(workspace.join("app.db").is_file());
        assert!(!workspace.join("settings.json").exists());
        assert!(!workspace.join("jobs.json").exists());
        assert!(!workspace.join("errors.json").exists());

        fs::write(workspace.join("originals").join("keep.txt"), "keep").expect("sentinel");
        let again = service.select_workspace(workspace.to_string_lossy().into_owned(), &api);
        assert_eq!(again.status, "ready");
        assert_eq!(
            fs::read_to_string(workspace.join("originals").join("keep.txt")).expect("sentinel"),
            "keep"
        );

        let restored = WorkspaceService::new(config).get_workspace_status();
        assert_eq!(restored.status, "ready");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn file_path_is_rejected_as_workspace() {
        let base = std::env::temp_dir().join(format!("slicer-service-file-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("base");
        let file = base.join("not-a-dir.txt");
        fs::write(&file, "file").expect("file");

        let service = WorkspaceService::new(base.join("config"));
        let api = ApiServerService::new(test_state(&base.join("config")));
        let result = service.select_workspace(file.to_string_lossy().into_owned(), &api);
        assert_eq!(result.status, "invalid");
        assert_eq!(result.error.expect("error").code, "workspace_not_directory");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn deleted_workspace_restores_as_missing() {
        let base =
            std::env::temp_dir().join(format!("slicer-service-missing-{}", std::process::id()));
        let config = base.join("config");
        let workspace = base.join("workspace");
        let _ = fs::remove_dir_all(&base);

        let service = WorkspaceService::new(config.clone());
        let api = ApiServerService::new(test_state(&config));
        let selected = service.select_workspace(workspace.to_string_lossy().into_owned(), &api);
        let expected_path = selected
            .workspace_path
            .clone()
            .expect("workspace path should exist");
        assert_eq!(selected.status, "ready");

        fs::remove_dir_all(&workspace).expect("workspace should be removable");

        let restored = WorkspaceService::new(config).get_workspace_status();
        assert_eq!(restored.status, "missing");
        assert_eq!(
            restored.workspace_path.as_deref(),
            Some(expected_path.as_str())
        );
        assert_eq!(restored.error.expect("error").code, "workspace_missing");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn readonly_probe_reports_workspace_not_writable() {
        let base =
            std::env::temp_dir().join(format!("slicer-service-readonly-{}", std::process::id()));
        let workspace = base.join("workspace");
        let probe = workspace.join(".slicer-write-test");
        let _ = fs::remove_dir_all(&base);

        fs::create_dir_all(&workspace).expect("workspace");
        fs::write(&probe, "locked").expect("probe");

        let mut permissions = fs::metadata(&probe).expect("metadata").permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&probe, permissions).expect("readonly");

        let service = WorkspaceService::new(base.join("config"));
        let api = ApiServerService::new(test_state(&base.join("config")));
        let result = service.select_workspace(workspace.to_string_lossy().into_owned(), &api);
        assert_eq!(result.status, "error");
        assert_eq!(result.error.expect("error").code, "workspace_not_writable");

        let mut cleanup_permissions = fs::metadata(&probe).expect("metadata").permissions();
        cleanup_permissions.set_readonly(false);
        fs::set_permissions(&probe, cleanup_permissions).expect("writable");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn display_paths_hide_windows_verbatim_prefixes() {
        assert_eq!(
            normalize_display_path(r"\\?\C:\Users\51314\Documents\slicer-data"),
            r"C:\Users\51314\Documents\slicer-data"
        );
        assert_eq!(
            normalize_display_path(r"\\?\UNC\server\share\slicer-data"),
            r"\\server\share\slicer-data"
        );
        assert_eq!(
            normalize_display_path(r"C:\Users\51314\Documents\slicer-data"),
            r"C:\Users\51314\Documents\slicer-data"
        );
    }
}
