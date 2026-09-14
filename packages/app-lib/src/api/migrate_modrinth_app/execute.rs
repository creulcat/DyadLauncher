//! Phase 2: actually copying the content a user selected from a Phase 1
//! `ImportPreview` into an already-created Dyad instance. Everything here
//! assumes the caller (the install-job engine, see
//! `crate::install::runner`) already resolved the selection from a preview -
//! this module never touches the source SQLite database again.

use super::{
    ContentCategory, ImportJavaVersionCandidate, ImportSettingsCandidate,
};
use crate::install::{InstallPhaseDetails, InstallPhaseId, InstallProgress};
use crate::state::{JavaVersion, WindowSize};
use crate::util::fetch::{self, IoSemaphore};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What to copy for one instance, as chosen by the user from a Phase 1
/// `ImportPreview`. Persisted as part of an install job's request (see
/// `InstallRequest::ImportModrinthApp`), so this has to stay serializable
/// and stable enough to survive an app restart mid-job.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSelection {
    /// Whole-folder categories to copy (mods, resourcepacks, config, ...).
    /// `Saves` is accepted here too (as a plain "the user wants some
    /// worlds" signal) but never copied as a whole folder - see `worlds`.
    pub categories: Vec<ContentCategory>,
    /// Names of world folders under `saves/` to copy, matching
    /// `ImportWorldCandidate::folder_name` values from the preview.
    pub worlds: Vec<String>,
}

impl ImportSelection {
    /// Every source-relative path that should be copied whole: one per
    /// selected category folder, plus one `saves/<world>` per selected
    /// world. `Saves` itself is deliberately excluded from the plain
    /// category list - only specific selected worlds under it are copied.
    fn roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = self
            .categories
            .iter()
            .filter(|category| **category != ContentCategory::Saves)
            .map(|category| PathBuf::from(category.folder_name()))
            .collect();

        let saves_folder = ContentCategory::Saves.folder_name();
        for world in &self.worlds {
            roots.push(Path::new(saves_folder).join(world));
        }

        roots
    }
}

/// Lists every source file that a given selection resolves to, across all
/// its selected category/world roots. Pulled out of `copy_selected_content`
/// so this - the part with actual selection logic and Phase 1's
/// nonexistent-folder pitfall - can be unit-tested against real temp
/// directories without needing a running app `State`.
async fn resolve_copy_manifest(
    source_instance_dir: &Path,
    selection: &ImportSelection,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in selection.roots() {
        let src_root = source_instance_dir.join(&root);
        // Mirrors the fix from Phase 1's real-install testing:
        // `get_all_subfiles` treats a nonexistent path as a single
        // (unstattable) "file" rather than "no files", so a stale
        // selection pointing at a folder that no longer exists must be
        // skipped explicitly rather than trusted.
        if !src_root.is_dir() {
            continue;
        }

        let Ok(subfiles) =
            crate::api::pack::import::get_all_subfiles(&src_root, false).await
        else {
            continue;
        };
        files.extend(subfiles);
    }
    files
}

/// Copies exactly the selected categories/worlds from `source_instance_dir`
/// into the already-created Dyad instance `instance_id`, reporting combined
/// progress across all of them as one count.
///
/// Uses `InstallPhaseId::DownloadingContent`, which the install-job engine
/// specifically throttles for high-frequency per-file progress (see
/// `InstallProgressReporterState::should_persist`) - calling `update` once
/// per file here is cheap, not something this function needs to throttle
/// itself.
pub async fn copy_selected_content(
    instance_id: &str,
    source_instance_dir: &Path,
    selection: &ImportSelection,
    io_semaphore: &IoSemaphore,
    reporter: &crate::install::InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    let dest_instance_dir =
        crate::api::instance::get_full_path(instance_id).await?;
    let files = resolve_copy_manifest(source_instance_dir, selection).await;

    let total = files.len() as u64;
    if total == 0 {
        return Ok(());
    }

    for (index, src_file) in files.into_iter().enumerate() {
        let Ok(relative) = src_file.strip_prefix(source_instance_dir) else {
            continue;
        };
        let dst_file = dest_instance_dir.join(relative);
        fetch::copy(&src_file, &dst_file, io_semaphore).await?;

        reporter
            .update(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: (index + 1) as u64,
                    total,
                    secondary: None,
                }),
                details.clone(),
            )
            .await?;
    }

    Ok(())
}

