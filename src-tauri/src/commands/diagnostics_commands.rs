use crate::errors::AppError;
use crate::repositories::ledger_repository::LedgerRepository;
use crate::services::workspace_service::WorkspaceService;
use tauri::State;

#[tauri::command]
pub fn record_diagnostic_error(
    code: String,
    message: String,
    stage: String,
    workspace: State<'_, WorkspaceService>,
) -> Result<AppError, AppError> {
    let error = AppError::new(code, message, stage, true);
    let span = crate::diagnostics::correlation_span(&error.correlation_id);
    let _guard = span.enter();
    let layout = workspace.current_layout()?;
    LedgerRepository::new(layout).record_error(&error)?;
    Ok(error)
}
