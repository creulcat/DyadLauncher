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

pub(crate) mod import_link;
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

/// The catch-all for everything at an instance's root that isn't one of the
/// 7 named `ContentCategory` folders and isn't a regenerable cache (see
/// `OTHER_FILES_EXCLUDED_ROOT_ENTRIES`) - loose per-instance files like
/// `options.txt`, `servers.dat`, `usercache.json`, `hotbar.nbt`, and misc
/// folders like `backups/`, `waypoints/`, mod-specific data directories,
/// none of which the original 7-category scan ever covered. Added after
/// real-install testing found settings silently missing post-import even
/// with every offered category selected.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOtherFilesCandidate {
    pub file_count: u64,
    pub total_size: u64,
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

/// A whole category folder (e.g. `mods/`) or a whole top-level world folder
/// (`saves/<world>`) that is itself a symlink or Windows junction, detected
/// so the user can choose what should happen to it instead of it being
/// silently flattened into a full copy of whatever it points at. Only
/// directory-shaped links at these two spots are detected - a symlink
/// nested deeper inside an otherwise-real folder tree, or one pointing at a
/// single file, is left to be copied as real content like today, since
/// handling those would mean rewriting the whole-folder copy into a
/// per-file model.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSymlinkCandidate {
    /// Matches a `roots()` entry in `execute::ImportSelection` - either a
    /// bare category folder name (e.g. `"mods"`) or `"saves/<world>"`.
    pub relative_path: String,
    pub category: ContentCategory,
    /// Absolute path this link ultimately resolves to.
    pub target: PathBuf,
    /// `false` means `target` lies inside the official Modrinth App's own
    /// managed directories (its settings dir or config dir) - recreating the
    /// link here would leave the imported instance depending on the source
    /// install staying in place, so callers should treat "recreate" as
    /// "copy the real content instead" for this one.
    pub target_outside_source_app: bool,
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
    /// Total seconds played on the source instance - its
    /// `submitted_time_played + recent_time_played` collapsed into one
    /// figure, since the split only matters for the source app's own
    /// reporting cadence, not to Dyad.
    pub total_time_played: u64,
    /// Only categories that actually have files are included. A category
    /// that's itself a symlink/junction is never listed here - see
    /// `symlinks` instead.
    pub categories: Vec<ImportContentCategory>,
    /// Per-world breakdown backing the `Saves` category, when present. A
    /// world that's itself a symlink/junction is still listed here (for its
    /// name/modified date) as well as in `symlinks`.
    pub worlds: Vec<ImportWorldCandidate>,
    /// Whole category folders or world folders that are themselves a
    /// symlink/junction - see `ImportSymlinkCandidate`.
    pub symlinks: Vec<ImportSymlinkCandidate>,
    /// Loose root-level files/folders outside the 7 named categories - see
    /// `ImportOtherFilesCandidate`. `None` if there's nothing there (or
    /// everything there is an excluded regenerable cache).
    pub other_files: Option<ImportOtherFilesCandidate>,
    /// This instance's own launch overrides (JVM args, memory, hooks, a
    /// specific Java path, ...) read from the source, if any were set and
    /// the JSON could be read. `None` if the source has no row for this
    /// instance at all, or nothing in it parsed to anything meaningful.
    pub launch_overrides: Option<ImportLaunchOverridesCandidate>,
    /// Set if this exact source instance was already imported into a Dyad
    /// instance that still exists, from a prior import run (see
    /// `import_link::find_existing_import`) - lets the UI warn on a re-import
    /// instead of silently creating a duplicate instance.
    pub already_imported: Option<import_link::ExistingImport>,
    /// `true` if `game_version`/`loader`/`loader_version` came from a
    /// best-effort fallback (the source instance's most recently modified
    /// content set) rather than its currently-`applied_content_set_id` row -
    /// see `resolve_game_version_fallback`. The UI should flag this so the
    /// user can double-check it before importing.
    pub game_version_inferred: bool,
}

