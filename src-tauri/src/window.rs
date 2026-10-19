//! Window control commands for the custom title bar.

use crate::error::{ApiError, ApiResult};

pub fn minimize(window: &tauri::Window) -> ApiResult<()> {
    window
        .minimize()
        .map_err(|error| ApiError::new("WINDOW", error.to_string()))
}

/// Returns `true` when the window ended up maximized.
pub fn toggle_maximize(window: &tauri::Window) -> ApiResult<bool> {
    let maximized = window
        .is_maximized()
        .map_err(|error| ApiError::new("WINDOW", error.to_string()))?;
    if maximized {
        window
            .unmaximize()
            .map_err(|error| ApiError::new("WINDOW", error.to_string()))?;
        Ok(false)
    } else {
        window
            .maximize()
            .map_err(|error| ApiError::new("WINDOW", error.to_string()))?;
        Ok(true)
    }
}

pub fn close(window: &tauri::Window) -> ApiResult<()> {
    window
        .close()
        .map_err(|error| ApiError::new("WINDOW", error.to_string()))
}