/// Deletes exactly the source folders that `copy_selected_content` would
/// copy for this same `selection` - never anything else in the source
/// instance (its `.minecraft`-root files like `options.txt`, `servers.dat`,
/// or any other category, are left untouched). Callers must only invoke
/// this after that copy has already succeeded; it does not itself verify
/// anything was actually copied.
///
/// Backs goal 6's opt-in "delete from source after import" - copy is always
/// the default, this is only ever run when the user explicitly asked to
/// reclaim disk space in the source install.
pub async fn delete_selected_source_content(
    source_instance_dir: &Path,
    selection: &ImportSelection,
) -> crate::Result<()> {
    for root in selection.roots() {
        let path = source_instance_dir.join(&root);
        if path.is_dir() {
            crate::util::io::remove_dir_all(&path).await?;
        }
    }
    Ok(())
}

/// Which global settings/Java paths to import, as chosen by the user from a
/// Phase 1 `ImportPreview`. This is a plain settings edit, not an install
/// job - it happens immediately, with no progress bar or job-engine
/// bookkeeping needed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsImportSelection {
    pub extra_launch_args: bool,
    pub custom_env_vars: bool,
    pub memory_maximum: bool,
    pub force_fullscreen: bool,
    pub game_resolution: bool,
    /// Covers all three hooks (pre-launch/wrapper/post-exit) together,
    /// matching the goal's "pre-launch/post-exit hooks" granularity rather
    /// than offering three separate toggles.
    pub hooks: bool,
    pub java_versions: bool,
}

