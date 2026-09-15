# Goal 6 design note: importing from the official Modrinth App

Design and implementation notes for [GOALS.md](GOALS.md) goal 6, updated as each phase lands new
facts or code. Treat it as a living companion to GOALS.md, not a one-time spec.

- **Phase 0** (below, "Schema drift" through "Confirmed source tables"): discovery/strategy only,
  no code.
- **Phase 1** ("Real-install validation" and earlier "Still open" items): read-only detection and
  preview, in `packages/app-lib/src/api/migrate_modrinth_app/{mod,source_db}.rs`.
- **Phase 2** (bottom of this doc): actual instance creation and content copying, in
  `packages/app-lib/src/api/migrate_modrinth_app/execute.rs` and the `install` job engine.

## Schema drift, measured

Dyad's `beta-release` branch has actually kept pulling upstream `modrinth/code` commits well past
the July 2026 branding-compliance fork point — its `packages/app-lib/migrations/` history matches
upstream all the way through `20260828120000_reset-synced-option-defaults.sql`. The real
divergence, checked by fetching `modrinth/code`'s `main` (`git fetch
https://github.com/modrinth/code.git main`, not added as a permanent remote) and diffing
`packages/app-lib/migrations/` at each HEAD, is much smaller than "everything since July" — it's
exactly:

**Upstream has added since Dyad's last sync (2026-08-28 → 2026-09-11), Dyad does not:**
- `20260827120000_game-options-sync.sql`, `20260902120000_enable-synced-options-for-fresh-installs.sql`,
  `20260902140000_sync-features-setting.sql`, `20260903120000_synced-packs.sql`,
  `20260908120000_game-option-locales.sql`, `20260911120000_linked-server-project-source.sql` — all
  part of upstream's cross-device settings-sync feature, which depends on a signed-in Modrinth
  account. Irrelevant to us: Dyad has no accounts (goal 3), so none of this data can mean anything
  here.
- `20260902130000_promote-instance-navigation-settings.sql` — adds 4 UI-preference columns to
  `settings` (`show_files_tab_in_instances`, `show_worlds_tab_in_instances`,
  `show_screenshots_tab_in_instances`, `show_skin_selector_in_sidebar`). Cosmetic; not in our
  import allowlist below, safely ignorable.

**Dyad has removed that upstream still has:**
- `20260904120000_remove-telemetry-and-ads-settings.sql` (drops `settings.telemetry`,
  `settings.personalized_ads`), `20260905120000_remove-modrinth-account.sql` (drops
  `modrinth_users`), `20260907120000_remove-onboarding-checklist.sql` (drops
  `onboarding_checklist`). All deliberate (goal 3) and all things we'd never import anyway.

Core takeaway: **the tables this feature actually touches — `instances`, `instance_content_sets`,
`instance_launch_overrides`, `instance_icon_configs`, `java_versions` — are currently
byte-for-byte identical between Dyad and upstream.** `settings` differs only by columns we don't
need. This is a good sign for feasibility, but it's a snapshot — a source install could be running
an older or newer official-app version than what upstream's `main` has today, so the reader still
has to be defensive rather than assume this alignment holds (see Compatibility strategy below).

