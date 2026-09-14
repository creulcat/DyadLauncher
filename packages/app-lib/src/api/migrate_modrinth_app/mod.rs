//! Goal 6 (see `docs/goal-6-import-design.md`): detection, preview, and
//! import support for bringing instances, content, and settings over from
//! the official Modrinth App into Dyad Launcher.
//!
//! Phase 1 (detection/preview, this module's top level plus `source_db`) is
//! read-only: it never writes to Dyad's own database, never writes to the
//! source install, and never touches `minecraft_users`
//! (Microsoft/Minecraft account credentials) - account sign-in is out of
//! scope for this feature entirely, handled the normal way through Dyad's
//! own sign-in flow instead.
//!
//! Phase 2 (`execute`) does the actual writes: copying selected content into
//! an already-created Dyad instance, and applying selected settings/Java
//! paths. It never re-reads the source database - everything it needs comes
//! from a Phase 1 preview the caller already has.

mod source_db;

pub mod execute;

use crate::state::ModLoader;
use futures::{StreamExt, future, stream};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The official Modrinth App's Tauri app identifier - see the "Rename app
/// identity" commit for why Dyad's own identifier is now different.
const SOURCE_APP_IDENTIFIER: &str = "ModrinthApp";
const SOURCE_DB_FILE_NAME: &str = "app.db";
/// Deliberately short: preview reads should fail fast rather than hang if
/// something unexpected does hold the database busy, unlike Dyad's own 30s
/// busy_timeout for its own database.
const SOURCE_DB_BUSY_TIMEOUT: Duration = Duration::from_millis(500);

/// A located (but not yet opened) official Modrinth App install.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedSource {
    /// The official app's settings directory (e.g. `%APPDATA%\ModrinthApp`).
    pub settings_dir: PathBuf,
    pub db_path: PathBuf,
}

/// Returns the default location the official Modrinth App would use for its
/// data directory on this platform, if it exists. Mirrors the approach
/// `pack::import::get_default_launcher_path` uses for other launchers
/// (checking `dirs::data_dir()` rather than assuming a fixed path).
pub fn default_source_dir() -> Option<PathBuf> {
    let dir = dirs::data_dir()?.join(SOURCE_APP_IDENTIFIER);
    dir.exists().then_some(dir)
}

/// Looks for a valid official Modrinth App install at `source_dir` (or the
/// default location if `None`), confirming it has an `app.db` file. Does not
/// open the database, so this alone can't tell whether it's a recognizable
/// schema or whether the app is still running.
pub async fn detect(
    source_dir: Option<PathBuf>,
) -> crate::Result<Option<DetectedSource>> {
    let Some(settings_dir) = source_dir.or_else(default_source_dir) else {
        return Ok(None);
    };

    let db_path = settings_dir.join(SOURCE_DB_FILE_NAME);
    if !tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
        return Ok(None);
    }

    Ok(Some(DetectedSource {
        settings_dir,
        db_path,
    }))
}

/// Returns `true` if the official Modrinth App appears to currently be
/// running.
///
/// This was originally a `BEGIN IMMEDIATE`/`ROLLBACK` probe against the
/// source database, on the theory that a running app would be holding its
/// write lock. Real-install testing disproved that: in SQLite's WAL mode
/// (which the source db uses), the write lock is only held for the duration
/// of an actual write *transaction*, not for as long as a connection/app is
/// open - so the probe reported "not locked" even with the real app
/// confirmed running, because it wasn't writing at that exact instant. That
/// approach can only ever catch a narrow race window, not "is the app
/// running", so it's been replaced with a process-list check instead -
/// the same kind of check `tauri-plugin-single-instance` relies on for
/// Dyad's own single-instance enforcement.
///
/// Matched by process name containing "modrinth" (case-insensitive) rather
/// than an exact string, since this is a "please close the app first" nag,
/// not a security boundary - a loose match that occasionally over-triggers
/// is a far smaller problem than a missed detection risking concurrent
/// writes to the same database.
pub fn is_source_running() -> bool {
    let system = sysinfo::System::new_all();
    system.processes().values().any(|process| {
        process
            .name()
            .to_string_lossy()
            .to_lowercase()
            .contains("modrinth")
    })
}

