use crate::domain::job::{
    core_status_catalog, CoreStatusCatalogDto, CreateJobRequestDto, JobDto,
    UpdateJobProgressRequestDto,
};
use crate::errors::AppError;
use crate::jobs::job_orchestrator::JobOrchestrator;
use crate::services::workspace_service::WorkspaceService;
use tauri::State;

#[tauri::command]
pub fn get_core_status_catalog() -> CoreStatusCatalogDto {
    core_status_catalog()
}

#[tauri::command]
pub fn list_jobs(workspace: State<'_, WorkspaceService>) -> Result<Vec<JobDto>, AppError> {
    let layout = workspace.current_layout()?;
    JobOrchestrator::new(layout).list_jobs()
}

#[tauri::command]
pub fn create_placeholder_job(
    job_type: String,
    workspace: State<'_, WorkspaceService>,
) -> Result<JobDto, AppError> {
    let layout = workspace.current_layout()?;
    JobOrchestrator::new(layout).enqueue_placeholder(&job_type)
}

#[tauri::command]
pub fn create_job(
    request: CreateJobRequestDto,
    workspace: State<'_, WorkspaceService>,
) -> Result<JobDto, AppError> {
    let layout = workspace.current_layout()?;
    JobOrchestrator::new(layout).create_job(&request.job_type)
}

#[tauri::command]
pub fn update_job_progress(
    request: UpdateJobProgressRequestDto,
    workspace: State<'_, WorkspaceService>,
) -> Result<JobDto, AppError> {
    let layout = workspace.current_layout()?;
    JobOrchestrator::new(layout).update_progress(
        &request.job_id,
        request.progress,
        request.message.as_deref(),
    )
}

#[tauri::command]
pub fn fail_job(
    job_id: String,
    code: String,
    message: String,
    workspace: State<'_, WorkspaceService>,
) -> Result<JobDto, AppError> {
    let layout = workspace.current_layout()?;
    let error = AppError::new(code, message.clone(), "job", true);
    JobOrchestrator::new(layout).mark_failed(&job_id, &error, &message)
}

#[tauri::command]
pub fn recover_interrupted_jobs(
    workspace: State<'_, WorkspaceService>,
) -> Result<Vec<JobDto>, AppError> {
    let layout = workspace.current_layout()?;
    JobOrchestrator::new(layout).recover_interrupted_jobs()
}