Verification artifacts (not committed, reproducible from the migrations above): materialized
`dyad-schema.db`/`upstream-schema.db` by pointing `sqlx migrate run` at each commit's
`migrations/` folder, then inspected with Node 24's built-in `node:sqlite` (`PRAGMA
table_info(...)`, `sqlite_master`) since neither a `sqlite3` CLI nor Python is available in this
environment.

## Compatibility strategy

- **Never** `SELECT *`, and never attach-and-bulk-copy a source table into Dyad's own DB. Every
  read is an explicit, named-column `SELECT` against the source `app.db`, opened read-only.
- Probe `PRAGMA table_info(<table>)` on the source DB before querying it, so a missing *optional*
  column degrades to a default instead of erroring, and an unrecognized/missing table (or a
  required column that's gone) fails with a specific, named error ("expected column `X` on table
  `Y`, not found — this Modrinth App version may be too old/new to import from") rather than a
  generic SQL error or, worse, silently importing garbage.
- No schema-version allowlist beyond that: given how closely the two schemas track today, checking
  column presence per-field is simpler and more forgiving than trying to pin supported migration
  ranges via `_sqlx_migrations`.
- `minecraft_users` (Microsoft/Minecraft account tokens) is **explicitly out of scope and must
  never be read** — this feature imports instances/content/settings, not credentials. Worth a
  code comment at the call site, not just an omission, so it's not "rediscovered" as a gap later.

## Locked-database detection

The source app's `app.db` is opened in WAL mode ([db.rs](../packages/app-lib/src/state/db.rs)).
Plan: open a second, independent read-only connection to the source file and probe with `BEGIN
IMMEDIATE; ROLLBACK;` — this fails with `SQLITE_BUSY` if the official app still holds the write
lock, giving a clean "close Modrinth App first" signal without needing to inspect `-wal`/`-shm`
files directly. To be validated for real in Phase 1 against an actual running install.

## Confirmed source tables and the import allowlist

From `packages/app-lib/migrations/` (identical between Dyad and upstream as of this check):

| Data wanted | Table(s) | Columns |
|---|---|---|
| Instance identity | `instances` | `id`, `path`, `name`, `icon_path`, `created`, `modified`, `last_played`, `applied_content_set_id` |
| Loader / MC version | `instance_content_sets` (joined via `applied_content_set_id`) | `game_version`, `loader`, `loader_version` |
| Icon | `instance_icon_configs` | `background`, `symbol` (fallback if `icon_path` is unset) |
| Per-instance launch overrides | `instance_launch_overrides` | `overrides` (JSONB blob — parse defensively field-by-field like [legacy_converter.rs](../packages/app-lib/src/state/legacy_converter.rs) does for the old JSON format, not deserialize-and-trust) |
| Java paths | `java_versions` | `major_version`, `full_version`, `architecture`, `path` |
| Global settings (allowlist only) | `settings` | `extra_launch_args`, `custom_env_vars`, `mc_memory_max`, `mc_force_fullscreen`, `mc_game_resolution_x`, `mc_game_resolution_y`, `hook_pre_launch`, `hook_wrapper`, `hook_post_exit` |
| Never read | `minecraft_users` | — (credentials; out of scope, see above) |

Content-category file selection (mods/resourcepacks/shaderpacks/config/saves/screenshots/logs)
reads from each instance's on-disk folder (via `instances.path`), not the DB — same approach as
[export_mrpack.rs](../packages/app-lib/src/api/instance/export_mrpack.rs)'s
`PackExportCandidate`/`ExportSelection` tree, which Phase 1/3 will adapt rather than reinvent.

## Still open for Phase 1

- Decide exactly which `instance_launch_overrides.overrides` JSON fields are worth carrying over
  (e.g. per-instance memory/JVM-arg overrides) versus left as Dyad defaults — needs a look at the
  current `InstanceLaunchOverrides`/`InstanceLaunchOverridesData` shape in
  `packages/app-lib/src/state/instances/model/launch.rs`.
- Confirm the official app's default data-dir name (`ModrinthApp`) hasn't changed in any recent
  upstream release, the way Dyad's own identifier was deliberately changed in
  `b8cb87ca0` ("Rename app identity so it doesn't clash with a real Modrinth App install").
- `Saves` category sizing is now decided and implemented — see finding below.

## Real-install validation (2026-09-13)

Ran the Phase 1 code (via a `#[ignore]`d integration test, `tests/manual_modrinth_app_preview.rs`,
not part of the normal suite) against a real official Modrinth App install with 7 real instances.
Two real bugs surfaced immediately — exactly the value of testing against a live install instead
of only fixtures:

