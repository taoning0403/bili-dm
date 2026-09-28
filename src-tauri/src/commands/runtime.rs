use tauri::State;

use crate::core::app_service::{AppService, RuntimeInfo};

#[tauri::command]
pub fn get_runtime_info(service: State<'_, AppService>) -> RuntimeInfo {
    service.runtime_info()
}
