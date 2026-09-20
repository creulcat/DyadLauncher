//! Theseus settings management interface

pub use crate::{
    State,
    state::{Hooks, MemorySettings, Settings, WindowSize},
};

/// Gets entire settings
#[tracing::instrument]
pub async fn get() -> crate::Result<Settings> {
    let state = State::get().await?;
    let settings = Settings::get(&state.pool).await?;
    Ok(settings)
}

/// Sets entire settings
#[tracing::instrument]
pub async fn set(settings: Settings) -> crate::Result<()> {
    let state = State::get().await?;
    let discord_rpc_changed =
        Settings::get(&state.pool).await?.discord_rpc != settings.discord_rpc;
    settings.update(&state.pool).await?;

    // Apply the Rich Presence toggle right away instead of waiting for the next launch/exit event
    if discord_rpc_changed && let Err(e) = state.discord_rpc.refresh(true).await
    {
        tracing::warn!(
            "Failed to refresh Discord presence after settings change: {e}"
        );
    }

    Ok(())
}

#[tracing::instrument]
pub async fn cancel_directory_change(
    app_identifier: &str,
) -> crate::Result<()> {
    // This is called to handle state initialization errors due to folder migrations
    // failing, so fetching a DB connection pool from `State::get` is not reliable here
    let pool = crate::state::db::connect(app_identifier).await?;
    let mut settings = Settings::get(&pool).await?;

    if let Some(prev_custom_dir) = settings.prev_custom_dir {
        settings.prev_custom_dir = None;
        settings.custom_dir = Some(prev_custom_dir);
    }

    settings.update(&pool).await?;

    Ok(())
}