1. **Wrong instance-folder resolution when `custom_dir` is set.** `settings.custom_dir` is not
   "the profiles folder" — it's the app's *config* dir override (equivalent to Dyad's own
   `DirectoryInfo.config_dir`), and profiles always live at `<config_dir>/profiles`. The real
   install had `custom_dir` populated (to the same value as its own settings dir, apparently set
   unconditionally rather than left `NULL` when unused), and the code used it directly as the
   instances folder, skipping the `profiles` join entirely. Every category for every instance came
   back as "1 file, 0 bytes" — `get_all_subfiles` was being pointed at a nonexistent path and
   falling back to treating that missing path itself as a single (unstattable) "file". Fixed in
   `build_preview_from_pool` and renamed the misleading `read_custom_instances_dir` to
   `read_custom_config_dir` to match.

2. **Sequential-then-underparallelized folder scanning is too slow for real worlds.** One instance
   had 17 worlds whose per-world mod-generated cache folders (Distant Horizons' LOD data, judging
   by the mods list) added up to 330,924 files / 270GB under `saves/`. The first working version
   took over 80 seconds just for that one instance. Parallelizing the final `stat()` calls
   (`futures::stream::buffer_unordered`, matching `export_mrpack.rs`'s existing
   `EXPORT_CANDIDATE_METADATA_CONCURRENCY` pattern) cut it to ~32s, but raising concurrency further
   (32 → 256) barely moved that number — confirming the actual bottleneck is `get_all_subfiles`'s
   recursive directory *walk* itself (fully sequential `read_dir`/`next_entry` calls across
   hundreds of nested per-world cache folders), not the metadata-stat step layered on top.

   **Fixed, per a decision with the user 2026-09-13:** `Saves` is no longer sized by a single
   recursive walk. `ImportInstanceCandidate` now carries a `worlds: Vec<ImportWorldCandidate>`
   (top-level `saves/` subfolders, each with just a name and modified-time - no recursion into
   it), and `ImportContentCategory::total_size` became `Option<u64>` with `Saves` the one category
   that's always `None` (its `file_count` is the world count instead of a file count). Exact
   byte-accurate sizing of a world, if ever wanted, is deferred to Phase 2's actual copy, where a
   progress bar makes a slower walk acceptable. Result against the same real install: **82s → 32s
   (parallelized stat calls) → 0.4s** (skipping the recursive walk into `saves/` entirely).

All other categories/instances (mods, resourcepacks, shaderpacks, config, screenshots, logs across
all 7 instances) previewed correctly and quickly once fix #1 landed. Settings and Java-path
candidates also matched the real install's values exactly.

3. **The `BEGIN IMMEDIATE` lock probe from Phase 0's design doesn't work at all - a false
   negative, confirmed live.** With the real Modrinth App running (confirmed via `Get-Process`),
   `is_source_locked` still reported `false`. Root cause: SQLite's WAL mode only holds the write
   lock for the duration of an actual write *transaction*, not for as long as a connection/app is
   open, so the probe only had a chance of detecting a razor-thin race window, not "is the app
   currently running." This wasn't a tuning problem - the whole approach was the wrong tool.

   **Fixed:** replaced with a process-list check (`is_source_running`, via `sysinfo` - already a
   dependency, and the same style of check `tauri-plugin-single-instance` uses for Dyad's own
   single-instance enforcement), matching by process name containing "modrinth"
   case-insensitively. Validated both directions against the real install: `false` before it was
   launched, `true` with it confirmed running via `Get-Process`. The Tauri command
   `is_modrinth_app_running` dropped its now-pointless `db_path` parameter and current signature
   is a plain sync `bool` check.

This is exactly the outcome real-install testing was for: two of these three findings would not
have been caught by fixture-based unit tests alone (the `custom_dir` value and the lock-probe's
false negative both required a real database and a real running process to surface).

## Phase 2: actual instance creation and content copying (2026-09-13)