/// A source instance's own `instance_launch_overrides` row, read
/// defensively field-by-field from its JSON blob rather than deserialized
/// directly into a fixed shape - see the compatibility strategy in
/// docs/goal-6-import-design.md. Every field independently optional, same
/// shape as `ImportSettingsCandidate` plus a specific Java path (global
/// settings import instead offers a *list* of detected installs to pick
/// from - see `ImportJavaVersionCandidate` - since a per-instance override
/// names exactly one).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLaunchOverridesCandidate {
    pub java_path: Option<String>,
    pub extra_launch_args: Option<Vec<String>>,
    pub custom_env_vars: Option<Vec<(String, String)>>,
    pub memory_maximum_mb: Option<u32>,
    pub force_fullscreen: Option<bool>,
    pub game_resolution: Option<(u16, u16)>,
    pub hook_pre_launch: Option<String>,
    pub hook_wrapper: Option<String>,
    pub hook_post_exit: Option<String>,
}

impl ImportLaunchOverridesCandidate {
    fn is_empty(&self) -> bool {
        self.java_path.is_none()
            && self.extra_launch_args.is_none()
            && self.custom_env_vars.is_none()
            && self.memory_maximum_mb.is_none()
            && self.force_fullscreen.is_none()
            && self.game_resolution.is_none()
            && self.hook_pre_launch.is_none()
            && self.hook_wrapper.is_none()
            && self.hook_post_exit.is_none()
    }
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
    /// The source app's resolved config dir - its `custom_dir` setting if
    /// set and still on disk, otherwise `source.settings_dir` (see
    /// `DirectoryInfo.config_dir`). Instances live under
    /// `source_config_dir/profiles`, and the app-level synced-options store
    /// (see `execute::migrate_synced_options_store`) lives directly under
    /// `source_config_dir` - exposed here so the frontend doesn't have to
    /// re-derive it to call that separately.
    pub source_config_dir: PathBuf,
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

    // Anywhere under these is considered "owned by the source app" for
    // symlink-safety purposes - see `ImportSymlinkCandidate::target_outside_source_app`.
    let mut source_roots = Vec::new();
    if let Ok(canon) = tokio::fs::canonicalize(&config_dir).await {
        source_roots.push(canon);
    }
    if let Ok(canon) = tokio::fs::canonicalize(&source.settings_dir).await
        && !source_roots.contains(&canon)
    {
        source_roots.push(canon);
    }

    let raw_instances = source_db::fetch_instances(pool).await?;
    // Keyed by source instance id, most-recently-modified content set first -
    // a best-effort fallback for instances whose `applied_content_set_id`
    // link is null/orphaned (see `resolve_game_version_fallback`) instead of
    // treating them as having no game version at all.
    let mut content_set_fallbacks: std::collections::HashMap<
        String,
        source_db::RawContentSet,
    > = std::collections::HashMap::new();
    for content_set in source_db::fetch_content_sets_by_recency(pool).await? {
        content_set_fallbacks
            .entry(content_set.instance_id.clone())
            .or_insert(content_set);
    }