async fn open_source_pool_readonly(
    db_path: &Path,
) -> crate::Result<Pool<Sqlite>> {
    let conn_options = SqliteConnectOptions::new()
        .filename(db_path)
        .read_only(true)
        .busy_timeout(SOURCE_DB_BUSY_TIMEOUT)
        .create_if_missing(false);

    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(conn_options)
        .await?)
}

/// One of the on-disk content folders a Modrinth-App instance can have.
/// Matches the categories called out in goal 6 (mods, resource/shader packs,
/// config, worlds/saves, screenshots, logs) - a coarser granularity than
/// `export_mrpack.rs`'s per-file selection tree, since goal 6 asks for
/// per-category selection specifically.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ContentCategory {
    Mods,
    ResourcePacks,
    ShaderPacks,
    Config,
    Saves,
    Screenshots,
    Logs,
}

impl ContentCategory {
    pub const ALL: [ContentCategory; 7] = [
        ContentCategory::Mods,
        ContentCategory::ResourcePacks,
        ContentCategory::ShaderPacks,
        ContentCategory::Config,
        ContentCategory::Saves,
        ContentCategory::Screenshots,
        ContentCategory::Logs,
    ];

    fn folder_name(self) -> &'static str {
        match self {
            ContentCategory::Mods => "mods",
            ContentCategory::ResourcePacks => "resourcepacks",
            ContentCategory::ShaderPacks => "shaderpacks",
            ContentCategory::Config => "config",
            ContentCategory::Saves => "saves",
            ContentCategory::Screenshots => "screenshots",
            ContentCategory::Logs => "logs",
        }
    }

    /// Logs default to unselected - matches `export_mrpack.rs`'s
    /// `DEFAULT_SELECTED_EXPORT_PATH_PREFIXES`, which also leaves logs out.
    fn default_selected(self) -> bool {
        !matches!(self, ContentCategory::Logs)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportContentCategory {
    pub category: ContentCategory,
    /// For every category except `Saves`, this is an exact recursive file
    /// count; for `Saves` it's the number of top-level world folders (see
    /// `ImportInstanceCandidate::worlds` for the per-world breakdown).
    pub file_count: u64,
    /// `None` for `Saves` specifically: a real install can have worlds whose
    /// mod-generated cache data (Distant Horizons and similar) balloons a
    /// full recursive size into the hundreds of thousands of files, which
    /// made a preview take 30+ seconds in manual testing against a real
    /// install - see the "Real-install validation" section of
    /// docs/goal-6-import-design.md. Exact sizing there is deferred to
    /// Phase 2's actual copy, where a progress bar makes a slow walk
    /// acceptable in a way a preview screen shouldn't be.
    pub total_size: Option<u64>,
    pub default_selected: bool,
}

/// One world folder found under an instance's `saves/` directory. Listed
/// individually rather than folded into a single recursive category total -
/// see `ImportContentCategory::total_size`'s doc comment.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWorldCandidate {
    pub folder_name: String,
    /// Unix timestamp, if the folder's modified time was readable.
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportInstanceCandidate {
    pub source_id: String,
    /// Resolved absolute path to this instance's folder in the source
    /// install - pass this straight through as
    /// `InstallRequest::ImportModrinthApp`'s `source_instance_dir` when
    /// starting the import; nothing else in this preview identifies it.
    pub instance_dir: PathBuf,
    pub name: String,
    /// Absolute path into the source app's own icon cache, if present and
    /// still on disk. Not copied yet - Phase 2's job.
    pub icon_path: Option<PathBuf>,
    pub loader: ModLoader,
    /// The raw `loader` string from the source DB, kept alongside `loader`
    /// so an unrecognized value (which `ModLoader::from_string` silently
    /// maps to `Vanilla`) can still be surfaced to the user instead of
    /// quietly mislabeled.
    pub raw_loader: Option<String>,
    pub loader_version: Option<String>,
    pub game_version: Option<String>,
    pub created: i64,
    pub modified: i64,
    pub last_played: Option<i64>,
    /// Only categories that actually have files are included.
    pub categories: Vec<ImportContentCategory>,
    /// Per-world breakdown backing the `Saves` category, when present.
    pub worlds: Vec<ImportWorldCandidate>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSettingsCandidate {
    pub extra_launch_args: Option<Vec<String>>,
    pub custom_env_vars: Option<Vec<(String, String)>>,
    pub memory_maximum_mb: Option<u32>,
    pub force_fullscreen: Option<bool>,
    pub game_resolution: Option<(u16, u16)>,
    pub hook_pre_launch: Option<String>,
    pub hook_wrapper: Option<String>,
    pub hook_post_exit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJavaVersionCandidate {
    pub major_version: u32,
    pub full_version: String,
    pub architecture: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub source: DetectedSource,
    pub instances: Vec<ImportInstanceCandidate>,
    pub settings: ImportSettingsCandidate,
    pub java_versions: Vec<ImportJavaVersionCandidate>,
    /// Non-fatal notes about optional columns this reader skipped because
    /// they weren't present in the source schema (see the compatibility
    /// strategy in docs/goal-6-import-design.md). An empty list means the
    /// source schema matched exactly what we expected.
    pub compatibility_notes: Vec<String>,
}

/// Builds a full read-only preview of what could be imported from `source`.
/// Fails with a specific `InputError` if the source app appears to still be
/// running, or if its database schema is missing something this reader
/// requires - never guesses at an unrecognized schema.
pub async fn build_preview(
    source: &DetectedSource,
) -> crate::Result<ImportPreview> {
    if is_source_running() {
        return Err(crate::ErrorKind::InputError(
            "The official Modrinth App appears to be running - close it before importing"
                .to_string(),
        )
        .into());
    }

    let pool = open_source_pool_readonly(&source.db_path).await?;
    let result = build_preview_from_pool(source, &pool).await;
    pool.close().await;
    result
}

async fn build_preview_from_pool(
    source: &DetectedSource,
    pool: &Pool<Sqlite>,
) -> crate::Result<ImportPreview> {
    source_db::check_required_schema(pool).await?;

    // `custom_dir` (when set) is the app's *config* dir override, matching
    // `DirectoryInfo.config_dir` - profiles live under `<config_dir>/profiles`
    // either way, same as `DirectoryInfo::instances_dir()`.
    let config_dir = source_db::read_custom_config_dir(pool)
        .await?
        .filter(|dir| dir.exists())
        .unwrap_or_else(|| source.settings_dir.clone());
    let base_instances_dir = config_dir.join("profiles");

    let raw_instances = source_db::fetch_instances(pool).await?;
    let mut instances = Vec::with_capacity(raw_instances.len());
    for raw in raw_instances {
        let instance_dir = base_instances_dir.join(&raw.path);
        let icon_path = match raw.icon_path {
            Some(p) if tokio::fs::try_exists(&p).await.unwrap_or(false) => {
                Some(PathBuf::from(p))
            }
            _ => None,
        };

        let (categories, worlds) = scan_content_categories(&instance_dir).await;

        instances.push(ImportInstanceCandidate {
            source_id: raw.id,
            instance_dir,
            name: raw.name,
            icon_path,
            loader: ModLoader::from_string(
                raw.loader.as_deref().unwrap_or("vanilla"),
            ),
            raw_loader: raw.loader,
            loader_version: raw.loader_version,
            game_version: raw.game_version,
            created: raw.created,
            modified: raw.modified,
            last_played: raw.last_played,
            categories,
            worlds,
        });
    }

    let (settings, mut compatibility_notes) =
        source_db::fetch_settings_candidate(pool).await?;

    let raw_java_versions = source_db::fetch_java_versions(pool).await?;
    let java_versions = raw_java_versions
        .into_iter()
        .map(|j| ImportJavaVersionCandidate {
            major_version: j.major_version as u32,
            full_version: j.full_version,
            architecture: j.architecture,
            path: j.path,
        })
        .collect();

    compatibility_notes.sort();
    compatibility_notes.dedup();

    Ok(ImportPreview {
        source: source.clone(),
        instances,
        settings,
        java_versions,
        compatibility_notes,
    })
}

/// How many `stat` calls to have in flight at once while sizing a category.
/// Some of these folders (screenshots, logs, a heavily-modded `config/`) can
/// hold thousands of files - matches `export_mrpack.rs`'s
/// `EXPORT_CANDIDATE_METADATA_CONCURRENCY`. Notably NOT used for `Saves` -
/// see `scan_saves_category`'s doc comment for why that one is handled
/// completely differently.
const CATEGORY_METADATA_CONCURRENCY: usize = 32;

async fn scan_content_categories(
    instance_dir: &Path,
) -> (Vec<ImportContentCategory>, Vec<ImportWorldCandidate>) {
    let scans = ContentCategory::ALL
        .into_iter()
        .filter(|category| *category != ContentCategory::Saves)
        .map(|category| {
            let folder = instance_dir.join(category.folder_name());
            async move { (category, scan_one_category(&folder).await) }
        });

    let mut categories: Vec<ImportContentCategory> = future::join_all(scans)
        .await
        .into_iter()
        .filter_map(|(category, sizing)| {
            let (file_count, total_size) = sizing?;
            Some(ImportContentCategory {
                category,
                file_count,
                total_size: Some(total_size),
                default_selected: category.default_selected(),
            })
        })
        .collect();

    let worlds = scan_saves_category(
        &instance_dir.join(ContentCategory::Saves.folder_name()),
    )
    .await;
    if !worlds.is_empty() {
        categories.push(ImportContentCategory {
            category: ContentCategory::Saves,
            file_count: worlds.len() as u64,
            total_size: None,
            default_selected: ContentCategory::Saves.default_selected(),
        });
    }

    (categories, worlds)
}

/// Returns `None` for an empty or nonexistent category folder (nothing to
/// offer), or `Some((file_count, total_size))` otherwise.
async fn scan_one_category(folder: &Path) -> Option<(u64, u64)> {
    let files = crate::api::pack::import::get_all_subfiles(folder, false)
        .await
        .ok()?;
    if files.is_empty() {
        return None;
    }

    let file_count = files.len() as u64;
    let total_size = stream::iter(files)
        .map(|file| async move {
            tokio::fs::metadata(&file)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
        })
        .buffer_unordered(CATEGORY_METADATA_CONCURRENCY)
        .fold(0u64, |acc, size| async move { acc + size })
        .await;

    Some((file_count, total_size))
}

/// Lists each top-level world folder under `saves/` without recursing into
/// it. A world's own on-disk size is not computed here: a real install's
/// `saves/` folder hit 330,924 files across 17 worlds in manual testing
/// (mod-generated cache data - Distant Horizons and similar - not the world
/// data itself), which made a full recursive size the dominant cost of the
/// entire preview. See `ImportContentCategory::total_size`'s doc comment.
async fn scan_saves_category(saves_dir: &Path) -> Vec<ImportWorldCandidate> {
    let Ok(mut read_dir) = crate::util::io::read_dir(saves_dir).await else {
        return Vec::new();
    };

    let mut worlds = Vec::new();
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(folder_name) = path.file_name() else {
            continue;
        };

        let modified = tokio::fs::metadata(&path)
            .await
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64);

        worlds.push(ImportWorldCandidate {
            folder_name: folder_name.to_string_lossy().to_string(),
            modified,
        });
    }

    worlds
}