/// Applies the selected fields from a settings/Java-path preview onto
/// Dyad's own settings. Every field is independently optional: a selection
/// flag with no corresponding value in `candidate` (because that column
/// wasn't present in the source schema - see `ImportSettingsCandidate`) is
/// simply skipped rather than clearing the existing Dyad value.
///
/// Known limitation: imported Java paths point at the official Modrinth
/// App's own managed Java install (under its data directory), not a copy -
/// if that install is later uninstalled, Dyad's copied path would break.
/// Copying the actual Java runtime files is out of scope for this pass;
/// re-detecting Java is the existing fallback if a stored path goes stale.
pub async fn apply_settings(
    candidate: &ImportSettingsCandidate,
    java_versions: &[ImportJavaVersionCandidate],
    selection: &SettingsImportSelection,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    let mut settings = crate::state::Settings::get(&state.pool).await?;

    if selection.extra_launch_args
        && let Some(args) = &candidate.extra_launch_args
    {
        settings.extra_launch_args = args.clone();
    }
    if selection.custom_env_vars
        && let Some(vars) = &candidate.custom_env_vars
    {
        settings.custom_env_vars = vars.clone();
    }
    if selection.memory_maximum
        && let Some(maximum) = candidate.memory_maximum_mb
    {
        settings.memory.maximum = maximum;
    }
    if selection.force_fullscreen
        && let Some(force_fullscreen) = candidate.force_fullscreen
    {
        settings.force_fullscreen = force_fullscreen;
    }
    if selection.game_resolution
        && let Some((x, y)) = candidate.game_resolution
    {
        settings.game_resolution = WindowSize(x, y);
    }
    if selection.hooks {
        settings.hooks.pre_launch = candidate.hook_pre_launch.clone();
        settings.hooks.wrapper = candidate.hook_wrapper.clone();
        settings.hooks.post_exit = candidate.hook_post_exit.clone();
    }

    settings.update(&state.pool).await?;

    if selection.java_versions {
        for java in java_versions {
            let version = JavaVersion {
                parsed_version: java.major_version,
                version: java.full_version.clone(),
                architecture: java.architecture.clone(),
                path: java.path.clone(),
            };
            version.upsert(&state.pool).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Builds a fake instance folder under a fresh temp dir: `mods/`,
    /// `config/`, `resourcepacks/` each with one file, and two worlds under
    /// `saves/` each with one file. `shaderpacks/` is deliberately never
    /// created, standing in for a category the preview would never have
    /// offered (or one that vanished between preview and import).
    async fn fixture_instance_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let root = dir.path();

        for (path, contents) in [
            ("mods/a.jar", "a"),
            ("mods/b.jar", "b"),
            ("config/c.txt", "c"),
            ("resourcepacks/d.zip", "d"),
            ("saves/World1/level.dat", "world1"),
            ("saves/World2/level.dat", "world2"),
        ] {
            let file_path = root.join(path);
            tokio::fs::create_dir_all(file_path.parent().unwrap())
                .await
                .unwrap();
            tokio::fs::write(&file_path, contents).await.unwrap();
        }

        dir
    }

    fn relative_paths(root: &Path, files: &[PathBuf]) -> HashSet<String> {
        files
            .iter()
            .map(|file| {
                file.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[tokio::test]
    async fn manifest_includes_only_selected_categories() {
        let dir = fixture_instance_dir().await;
        let selection = ImportSelection {
            categories: vec![ContentCategory::Mods, ContentCategory::Config],
            worlds: Vec::new(),
        };

        let files = resolve_copy_manifest(dir.path(), &selection).await;
        let relative = relative_paths(dir.path(), &files);

        assert_eq!(
            relative,
            HashSet::from([
                "mods/a.jar".to_string(),
                "mods/b.jar".to_string(),
                "config/c.txt".to_string(),
            ])
        );
    }

    #[tokio::test]
    async fn manifest_includes_only_selected_worlds_even_if_saves_selected() {
        let dir = fixture_instance_dir().await;
        // `categories` including `Saves` is what a UI would send when the
        // user ticked the Saves checkbox generally - it must NOT cause the
        // whole `saves/` folder to be copied; only `worlds` controls that.
        let selection = ImportSelection {
            categories: vec![ContentCategory::Saves],
            worlds: vec!["World1".to_string()],
        };

        let files = resolve_copy_manifest(dir.path(), &selection).await;
        let relative = relative_paths(dir.path(), &files);

        assert_eq!(
            relative,
            HashSet::from(["saves/World1/level.dat".to_string()])
        );
    }

    #[tokio::test]
    async fn manifest_skips_a_category_folder_that_does_not_exist() {
        let dir = fixture_instance_dir().await;
        // ShaderPacks was never created by the fixture - a stale/tampered
        // selection naming it must not resurrect Phase 1's
        // "nonexistent-folder treated as one file" bug.
        let selection = ImportSelection {
            categories: vec![
                ContentCategory::Mods,
                ContentCategory::ShaderPacks,
            ],
            worlds: Vec::new(),
        };

        let files = resolve_copy_manifest(dir.path(), &selection).await;
        let relative = relative_paths(dir.path(), &files);

        assert_eq!(
            relative,
            HashSet::from(
                ["mods/a.jar".to_string(), "mods/b.jar".to_string(),]
            )
        );
    }

    #[tokio::test]
    async fn delete_only_removes_the_selected_categories_and_worlds() {
        let dir = fixture_instance_dir().await;
        let selection = ImportSelection {
            categories: vec![ContentCategory::Mods],
            worlds: vec!["World1".to_string()],
        };

        delete_selected_source_content(dir.path(), &selection)
            .await
            .unwrap();

        assert!(!dir.path().join("mods").exists());
        assert!(!dir.path().join("saves/World1").exists());
        // Everything not selected must survive untouched.
        assert!(dir.path().join("config/c.txt").exists());
        assert!(dir.path().join("resourcepacks/d.zip").exists());
        assert!(dir.path().join("saves/World2/level.dat").exists());
    }
}