    let state = crate::State::get().await?;
    let mut instances = Vec::with_capacity(raw_instances.len());
    for raw in raw_instances {
        let instance_dir = base_instances_dir.join(&raw.path);
        let icon_path = match &raw.icon_path {
            Some(p) if tokio::fs::try_exists(p).await.unwrap_or(false) => {
                Some(PathBuf::from(p))
            }
            _ => None,
        };

        let (categories, worlds, symlinks, other_files) =
            scan_content_categories(&instance_dir, &source_roots).await;
        let launch_overrides =
            source_db::fetch_launch_overrides(pool, &raw.id)
                .await
                .filter(|overrides| !overrides.is_empty());

        let (game_version, loader, loader_version, game_version_inferred) =
            resolve_game_version_fallback(&raw, &content_set_fallbacks);

        let already_imported = import_link::find_existing_import(
            &state.pool,
            &source.settings_dir,
            &raw.id,
        )
        .await?
        .filter(|existing| existing.instance_name.is_some());

        instances.push(ImportInstanceCandidate {
            source_id: raw.id,
            instance_dir,
            name: raw.name,
            icon_path,
            loader: ModLoader::from_string(
                loader.as_deref().unwrap_or("vanilla"),
            ),
            raw_loader: loader,
            loader_version,
            game_version,
            created: raw.created,
            modified: raw.modified,
            last_played: raw.last_played,
            total_time_played: (raw.submitted_time_played.max(0)
                + raw.recent_time_played.max(0))
                as u64,
            categories,
            worlds,
            symlinks,
            other_files,
            launch_overrides,
            already_imported,
            game_version_inferred,
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
        source_config_dir: config_dir,
        instances,
        settings,
        java_versions,
        compatibility_notes,
    })
}

/// Resolves an instance's game version/loader/loader_version, falling back
/// to its most recently modified `instance_content_sets` row (from
/// `fallbacks`, keyed by source instance id) when the row `raw` joined via
/// `applied_content_set_id` came back empty - e.g. because that pointer is
/// null or references a content set that's since been deleted, orphaning an
/// otherwise-real, previously-played instance. Returns
/// `(game_version, loader, loader_version, used_fallback)`.
fn resolve_game_version_fallback(
    raw: &source_db::RawInstance,
    fallbacks: &std::collections::HashMap<String, source_db::RawContentSet>,
) -> (Option<String>, Option<String>, Option<String>, bool) {
    if raw.game_version.is_some() {
        return (
            raw.game_version.clone(),
            raw.loader.clone(),
            raw.loader_version.clone(),
            false,
        );
    }

    match fallbacks.get(&raw.id) {
        Some(fallback) => (
            Some(fallback.game_version.clone()),
            Some(fallback.loader.clone()),
            fallback.loader_version.clone(),
            true,
        ),
        None => (None, None, None, false),
    }
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
    source_roots: &[PathBuf],
) -> (
    Vec<ImportContentCategory>,
    Vec<ImportWorldCandidate>,
    Vec<ImportSymlinkCandidate>,
    Option<ImportOtherFilesCandidate>,
) {
    let mut categories = Vec::new();
    let mut symlinks = Vec::new();

    let scans = ContentCategory::ALL
        .into_iter()
        .filter(|category| *category != ContentCategory::Saves)
        .map(|category| {
            let folder = instance_dir.join(category.folder_name());
            async move {
                let symlink = detect_symlink(
                    &folder,
                    category,
                    category.folder_name().to_string(),
                    source_roots,
                )
                .await;
                if symlink.is_some() {
                    return (category, None, symlink);
                }
                (category, scan_one_category(&folder).await, None)
            }
        });

    for (category, sizing, symlink) in future::join_all(scans).await {
        if let Some(symlink) = symlink {
            symlinks.push(symlink);
            continue;
        }
        if let Some((file_count, total_size)) = sizing {
            categories.push(ImportContentCategory {
                category,
                file_count,
                total_size: Some(total_size),
                default_selected: category.default_selected(),
            });
        }
    }

    let saves_dir = instance_dir.join(ContentCategory::Saves.folder_name());
    let worlds = scan_saves_category(&saves_dir).await;
    if !worlds.is_empty() {
        categories.push(ImportContentCategory {
            category: ContentCategory::Saves,
            file_count: worlds.len() as u64,
            total_size: None,
            default_selected: ContentCategory::Saves.default_selected(),
        });
    }
    for world in &worlds {
        if let Some(symlink) = detect_symlink(
            &saves_dir.join(&world.folder_name),
            ContentCategory::Saves,
            format!("saves/{}", world.folder_name),
            source_roots,
        )
        .await
        {
            symlinks.push(symlink);
        }
    }

    let other_files = scan_other_files(instance_dir).await;

    (categories, worlds, symlinks, other_files)
}

/// Root-level entries inside an instance folder that always excluded from
/// the catch-all "everything else" copy because they're regenerable
/// loader/asset caches, not user data or settings - `.fabric` in particular
/// held 40,000+ small processed/remapped-jar files in real-install testing
/// (see the CreulCat findings), fully rebuilt by Fabric on next launch.
/// Case-sensitive: these are the exact names the official app creates.
const OTHER_FILES_EXCLUDED_ROOT_ENTRIES: [&str; 2] = [".fabric", ".cache"];

/// Lists the top-level entries directly under `instance_dir` that aren't one
/// of the 7 known `ContentCategory` folders and aren't an excluded
/// regenerable cache - see `ImportOtherFilesCandidate`. Each returned path
/// can be a file or a directory; recursing into a directory entry is left to
/// the caller (`get_all_subfiles`, same as every other category).
pub(crate) async fn other_root_entries(instance_dir: &Path) -> Vec<PathBuf> {
    let known_category_folders: Vec<&str> = ContentCategory::ALL
        .iter()
        .map(|category| category.folder_name())
        .collect();

    let Ok(mut read_dir) = crate::util::io::read_dir(instance_dir).await
    else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if known_category_folders.contains(&name)
            || OTHER_FILES_EXCLUDED_ROOT_ENTRIES.contains(&name)
        {
            continue;
        }
        entries.push(path);
    }
    entries
}

