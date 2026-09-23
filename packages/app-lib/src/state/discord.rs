use std::sync::{Arc, atomic::AtomicBool};

use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, Assets, Timestamps},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::State;
use crate::state::{ContentSet, ModLoader};

/// Discord rejects `details`/`state` strings longer than 128 bytes
const MAX_FIELD_BYTES: usize = 128;

/// Which part of the launcher the user is in. Only shown while no visible instance is running.
/// Deliberately coarse: it never carries a project or instance name
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LauncherActivity {
    #[default]
    InLauncher,
    BrowsingMods,
    LookingAtInstances,
    ChangingSkins,
    LookingAtScreenshots,
}

impl LauncherActivity {
    fn label(self) -> &'static str {
        match self {
            Self::InLauncher => "In the launcher",
            Self::BrowsingMods => "Browsing mods",
            Self::LookingAtInstances => "Looking at instances",
            Self::ChangingSkins => "Changing skins",
            Self::LookingAtScreenshots => "Looking at screenshots",
        }
    }
}

/// What Rich Presence should show right now
struct Presence {
    details: String,
    state: Option<String>,
    /// Unix timestamp (seconds) that Discord counts the elapsed time from
    started_at: Option<i64>,
}

impl Presence {
    /// `None` means there is nothing to show, so the presence should be cleared: that is the case
    /// when instances are running but every one of them is hidden from Discord
    async fn current(
        state: &State,
        launcher_activity: LauncherActivity,
    ) -> Option<Self> {
        let mut running = state.process_manager.get_all();
        if running.is_empty() {
            return Some(Self {
                details: launcher_activity.label().to_string(),
                state: None,
                started_at: None,
            });
        }

        // The process map has no stable order, so sort to keep "oldest" meaningful
        running.sort_by_key(|process| process.start_time);

        let mut visible = Vec::new();
        for process in running {
            // If an instance can't be looked up we can't tell whether it is hidden, so leave it
            // out rather than risk showing something the user opted out of
            let Some(context) =
                crate::state::instances::commands::get_instance_launch_context(
                    &process.instance_id,
                    &state.pool,
                )
                .await
                .ok()
                .flatten()
            else {
                continue;
            };

            if !context.launch_overrides.hide_from_discord {
                visible.push((process, context));
            }
        }

        match visible.as_slice() {
            [] => None,
            [(process, context)] => Some(Self {
                details: fit(&format!("Playing: {}", process.instance_name)),
                state: Some(game_version_line(&context.applied_content_set)),
                started_at: Some(process.start_time.timestamp()),
            }),
            [(first, _), ..] => Some(Self {
                details: format!("Playing {} instances at once", visible.len()),
                state: None,
                started_at: Some(first.start_time.timestamp()),
            }),
        }
    }
}

/// e.g. "1.21.4 · Fabric"
fn game_version_line(content_set: &ContentSet) -> String {
    fit(&format!(
        "{} · {}",
        content_set.game_version,
        loader_name(content_set.loader)
    ))
}

fn loader_name(loader: ModLoader) -> &'static str {
    match loader {
        ModLoader::Vanilla => "Vanilla",
        ModLoader::Forge => "Forge",
        ModLoader::Fabric => "Fabric",
        ModLoader::Quilt => "Quilt",
        ModLoader::NeoForge => "NeoForge",
    }
}

