use crate::api::Result;
use std::path::Path;
use theseus::prelude::*;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("background")
        .invoke_handler(tauri::generate_handler![background_cache_image])
        .build()
}

// Copy a user-chosen image into the launcher's backgrounds cache and return the path of the copy
// invoke('plugin:background|background_cache_image', { path })
#[tauri::command]
pub async fn background_cache_image(path: &Path) -> Result<String> {
    Ok(background::cache_image(path).await?)
}
