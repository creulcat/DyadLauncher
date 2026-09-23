use crate::api::Result;
use theseus::prelude::*;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("discord")
        .invoke_handler(tauri::generate_handler![discord_set_launcher_activity])
        .build()
}

// Tell Rich Presence which part of the launcher the user is in
// invoke('plugin:discord|discord_set_launcher_activity', { activity })
#[tauri::command]
pub async fn discord_set_launcher_activity(
    activity: LauncherActivity,
) -> Result<()> {
    discord::set_launcher_activity(activity).await?;
    Ok(())
}