/// Trim to Discord's length limit, on a character boundary
fn fit(text: &str) -> String {
    if text.len() <= MAX_FIELD_BYTES {
        return text.to_string();
    }

    let mut end = MAX_FIELD_BYTES - '…'.len_utf8();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

pub struct DiscordGuard {
    client: Arc<RwLock<DiscordIpcClient>>,
    connected: Arc<AtomicBool>,
    /// Held for a whole refresh (work out the presence, then send it), so two refreshes
    /// that overlap (e.g. two instances launching at once) can't apply out of order
    refresh_lock: Mutex<()>,
    /// The page the user is on, shown when no visible instance is running
    launcher_activity: std::sync::Mutex<LauncherActivity>,
}

impl DiscordGuard {
    /// Initialize discord IPC client, and attempt to connect to it
    /// If it fails, it will still return a DiscordGuard, but the client will be unconnected
    pub fn init() -> crate::Result<DiscordGuard> {
        let dipc = DiscordIpcClient::new("1546488855287566437");

        Ok(DiscordGuard {
            client: Arc::new(RwLock::new(dipc)),
            connected: Arc::new(AtomicBool::new(false)),
            refresh_lock: Mutex::new(()),
            launcher_activity: std::sync::Mutex::new(
                LauncherActivity::default(),
            ),
        })
    }

    /// Record which part of the launcher the user is in, and update Discord if that changed
    pub async fn set_launcher_activity(
        &self,
        activity: LauncherActivity,
    ) -> crate::Result<()> {
        {
            let mut current = self
                .launcher_activity
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if *current == activity {
                return Ok(());
            }
            *current = activity;
        }

        self.refresh(true).await
    }

    /// If the client failed connecting during init(), this will check for connection and attempt to reconnect
    /// This MUST be called first in any client method that requires a connection, because those can PANIC if the client is not connected
    /// (No connection is different than a failed connection, the latter will not panic and can be retried)
    pub async fn retry_if_not_ready(&self) -> bool {
        let mut client = self.client.write().await;
        if !self.connected.load(std::sync::atomic::Ordering::Relaxed) {
            if client.connect().is_ok() {
                self.connected
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            return false;
        }
        true
    }

    /// Bring Discord in line with the launcher's current state: clear the presence if Rich
    /// Presence is turned off (or only hidden instances are running), otherwise show what is
    /// being played (or what the user is doing in the launcher)
    /// Call this whenever something that affects the presence changes
    pub async fn refresh(&self, reconnect_if_fail: bool) -> crate::Result<()> {
        let _guard = self.refresh_lock.lock().await;

        let state = State::get().await?;
        let settings = crate::state::Settings::get(&state.pool).await?;
        if !settings.discord_rpc {
            return self.clear_activity(reconnect_if_fail).await;
        }

        let launcher_activity = *self
            .launcher_activity
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match Presence::current(&state, launcher_activity).await {
            Some(presence) => {
                self.set_presence(&presence, reconnect_if_fail).await
            }
            None => self.clear_activity(reconnect_if_fail).await,
        }
    }

    async fn set_presence(
        &self,
        presence: &Presence,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Attempt to connect if not connected. Do not continue if it fails, as the client.set_activity can panic if it never was connected
        if !self.retry_if_not_ready().await {
            return Ok(());
        }

        let mut activity = Activity::new().details(&presence.details).assets(
            Assets::new()
                .large_image("logo_square_1024")
                .large_text("Dyad Launcher"),
        );
        if let Some(line) = &presence.state {
            activity = activity.state(line);
        }
        if let Some(started_at) = presence.started_at {
            activity = activity.timestamps(Timestamps::new().start(started_at));
        }

        // Attempt to set the activity
        // If the existing connection fails, attempt to reconnect and try again
        let mut client: tokio::sync::RwLockWriteGuard<'_, DiscordIpcClient> =
            self.client.write().await;
        let res = client.set_activity(activity.clone());

        if reconnect_if_fail {
            if let Err(_e) = res {
                client.reconnect()?;
                return Ok(client.set_activity(activity)?); // try again, but don't reconnect if it fails again
            }
        } else {
            res?;
        }

        Ok(())
    }

    /// Clear the activity entirely ('disabling' the RPC until the next refresh)
    /// Never connects to Discord on its own: if we never connected there is no activity to clear,
    /// and users who keep Rich Presence off should not have the launcher talk to Discord at all
    async fn clear_activity(
        &self,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Do not continue if we never connected, as the client.clear_activity can panic in that case
        if !self.connected.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        // Attempt to clear the activity
        // If the existing connection fails, attempt to reconnect and try again
        let mut client = self.client.write().await;
        let res = client.clear_activity();

        if reconnect_if_fail {
            if res.is_err() {
                client.reconnect()?;
                return Ok(client.clear_activity()?); // try again, but don't reconnect if it fails again
            }
        } else {
            res?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The frontend sends these exact strings (apps/app-frontend/src/helpers/discord.ts)
    #[test]
    fn launcher_activity_matches_the_names_the_frontend_sends() {
        for (name, expected) in [
            ("in_launcher", LauncherActivity::InLauncher),
            ("browsing_mods", LauncherActivity::BrowsingMods),
            ("looking_at_instances", LauncherActivity::LookingAtInstances),
            ("changing_skins", LauncherActivity::ChangingSkins),
            (
                "looking_at_screenshots",
                LauncherActivity::LookingAtScreenshots,
            ),
        ] {
            let parsed: LauncherActivity =
                serde_json::from_str(&format!("\"{name}\"")).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn fit_leaves_short_text_alone() {
        assert_eq!(fit("Playing Vanilla"), "Playing Vanilla");
    }

    #[test]
    fn fit_trims_long_text_to_the_limit() {
        let fitted = fit(&"a".repeat(300));
        assert_eq!(fitted.len(), MAX_FIELD_BYTES);
        assert!(fitted.ends_with('…'));
    }

    #[test]
    fn fit_never_splits_a_multibyte_character() {
        // 3-byte characters, so the cut point lands mid-character for most offsets
        let fitted = fit(&"日".repeat(100));
        assert!(fitted.len() <= MAX_FIELD_BYTES);
        assert!(fitted.ends_with('…'));
    }
}