**Architecture decision:** rather than a bespoke one-off writer, this landed as a new
`InstallRequest::ImportModrinthApp` variant in Dyad's existing install-job engine
(`packages/app-lib/src/install/{model,runner,recovery}.rs`) - the same shared system every other
"create/populate an instance" flow (plain creation, modpack installs, the existing
MultiMC/CurseForge/etc. importer) already goes through. This gets progress reporting, resuming
after an app crash/restart, and rollback-on-failure for free, and keeps a Modrinth-App import
showing up in the UI exactly like every other install job.

One simplification versus the existing multi-launcher importer: that one creates the instance
with placeholder values (`"1.19.4"`, vanilla) and corrects them after parsing the source's
metadata files mid-import. Here, the real name/game version/loader/loader version are already
known exactly from the Phase 1 preview, so the instance is created correctly from the start - no
placeholder-then-correct step.

**What actually happens, per selected instance (one install job each):**
1. `crate::api::instance::create(...)` with the real values from the preview, `InstanceLink::Unmanaged`.
   The source's icon path (absolute, into the *source* app's own icon cache) is passed straight
   through as the `icon_path` string - `PathBuf::join` on an already-absolute path just becomes
   that path, so the existing `resolve_icon_path`/`cache_icon_from_path` machinery re-caches it
   into Dyad's own cache correctly with no special-casing needed.
2. Copy the selected content: `execute::copy_selected_content`, filtered to exactly the
   user-selected categories/worlds (`ImportSelection`) - not "copy everything" like the legacy
   importers. Reports progress via `InstallPhaseId::DownloadingContent`, which the job engine
   already throttles specifically for high-frequency per-file updates
   (`InstallProgressReporterState::should_persist`), so calling `.update()` once per file needed
   no extra throttling in this code.
3. Optional, opt-in **delete from source after import** (`execute::delete_selected_source_content`)
   - only ever runs after step 2 has already succeeded, and only ever removes the exact
     folders/worlds that were just copied (never anything else in the source instance).
4. `install_minecraft_with_reporter` - same as a plain `CreateInstance` job - so the instance is
   actually launchable afterward, not just a folder of copied files.

**Settings/Java-path import** (`execute::apply_settings`) is a separate, simple settings edit -
not an install job, no progress bar, applies immediately. Every field is independently optional:
a selection flag with no corresponding preview value (because that source column didn't exist -
see the Phase 0 compatibility allowlist) is skipped rather than clearing Dyad's existing value.
Known limitation, deliberately out of scope for this pass: an imported Java path points at the
*source* app's own managed Java install, not a copy - if that install is later removed, the path
would go stale (Dyad's existing Java re-detection is the fallback, same as any other stale path).

**A subtlety worth remembering:** `settings.custom_dir` from Phase 0/1 is about where the
*source's* `profiles/` folder lives, not about how Dyad organizes anything - Phase 2 never touches
Dyad's own directory layout. The `ImportSelection.categories` list can include `Saves` (as a
"the user wants some worlds" signal from the UI), but it is never copied as a whole folder -
`ImportSelection::roots()` deliberately filters `Saves` out of the plain per-category list and
expands `worlds` into individual `saves/<world>` roots instead, mirroring Phase 1's decision to
never treat `saves/` as one foldable unit.

**Testing:** per the user's preference, no end-to-end run against the real Modrinth App install or
real Dyad database from this session - real-world verification is being done separately on a VM.
Instead, `copy_selected_content`'s selection logic was pulled out into a pure, `State`-independent
`resolve_copy_manifest` function specifically so it could be unit-tested against real temp
directories: 4 new tests cover selecting only certain categories, selecting worlds independently
of whether `Saves` is in the category list, skipping a category folder that doesn't exist (a
regression test for the exact bug Phase 1's real-install testing found), and that deleting a
selection only removes what was selected and leaves everything else untouched. All 9
`migrate_modrinth_app` tests (5 from Phase 1, 4 new) and the crate's full 33-test suite pass;
`clippy --all-targets` is clean on both `theseus` and `theseus_gui`.