/// Sizes every `other_root_entries` entry the same way `scan_one_category`
/// sizes a single category folder - one combined file count/total size
/// across every loose file and misc folder at the instance root.
async fn scan_other_files(
    instance_dir: &Path,
) -> Option<ImportOtherFilesCandidate> {
    let entries = other_root_entries(instance_dir).await;
    if entries.is_empty() {
        return None;
    }

    let mut files = Vec::new();
    for entry in entries {
        if let Ok(subfiles) =
            crate::api::pack::import::get_all_subfiles(&entry, false).await
        {
            files.extend(subfiles);
        }
    }
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

    Some(ImportOtherFilesCandidate {
        file_count,
        total_size,
    })
}

/// Checks whether `path` is itself a symlink/junction resolving to a
/// directory - the only shape this reader special-cases (see
/// `ImportSymlinkCandidate`'s doc comment for why file-level and
/// deeper-nested links are left alone). Returns `None` for anything else,
/// including a broken link (nothing sensible to offer for it).
async fn detect_symlink(
    path: &Path,
    category: ContentCategory,
    relative_path: String,
    source_roots: &[PathBuf],
) -> Option<ImportSymlinkCandidate> {
    let metadata = tokio::fs::symlink_metadata(path).await.ok()?;
    if !metadata.file_type().is_symlink() {
        return None;
    }

    let target = tokio::fs::canonicalize(path).await.ok()?;
    if !target.is_dir() {
        return None;
    }

    let target_outside_source_app =
        !source_roots.iter().any(|root| path_is_within(&target, root));

    Some(ImportSymlinkCandidate {
        relative_path,
        category,
        target,
        target_outside_source_app,
    })
}

