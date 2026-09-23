//! Discord Rich Presence interface

use crate::State;
pub use crate::state::LauncherActivity;

/// Tell Rich Presence which part of the launcher the user is in. It is shown while no visible
/// instance is running, and only sent to Discord when it actually changed
#[tracing::instrument]
pub async fn set_launcher_activity(
    activity: LauncherActivity,
) -> crate::Result<()> {
    let state = State::get().await?;
    state.discord_rpc.set_launcher_activity(activity).await
}