## Real-install validation, round 2: Linux + first real Phase 2 E2E run (2026-09-15)

Ran the same kind of real-install validation as above, but on a second machine (Linux, not
Windows) and, new this round, an actual end-to-end run of Phase 2's write path - not just Phase
1's read-only preview. Four findings:

1. **`is_source_running`'s process-name substring match can self-detect the test binary.** Linux
   truncates process `comm` names to 15 characters
   ([`TASK_COMM_LEN`](https://man7.org/linux/man-pages/man5/proc_pid_comm.5.html)). The original
   manual test binary, `manual_modrinth_app_preview`, truncates to exactly `manual_modrinth` -
   which itself contains "modrinth", so `is_source_running()` reported `true` (blocking the
   preview branch) even with the real app fully closed. Confirmed by temporarily dumping matching
   `sysinfo` processes: the only match was the test's own PID, `exe` pointing at
   `target/debug/deps/manual_modrinth_app_preview-...`. Not a production bug - the shipped app
   isn't named anything containing "modrinth" - but it made this exact test unable to validate its
   own "app is closed" branch. Fixed by renaming the test to
   `tests/manual_migrate_app_preview.rs` (truncated name no longer contains "modrinth"). Worth
   remembering if any *other* dev-only binary/script here ever ends up with "modrinth" in its first
   15 characters.
2. **A real install can be running an old, pre-refactor schema - confirmed live, not just
   theorized.** This machine's real official Modrinth App install (last updated some months prior)
   was still on migration `20260323185654`, well before the `instances`/`instance_content_sets`
   split - it had the old flat `profiles` table (with `override_*` columns inline) and still had
   `modrinth_users`. `build_preview` failed exactly as designed: a specific, named error
   (`"...missing the expected instances table...may be too old or too new a version to
   import from."`) rather than a crash or silently-wrong data. This is a real validation of the
   Phase 0 compatibility strategy, and a reminder that "too old a source version" isn't a lab
   hypothetical - real users sitting on months-old official-app installs is a plausible support
   case, not just "too new" schema drift. No import support was added for this older shape; the
   clean failure is the intended behavior for now. After updating the real install to current
   (`20260911120000`, matching upstream `main` exactly), the full preview succeeded: 1 instance
   (`CreulCat`, Fabric 26.2), correctly showing only the categories that actually had files (Mods:
   34, Config: 46, Logs: 4) and correctly omitting Resourcepacks/Shaderpacks/Saves/Screenshots,
   whose folders exist on disk but are empty - confirming empty-category omission isn't a bug.
3. **First real end-to-end Phase 2 run (not just the `resolve_copy_manifest` unit tests).** Added
   `tests/manual_migrate_app_import.rs` (same `#[ignore]`d pattern, a dedicated
   `DyadLauncherManualTest` app identifier so it never touches a real Dyad data directory) that
   calls `install::import_modrinth_app_instance` for real against the validated install above,
   selecting Mods + Config, and polls the job to completion. Result: succeeded in ~55s including a
   real ~600MB Minecraft 26.2 + Fabric 0.19.3 + JRE download, 34/34 mod files and 46/46 config
   files copied byte-for-byte identical to the source, the new instance registered correctly
   (name/loader/game version all correct), and the source instance directory confirmed untouched
   afterward (`delete_source_after_import: false`). This is the first time Phase 2's actual write
   path - not just its pure-function unit tests - has run against a real source install.
4. **Minor: empty directories inside a copied category aren't recreated.** Diffing source vs.
   copied `config/` folder-by-folder (after confirming all 46 files matched) found 9 empty
   directories present in the source (`worldedit/`, `craftingtweaks/grids/`,
   `viafabricplus/jars/`, etc. - leftover empty folders some mods create) that don't exist in the
   copy. Every actual file copied correctly; only zero-file directories were dropped, presumably
   because the copy walks/copies files rather than mirroring the directory tree itself. Low
   impact - mods that use these folders generally recreate them on first run - but worth a
   deliberate decision (recreate empty dirs too, or explicitly accept the gap) rather than leaving
   it as an unnoticed side effect the next time someone touches this code.

## Phase 3: frontend UI (2026-09-15)

Built the full import flow in `apps/app-frontend`: a dedicated `MigrateModrinthAppModal.vue`
(detect → "please close it" gate → per-instance/per-category/per-world selection → global
settings/Java-path selection → confirm, with live per-job progress and cancel/cancel-all wired to
the existing `install_job_cancel` command), a Settings-page entry, and a welcome-screen entry shown
when an install is auto-detected. Command param/return types are hand-written TS (matching the
pattern every other command wrapper in this codebase already uses - only backend→frontend *events*
are ts-rs-generated here, not command types), not blocked on Phase 4's bindings pass.

**Two real bugs found the moment this was actually exercised through the Tauri IPC layer for the
first time** (everything before this was tested by calling the Rust functions directly, never
through the GUI):

1. **Missing Tauri capability grants.** This app allowlists every plugin command through
   `apps/app/build.rs` (`InlinedPlugin::new().commands(&[...])`) plus a matching entry in
   `apps/app/capabilities/plugins.json`. The `migrate_modrinth_app` plugin was never added to
   either from the original Phase 1/2 work, and the new `install_import_modrinth_app_instance`
   command was never added to the existing `install` plugin's list. Every call from the frontend
   was being silently denied by Tauri's runtime ACL - detection and import both appeared to just
   do nothing.
2. **The plugin was registered under an invalid identifier.** `migrate_modrinth_app.rs` registered
   itself as `"migrate_modrinth_app"` (underscore) - Tauri plugin identifiers only allow lowercase
   ASCII and hyphens, the same reason `minecraft_skins.rs` registers as `"minecraft-skins"`. This
   was wrong from the moment Phase 1 landed but nothing had referenced it as a formal identifier
   until the capability fix above, which is what surfaced it (a JSON-parse panic in `build.rs`).
   Renamed to `"migrate-modrinth-app"` everywhere (Rust registration, `build.rs`, capabilities,
   frontend `invoke()` calls).

Also added `$DATA/ModrinthApp/caches/icons/*` to `tauri.conf.json`'s asset-protocol scope, since
the source app's icon cache was outside the webview's allowed scope for the preview's instance-icon
thumbnails.

**Last-played and playtime weren't being carried over - not a deliberate cut, just an incomplete
wire-up.** `last_played` was already read from the source and present in `ImportInstanceCandidate`,
but Phase 2's `prepare_initial_instance` never applied it to the newly created instance (which
`instance::create` always starts at `last_played: None`). Playtime
(`submitted_time_played`/`recent_time_played`) wasn't even in the Phase 0 read allowlist. Fixed:
both columns are now read (`source_db::fetch_instances`), collapsed into one
`total_time_played: u64` on the preview candidate (the source's own split between the two only
matters for its own reporting cadence, not to Dyad), and `InstallRequest::ImportModrinthApp` now
carries `last_played`/`total_time_played` through to a follow-up `instance::edit` call right after
creation.

## Symlink/junction handling (2026-09-16)

Prompted by a user question: what happens to a symlink or Windows junction inside a category
folder? Before this, nothing special - `get_all_subfiles`'s `is_dir()` check follows symlinks
transparently, so a linked directory was silently walked into and its real content fully copied,
on every platform. For a junction pointing at a large shared folder (or a world symlinked onto a
different drive), that meant an expensive, redundant full copy instead of a lightweight link.

**Scope decision:** only two shapes are detected - a whole category folder itself being a
symlink/junction (e.g. `mods/`), or a whole top-level `saves/<world>` folder being one. A symlink
nested deeper inside an otherwise-real folder tree, or one pointing at a single file, is left to be
copied as real content exactly like before; handling arbitrary nesting would mean rewriting the
whole-folder copy into a per-file model, for comparatively rare real-world cases.

**Per-symlink choice, not a blanket setting:** `ImportInstanceCandidate.symlinks` (new preview
field) lists each detected symlink with its category, resolved absolute target, and whether that
target lies outside the official Modrinth App's own managed directories
(`targetOutsideSourceApp`). The user picks one of three actions per symlink, always all three
offered:

- **Copy** - today's behavior, copy the real content.
- **Recreate** - relink instead of copying, but only genuinely happens when the target is outside
  the source app's tree; the *frontend* resolves this before sending the request (using the
  preview's `targetOutsideSourceApp`), translating "Recreate" into "Copy" for a target that's
  inside. This keeps `execute.rs` from ever needing to re-read the source database (its existing
  architectural rule - see the module doc comment), matching how the rest of Phase 2 already trusts
  preview-derived values (name, game version, loader, ...) without re-verifying them.
- **Ignore** - skip it entirely.

Default for every detected symlink: **Recreate**.

**A real reconciliation gap found while tracing a concrete scenario through the design before
implementing** (a symlinked world pointing at a different drive on Windows): worlds already have
their own plain "include this world" checkbox, entirely separate from the new per-symlink choice.
Without fixing this, a symlinked world's checkbox and its 3-way selector could contradict each
other. Fixed by having the 3-way selector *replace* the checkbox for any world (or whole category)
that's a detected symlink, rather than existing alongside it - "Ignore" is now how you exclude it.

**Recreation itself:**
- Linux/macOS: a plain Unix symlink (`std::os::unix::fs::symlink`).
- Windows: an actual NTFS **junction**, not a Windows symlink - a junction needs no elevation or
  Developer Mode (a real Windows symlink does), and is almost certainly what created the link being
  imported in the first place (`mklink /J`). `std` has no junction support at all, so this pulls in
  the small `junction` crate (Windows-only dependency) rather than hand-rolling the
  `DeviceIoControl`/`FSCTL_SET_REPARSE_POINT` call. Verified the crate's exact `create(target,
  junction)` signature against its docs before using it, since this branch can't be compile-checked
  on this (Linux) machine.
- If recreation fails for any reason (permissions, an exotic filesystem, the item turning out not
  to actually be a directory symlink by execution time), that one item falls back to a plain copy
  rather than failing the whole import.
- Deleting a symlinked source folder afterward (the existing "delete from source after import"
  option) was already safe with no changes needed: `remove_dir_all` on a path that's itself a
  symlink/junction only removes the link entry on both platforms, never the real target content -
  checked directly rather than assumed, since a symlinked-world scenario made it worth confirming.

New tests: `mod.rs` covers `path_is_within`'s component-wise (not naive string-prefix) matching and
`detect_symlink`'s handling of a real directory, a symlinked file, a broken link, and both
inside/outside targets (Unix-gated, using real symlinks in temp dirs - the Windows junction path
can only be compile-checked here). `execute.rs` covers a `Recreate` action actually creating a real
symlink and resolving to the right target, an `Ignore`d root being excluded from the copy manifest,
and a stale `Recreate` selection against a plain real directory falling back to a normal copy.

**Not yet done:**
- ts-rs/postcard bindings haven't been regenerated for the new types
  (`InstallRequest::ImportModrinthApp`, `ImportSelection`, `SettingsImportSelection`, etc.) -
  planned for Phase 4 alongside the rest of the bindings pass. Not a blocker for Phase 3, since
  command param/return types in this codebase are hand-written TS regardless.
- The `instance_launch_overrides` JSON field question flagged back in Phase 1's "still open" list
  remains unaddressed - imported instances get Dyad's default launch overrides, nothing from the
  source's per-instance overrides is carried over yet.
- Empty directories inside a copied category still aren't recreated (see finding 4 above) -
  unaddressed.
