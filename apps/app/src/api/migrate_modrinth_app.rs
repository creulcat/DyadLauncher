use std::path::PathBuf;

use crate::api::Result;
use theseus::migrate_modrinth_app::execute::SettingsImportSelection;
use theseus::migrate_modrinth_app::{
    self, DetectedSource, ImportJavaVersionCandidate, ImportPreview,
    ImportSettingsCandidate,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("migrate-modrinth-app")
        .invoke_handler(tauri::generate_handler![
            detect_modrinth_app_install,
            is_modrinth_app_running,
            preview_modrinth_app_import,
            apply_modrinth_app_settings,
        ])
        .build()
}

/// Looks for an official Modrinth App install at `source_dir` (or the
/// platform's default data directory if omitted). Returns `None` if nothing
/// was found there - this alone doesn't check whether the app is still
/// running or whether its database is a schema this reader understands.
#[tauri::command]
pub async fn detect_modrinth_app_install(
    source_dir: Option<PathBuf>,
) -> Result<Option<DetectedSource>> {
    Ok(migrate_modrinth_app::detect(source_dir).await?)
}

/// Checks whether the official Modrinth App appears to be running (via a
/// process-list check - see `is_source_running`'s doc comment for why this
/// isn't a database-level check), i.e. whether it needs to be closed before
/// importing.
#[tauri::command]
pub fn is_modrinth_app_running() -> bool {
    migrate_modrinth_app::is_source_running()
}

/// Builds a full read-only preview of what could be imported from `source`:
/// candidate instances (with per-category content sizes), global settings,
/// and Java installation paths. Never writes anything - see
/// `docs/goal-6-import-design.md` for the Phase 2 plan to act on a preview
/// like this.
#[tauri::command]
pub async fn preview_modrinth_app_import(
    source: DetectedSource,
) -> Result<ImportPreview> {
    Ok(migrate_modrinth_app::build_preview(&source).await?)
}

/// Applies the selected fields from a settings/Java-path preview onto
/// Dyad's own settings. A plain settings edit, not an install job - no
/// progress bar, takes effect immediately.
#[tauri::command]
pub async fn apply_modrinth_app_settings(
    candidate: ImportSettingsCandidate,
    java_versions: Vec<ImportJavaVersionCandidate>,
    selection: SettingsImportSelection,
) -> Result<()> {
    Ok(migrate_modrinth_app::execute::apply_settings(
        &candidate,
        &java_versions,
        &selection,
    )
    .await?)
}
