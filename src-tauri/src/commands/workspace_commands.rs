use crate::domain::workspace::WorkspaceStatusDto;
use crate::services::workspace_service::WorkspaceService;
use tauri::State;

#[tauri::command]
pub fn get_workspace_status(workspace: State<'_, WorkspaceService>) -> WorkspaceStatusDto {
    workspace.get_workspace_status()
}

#[tauri::command]
pub fn select_workspace(
    path: String,
    workspace: State<'_, WorkspaceService>,
) -> WorkspaceStatusDto {
    workspace.select_workspace(path)
}