/// Component-wise, case-insensitive-on-Windows check for whether `path` lies
/// under `root`. Both should already be canonicalized. Component-wise
/// (rather than a naive string prefix) so `/foo/bar2` never matches root
/// `/foo/bar`.
fn path_is_within(path: &Path, root: &Path) -> bool {
    let mut path_components = path.components();
    for root_component in root.components() {
        let Some(path_component) = path_components.next() else {
            return false;
        };
        let matches = if cfg!(windows) {
            path_component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&root_component.as_os_str().to_string_lossy())
        } else {
            path_component == root_component
        };
        if !matches {
            return false;
        }
    }
    true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_within_matches_a_real_prefix() {
        assert!(path_is_within(
            Path::new("/home/user/App/profiles/X"),
            Path::new("/home/user/App"),
        ));
    }

    #[test]
    fn path_is_within_rejects_a_sibling_with_a_shared_string_prefix() {
        // A naive string-prefix check would wrongly match "/home/user/App2"
        // against root "/home/user/App" - component-wise comparison must not.
        assert!(!path_is_within(
            Path::new("/home/user/App2/profiles/X"),
            Path::new("/home/user/App"),
        ));
    }

    #[test]
    fn path_is_within_rejects_an_unrelated_path() {
        assert!(!path_is_within(
            Path::new("/mnt/d/Worlds/MyWorld"),
            Path::new("/home/user/App"),
        ));
    }

    fn raw_instance(id: &str, game_version: Option<&str>) -> source_db::RawInstance {
        source_db::RawInstance {
            id: id.to_string(),
            path: id.to_string(),
            name: id.to_string(),
            icon_path: None,
            created: 0,
            modified: 0,
            last_played: None,
            submitted_time_played: 0,
            recent_time_played: 0,
            game_version: game_version.map(str::to_string),
            loader: game_version.map(|_| "fabric".to_string()),
            loader_version: None,
        }
    }

    #[test]
    fn game_version_fallback_prefers_the_applied_content_set() {
        let raw = raw_instance("i1", Some("1.20.1"));
        let fallbacks = std::collections::HashMap::from([(
            "i1".to_string(),
            source_db::RawContentSet {
                instance_id: "i1".to_string(),
                game_version: "1.19.2".to_string(),
                loader: "forge".to_string(),
                loader_version: None,
            },
        )]);

        let (game_version, loader, _, inferred) =
            resolve_game_version_fallback(&raw, &fallbacks);

        assert_eq!(game_version.as_deref(), Some("1.20.1"));
        assert_eq!(loader.as_deref(), Some("fabric"));
        assert!(!inferred);
    }

    #[test]
    fn game_version_fallback_used_when_applied_content_set_is_missing() {
        // Mirrors an instance whose `applied_content_set_id` is null/orphaned
        // in the source database (e.g. the CreulCat real-install case) but
        // still has a real, previously-applied content set on record.
        let raw = raw_instance("i1", None);
        let fallbacks = std::collections::HashMap::from([(
            "i1".to_string(),
            source_db::RawContentSet {
                instance_id: "i1".to_string(),
                game_version: "1.19.2".to_string(),
                loader: "forge".to_string(),
                loader_version: Some("47.2.0".to_string()),
            },
        )]);

        let (game_version, loader, loader_version, inferred) =
            resolve_game_version_fallback(&raw, &fallbacks);

        assert_eq!(game_version.as_deref(), Some("1.19.2"));
        assert_eq!(loader.as_deref(), Some("forge"));
        assert_eq!(loader_version.as_deref(), Some("47.2.0"));
        assert!(inferred);
    }

    #[test]
    fn game_version_fallback_stays_none_with_no_content_set_at_all() {
        let raw = raw_instance("i1", None);
        let (game_version, loader, loader_version, inferred) =
            resolve_game_version_fallback(&raw, &std::collections::HashMap::new());

        assert!(game_version.is_none());
        assert!(loader.is_none());
        assert!(loader_version.is_none());
        assert!(!inferred);
    }

    #[tokio::test]
    async fn other_root_entries_excludes_known_categories_and_caches() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let instance_dir = dir.path();

        for name in ["mods", "config", "saves", ".fabric", ".cache"] {
            tokio::fs::create_dir(instance_dir.join(name)).await.unwrap();
        }
        tokio::fs::write(instance_dir.join("servers.dat"), "s").await.unwrap();
        tokio::fs::create_dir(instance_dir.join("backups")).await.unwrap();

        let entries = other_root_entries(instance_dir).await;
        let names: std::collections::HashSet<String> = entries
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            names,
            std::collections::HashSet::from([
                "servers.dat".to_string(),
                "backups".to_string(),
            ])
        );
    }

    #[cfg(unix)]
    mod unix_symlinks {
        use super::*;

        #[tokio::test]
        async fn detect_symlink_ignores_a_real_directory() {
            let dir = tempfile::tempdir().unwrap();
            let real = dir.path().join("mods");
            tokio::fs::create_dir(&real).await.unwrap();

            let result = detect_symlink(
                &real,
                ContentCategory::Mods,
                "mods".to_string(),
                &[],
            )
            .await;
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn detect_symlink_ignores_a_symlinked_file() {
            let dir = tempfile::tempdir().unwrap();
            let real_file = dir.path().join("real.txt");
            tokio::fs::write(&real_file, "hi").await.unwrap();
            let link = dir.path().join("linked.txt");
            std::os::unix::fs::symlink(&real_file, &link).unwrap();

            let result = detect_symlink(
                &link,
                ContentCategory::Config,
                "config".to_string(),
                &[],
            )
            .await;
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn detect_symlink_ignores_a_broken_link() {
            let dir = tempfile::tempdir().unwrap();
            let link = dir.path().join("broken");
            std::os::unix::fs::symlink(dir.path().join("nonexistent"), &link)
                .unwrap();

            let result = detect_symlink(
                &link,
                ContentCategory::Mods,
                "mods".to_string(),
                &[],
            )
            .await;
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn detect_symlink_flags_a_target_outside_the_source_app() {
            let source_app = tempfile::tempdir().unwrap();
            let external = tempfile::tempdir().unwrap();
            let real_dir = external.path().join("SharedMods");
            tokio::fs::create_dir(&real_dir).await.unwrap();

            let instance_dir = source_app.path().join("profiles/Instance");
            tokio::fs::create_dir_all(&instance_dir).await.unwrap();
            let link = instance_dir.join("mods");
            std::os::unix::fs::symlink(&real_dir, &link).unwrap();

            let source_roots =
                [tokio::fs::canonicalize(source_app.path()).await.unwrap()];
            let result = detect_symlink(
                &link,
                ContentCategory::Mods,
                "mods".to_string(),
                &source_roots,
            )
            .await
            .expect("a directory symlink should be detected");

            assert!(result.target_outside_source_app);
            assert_eq!(
                result.target,
                tokio::fs::canonicalize(&real_dir).await.unwrap()
            );
        }

        #[tokio::test]
        async fn detect_symlink_flags_a_target_inside_the_source_app() {
            let source_app = tempfile::tempdir().unwrap();
            let shared_dir = source_app.path().join("profiles/Other/mods");
            tokio::fs::create_dir_all(&shared_dir).await.unwrap();

            let instance_dir = source_app.path().join("profiles/Instance");
            tokio::fs::create_dir_all(&instance_dir).await.unwrap();
            let link = instance_dir.join("mods");
            std::os::unix::fs::symlink(&shared_dir, &link).unwrap();

            let source_roots =
                [tokio::fs::canonicalize(source_app.path()).await.unwrap()];
            let result = detect_symlink(
                &link,
                ContentCategory::Mods,
                "mods".to_string(),
                &source_roots,
            )
            .await
            .expect("a directory symlink should be detected");

            assert!(!result.target_outside_source_app);
        }

        #[tokio::test]
        async fn scan_content_categories_excludes_a_symlinked_category_from_categories()
         {
            let dir = tempfile::tempdir().unwrap();
            let instance_dir = dir.path().join("Instance");
            tokio::fs::create_dir_all(&instance_dir).await.unwrap();

            // A real, normally-scanned category.
            let config_dir = instance_dir.join("config");
            tokio::fs::create_dir(&config_dir).await.unwrap();
            tokio::fs::write(config_dir.join("a.txt"), "a").await.unwrap();

            // A whole-category symlink.
            let external = tempfile::tempdir().unwrap();
            let real_mods = external.path().join("SharedMods");
            tokio::fs::create_dir(&real_mods).await.unwrap();
            std::os::unix::fs::symlink(&real_mods, instance_dir.join("mods"))
                .unwrap();

            let (categories, _worlds, symlinks, _other_files) =
                scan_content_categories(&instance_dir, &[]).await;

            assert!(
                categories.iter().all(|c| c.category != ContentCategory::Mods),
                "a symlinked category must not appear in categories: {categories:?}"
            );
            assert!(
                categories.iter().any(|c| c.category == ContentCategory::Config),
                "a real category must still be scanned normally: {categories:?}"
            );
            assert_eq!(symlinks.len(), 1);
            assert_eq!(symlinks[0].relative_path, "mods");
            assert!(symlinks[0].target_outside_source_app);
        }
    }
}
