# Dyad Launcher — Project Goals

Dyad Launcher is a fork of the [Modrinth Monorepo](https://github.com/modrinth/code), focused
specifically on the desktop app (`apps/app`, `apps/app-frontend`, and the `theseus` library in
`packages/app-lib`). The web frontend and backend (`apps/frontend`, `apps/labrinth`) are carried
along as part of the monorepo but are not a development focus here.

The name comes from *dyad* (Greek, "a pair") — a nod to the core feature of running two linked
instances of the same setup side by side.

**Primary platform**: Windows. Changes should stay cross-platform (macOS/Linux) where practical,
but Windows behavior is what gets prioritized and tested first.

## Goals

### 1. Concurrent multi-account launches — done

Allow the same instance to be launched more than once at a time, each under a different
Microsoft account, as long as the instance has opted into this via a per-instance setting.

**Implemented as of 2026-09-04:** a per-instance `allow_concurrent_launches` setting (off by
default) was added to `InstanceLaunchOverrides`
(`packages/app-lib/src/state/instances/model/launch.rs`), editable through the existing
`edit_instance` patch flow. When set, `launch_minecraft`
(`packages/app-lib/src/launcher/mod.rs`) skips its "instance already running" checks instead of
rejecting the launch. The setting has a toggle in the instance's General settings tab, with the
tradeoffs below spelled out in its description.

Account selection deliberately reuses the existing global "active account" switcher
(`AccountsCard.vue`) rather than adding a dedicated per-launch account picker: to run a second
copy, switch the active account, then launch again. Nothing tracks which account launched which
running process, so the app does not stop you from (accidentally) launching the same account
twice — this was a deliberate scope cut, not an oversight. Because the main Play/Stop button
hides Play once an instance is playing, the second launch is triggered from the library grid's
right-click menu ("Launch another copy") or the instance page's "More actions" overflow menu,
both of which only offer it when the instance has opted in.

- Concurrent launches point at the **same instance folder** (not per-account cloned folders).
- Known tradeoff, accepted deliberately: `logs/latest.log`, `usercache.json`, and crash reports
  can be overwritten or interleaved between the two running processes, and a world open in both
  processes at once can corrupt. This is a conscious choice, not an oversight.

### 2. Symlink-based resource sharing

Let instances share resources with each other by configuring symlinks, covering:

- Mods folder
- Resource packs / shader packs
- Config/settings (`config/`, `options.txt`)
- Worlds/saves — with the same caveat as above: only safe when the linked instances aren't
  running concurrently against the same world.

### 3. Debloating the desktop app — done

Remove:

- Telemetry/analytics calls
- Account/login promos & ads
- News/Discover/social panels

Explicitly **keep** Discord Rich Presence, but revisit/tweak its behavior (the result is described
in "Discord Rich Presence" below). Explicitly **keep** anonymous content browsing/searching/
downloading against Modrinth's API (no login required) — that's the actual point of the launcher,
not bloat.

**Discord Rich Presence — done as of 2026-09-20.** From 2026-09-07 (commit `01e887690`) until now
Rich Presence was a stopgap: force-disabled in `Settings::get()` and its settings tab hidden,
because the presence still showed Modrinth branding (the registered Discord Application's name and
icon). It is now a finished, opt-in feature:

- **Branding.** A Dyad-owned Discord Application (client ID in
  `packages/app-lib/src/state/discord.rs`), so Discord shows "Playing Dyad Launcher" with the Dyad
  mark. The image is the art asset `logo_square_1024`, a square export of `docs/branding/mark.svg`
  (the asset is uploaded in the Discord Developer Portal, not stored in the repo).
- **Off by default, opt-in.** Migration `20260920120000_discord-rpc-off-by-default.sql` resets the
  setting (the column was created `DEFAULT TRUE` by upstream). The toggle lives under Settings →
  Behavior → Discord (the old Privacy tab, which held only this toggle, was removed) and applies
  immediately, no restart. With it off the launcher never connects to Discord at all. The welcome
  screen also offers it: a toggle row that reflects the real setting, so it never re-asks (that
  screen shows again whenever there are no instances). Existing users, who never see the welcome
  screen, find it under Behavior.
- **What it shows.** One presence per launcher, computed by `DiscordGuard::refresh()` from the
  running instances:

  | Situation | Line 1 | Line 2 | Timer |
  |---|---|---|---|
  | 1 visible instance | `Playing: <instance name>` | `<game version> · <loader>` | since it launched |
  | 2+ visible instances | `Playing N instances at once` | — | since the oldest launched |
  | Nothing running | what the user is doing in the launcher (below) | — | — |

  It refreshes on launch, on exit, at startup, when the setting changes, and when an instance's
  visibility changes.
- **Per-instance opt-out.** Instance Settings → General → "Show in Discord Rich Presence" (default
  on), stored as `hide_from_discord` in the instance's launch overrides (JSON, so no migration).
  Hidden instances are left out entirely, including from the count. If every running instance is
  hidden, the presence is cleared rather than showing an idle line while playing. An instance that
  can't be looked up is treated as hidden (fails closed).
- **Page-aware idle text.** While nothing visible is running, the frontend reports a coarse
  category (never a project or instance name), debounced by one second because Discord rate-limits
  updates, via the `discord` Tauri plugin (`discord_set_launcher_activity`):

  | Page | Shown |
  |---|---|
  | Home (includes the Library) and `/instance/…` | Looking at instances |
  | Browse, project and user pages | Browsing mods |
  | Skin selector | Changing skins |
  | Global Screenshots page | Looking at screenshots |
  | Anything else | In the launcher |

Known tradeoffs and notes:

- The strings are hardcoded English (the previous ones were too).
- Two copies of the same instance (goal 1) count as "2 instances".
- The account being played is deliberately never shown.
- Rich Presence that mods add inside Minecraft is unaffected.
- The `discord-rich-presence` crate (1.0.0) sends activities without reading Discord's reply and
  doesn't check the handshake result, so a rejected activity fails silently. If the presence ever
  doesn't appear, check Discord itself first: Settings → Activity Privacy must allow sharing
  activity (this was the cause the first time it "didn't work" during development).

**Part 1 — done (telemetry, ads, promos, news/friends UI):**

- Removed PostHog analytics (`helpers/analytics.ts` and all `trackEvent(...)` call sites) and Sentry
  crash/performance reporting (`main.js`), including their dependencies and CSP entries. Removed the
  Rust-side `analytics/playtime` and `analytics/minecraft-server-play` POSTs in
  `packages/app-lib/src/api/instance/run.rs` (local playtime bookkeeping is preserved, it's just no
  longer reported to Modrinth). Removed the `telemetry`/`personalized_ads` settings columns via a new
  migration and their Privacy-settings UI.
- Removed the entire native ads system: `apps/app/src/api/ads.rs`, the macOS/Windows occlusion-tracking
  files, the IAB consent-management-platform scripts (`ads-consent/`), the `ads` Tauri
  plugin/capability, and the frontend ad-slot plumbing (`helpers/ads.js`, `PromotionWrapper.vue`).
- Removed promotional/upsell surfaces: the "Upgrade to Modrinth+" links (nav sidebar and account menu),
  the Pride fundraiser banner, the Tally survey popups, and the Modrinth Servers Intercom support-chat
  widget — pruning their CSP entries too.
- Removed the News feed panel and the Friends/presence panel + "Add a friend" menu entry from the
  sidebar. The underlying Friends source files (`components/ui/friends/`, `composables/use-friends.ts`,
  `helpers/friends.ts`) are intentionally still present — they're still used internally by the
  Modrinth-account-based "shared instances" cloud feature, and will be deleted together with that
  feature in Part 2 below.

**Part 2 — done (Modrinth account removal):** based on user direction during scoping, this goal
turned out to require removing the Modrinth account system entirely (sign-in/OAuth, not the
separate Microsoft/Minecraft account needed to launch the game), since keeping it would still
suggest a live connection between Dyad and Modrinth.

**Implemented as of 2026-09-05:** removed entirely —

- Sign-in/OAuth: `mr_auth` (frontend and `packages/app-lib`), the hydra/oauth flow
  (`api/oauth_utils/`), and `ModrinthAccountRequiredModal.vue`.
- The cloud "shared instances" feature — a different feature from goal 2's symlink sharing, since it
  only worked via Modrinth accounts: invites, publish/unlink, sync, and install/update flows
  (`pages/instance/share/`, `components/settings-modal/sharing-settings.vue`,
  `packages/app-lib/src/api/instance/shared/`, `install/shared_instance.rs`), along with the
  friends/presence system it depended on (`use-friends.ts`, `components/ui/friends/`,
  `state/friends.rs`, `api/friends.rs`).
- Modrinth Servers hosting/billing: the Stripe purchase flow, server management pages, and the
  hosting content-install wizard (`HostingUpdateRequired.vue` and related).
- Other account-gated settings/UI (`ProfileSettings.vue`, `SocialSettings.vue`), the now-dead
  onboarding checklist step tied to these features, and the `modrinth_users` session table (dropped
  via migration `20260905120000_remove-modrinth-account.sql`).
- ts-rs/postcard TS bindings were regenerated to match the trimmed Rust event/command enums.

Foundational data-model fields that would cascade into large parts of core instance management
(shared_instance attachment, content-set sync provider, a couple of client base-url configs) were
kept in place but are now permanently unpopulated, since nothing can construct them anymore —
removing them outright was judged not worth the churn.

Anonymous content browsing, search, and downloads against Modrinth's API were explicitly **not**
affected by Part 2 — they don't require an account and kept working exactly as before.

### 4. Update notifications — done

Tell users when a new Dyad release exists, without depending on Modrinth's update infrastructure.
This goal started out as "an auto-update mechanism"; the in-place self-updater was built and then
replaced by a simple version check on 2026-09-21 (see "Why the self-updater was dropped" below).

**Current behavior (as of 2026-09-21, `9500ef50d`):** Dyad does **not** update itself. It checks the
release manifest and points the user at the download.

- `apps/app-frontend/src/providers/app-update.ts` fetches
  `https://github.com/creulcat/DyadLauncher/releases/latest/download/updates.json` (via
  `@tauri-apps/plugin-http`) once at startup and then every hour, and compares the manifest's
  `version` with the running version. GitHub always resolves `latest` to the newest release, so
  publishing a release *is* what makes it visible; there is no separate publish step.
- When a newer version exists, a banner appears at the bottom of the sidebar (`App.vue`) and the
  Settings modal's footer, under the version number, shows a "Download" button next to a manual
  "Check for updates" button. Both open the platform's installer link (the manifest's `install_urls`
  entry) in the system browser, falling back to the GitHub releases page, and to copying the link to
  the clipboard if the browser can't be opened. The user runs the installer themselves.
- The check is **unconditional**: there is no toggle, and it runs at every launch and hourly
  after. Whether to add an off switch is an open item in goal 7 (item 5).
- No Tauri updater plugin is compiled in or configured any more (no `updater` Cargo feature or
  capability, no `plugins.updater` block or public key). The app never downloads or runs anything
  itself, so there is nothing for it to verify: the only network request is a read of the manifest
  over HTTPS, and installing is the user's own action.

**Why the self-updater was dropped:** the opt-in Tauri self-updater built in phase 2 (2026-09-14, see
"History" below) was broken in production. The release workflow only ever uploaded the installers and
`updates.json` to the GitHub release, never the signed updater bundles (`.nsis.zip`,
`.AppImage.tar.gz`, `.app.tar.gz` and their `.sig` files), so the manifest's `url` fields pointed at
files that didn't exist. An attempt to upload them (`07079e5d1`) was reverted (`4adaad574`) the same
day in favor of dropping the self-updater altogether.

**Release pipeline (still current):**

- Both build workflows were originally written for `namespace-profile-*` runners — Modrinth's own
  paid Namespace Cloud pool, not available to this fork — so neither could run here at all. They were
  rewritten onto standard GitHub-hosted runners (`macos-latest`/`windows-latest`/`ubuntu-22.04`),
  dropping the Namespace-specific caching steps. They also had a dormant bug from the product rename
  (hardcoded "Modrinth App" bundle filename patterns that never matched the "Dyad Launcher"-named
  output), fixed at the same time.
- Releases are published as GitHub Release assets directly instead of to Modrinth's S3 bucket (no
  secrets for it in this fork). GitHub replaces whitespace in uploaded asset filenames with `.`, so
  every bundle is staged into a flat, space-free-named `release-assets/` directory first and the
  uploaded name matches the name in the manifest exactly.
- The release job lives in `theseus-build.yml` (commit `2fcb325fa`), not in a separate workflow:
  `workflow_run` doesn't reliably fire for workflows triggered by a tag push. It is gated on the build
  job succeeding for a tag ref, and bundle staging locates each file by name under the downloaded
  artifact directory rather than a hardcoded nested path (`d68398da5`).
- macOS code signing is skipped on forks without Apple secrets (`b4ed482bf`), the same fallback
  pattern the Windows signing step already uses.
- Twelve other workflows scoped to the web frontend, the `labrinth` backend and Modrinth-account
  infrastructure (Crowdin sync, `labrinth` Docker builds, the `daedalus` metadata service, the ArgoCD
  `/deploy` command, etc.) and the stopgap `manual-build.yml` were deleted as out of scope for a
  desktop-app-only fork.
- Every pushed tag is published as a full release and becomes "latest" for the manifest. There is no
  beta/prerelease channel distinction, by deliberate choice.
- `updates.json` is generated in the workflow with the release `version`, `notes`, `pub_date` and a
  `platforms` map. The app only reads `version`, `notes` and each platform's `install_urls`.

**Leftovers from the self-updater, not yet cleaned up:**

- `bundle.createUpdaterArtifacts: "v1Compatible"` in `apps/app/tauri-release.conf.json` still makes
  release builds produce the updater bundles and `.sig` files, and the `TAURI_PRIVATE_KEY` /
  `TAURI_KEY_PASSWORD` repo secrets still sign them. Nothing consumes them: the signatures are only
  inlined into `updates.json` as `signature`/`url` fields the app ignores. Dropping
  `createUpdaterArtifacts` would also make those two secrets, and the rotated keypair, unnecessary.
- A stale comment in `theseus-build.yml` still describes the manifest as "the updater feed configured
  in tauri-release.conf.json".
- The `check_for_updates` setting (migration `20260914120000_add-check-for-updates-setting.sql`, the
  Rust field in `state/settings.rs`, the type in `helpers/settings.ts`) has no UI and nothing reads
  it. Its toggle in Behavior settings was removed in `9500ef50d`.
- `@tauri-apps/plugin-updater` is still listed in `apps/app-frontend/package.json`.
- If a real in-place updater is wanted again later, it means building that back properly, including
  uploading the signed bundles the manifest points to.

**History:**

- **Phase 1 — done 2026-09-04:** disabled Modrinth's updater. `tauri-release.conf.json` stopped
  enabling the `updater` Cargo feature/capability and the `plugins.updater` block, so release builds
  never compile in or contact Modrinth's update endpoint
  (`https://launcher-files.modrinth.com/updates.json`) or its signing key. A second, independent check
  was removed too: `App.vue` had a `checkLinuxUpdates()` fallback that fetched Modrinth's
  `updates.json` directly whenever the updater feature was off, i.e. in exactly the state this phase
  put every platform in. It was deleted along with its now-unused i18n message.
- **Phase 2 — done 2026-09-14, superseded 2026-09-21:** a fork-owned, opt-in Tauri self-updater on
  GitHub Releases, with its own Ed25519 update-signing keypair (unrelated to goal 5's Authenticode
  signing, which is about binary trust, not update-payload integrity) and an off-by-default
  `check_for_updates` setting. It also fixed the hardcoded "Modrinth App" wording in the
  update-download popup. During the first tagged releases (2026-09-16/17) the update-signing keypair
  had to be rotated, because the `TAURI_PRIVATE_KEY`/`TAURI_KEY_PASSWORD` secrets from the initial
  setup didn't match each other, which went unnoticed until `createUpdaterArtifacts` was turned on and
  the build hard-failed. That was safe to do because no release had shipped with working updater
  artifacts yet.

Known tradeoffs:

- No in-place update. Users download and run the installer for every release, and because the Windows
  installer is unsigned (goal 5), each update download meets the same browser and SmartScreen
  warnings.
- The always-on check is a request to `github.com` at every launch and hourly, with no way to
  switch it off (goal 7, item 5).

### 5. Windows installer trust warning (SmartScreen) — partly done, signing itself still open

**Problem:** the NSIS installer this fork's CI produces for Windows is unsigned, so both the
browser download warning and Windows SmartScreen's "Windows protected your PC" prompt trigger on
run, with no verified publisher shown. This isn't fixable via NSIS/installer settings alone — it's
a missing Authenticode signature.

**Root cause:** `apps/app/tauri-release.conf.json`'s `bundle.windows.signCommand` calls `jsign`
against **Modrinth's own** DigiCert ONE cloud signer, credentialed by
`DIGICERT_ONE_SIGNER_*`/`TAURI_SIGNING_PRIVATE_KEY*` secrets that only exist in Modrinth's
upstream repo. `.github/workflows/theseus-build.yml`'s "Set up Windows code signing" step (around
line 159) already detects when signing isn't requested and strips `signCommand` before building,
so normal builds are just unsigned — this fork has never had its own signing credentials to put in
its place.

**Decided approach:** apply for **SignPath.io's free code-signing program for open-source
projects** — it should get EV-equivalent trust (immediate SmartScreen clearance, no
reputation-building wait) at no cost, which fits a public open-source fork. If that application is
rejected or stalls indefinitely, fall back to evaluating a paid OV or EV certificate, or
documenting the warning as an accepted tradeoff — but SignPath is the path to actually pursue
first, not just one option among several.

**Separate fix, not blocked on the above — done as of 2026-09-13 (`efcbb80df`):**
`.github/workflows/theseus-build.yml`'s Windows signing step used to take the "sign" branch on any
`refs/tags/v*` push and would have failed outright against this fork's empty DigiCert secrets. It
now also checks that the DigiCert secret is actually present, so a tag/release build degrades to an
unsigned build (like normal branch builds) instead of hard-failing. When SignPath (or another
signing path) is set up, this condition will need to be revisited to check for whichever
credentials it uses.

**Independent cheap improvement — done as of 2026-09-13 (`efcbb80df`):** `apps/app/tauri.conf.json`'s
`copyright`, `publisher`, `shortDescription`, and `longDescription` fields, previously empty, are
now filled in. This doesn't remove the SmartScreen prompt by itself, but the warning dialog now
identifies a real app instead of a blank one — and it's metadata a certificate would need attached
anyway.

**Still open:** the installer itself is still unsigned, so the SmartScreen and browser download
warnings still appear. No implementation timeline yet — next step is submitting the SignPath
application.

### 6. Migrate-from-Modrinth-App import tool

Let a user who's already using the **official Modrinth App** bring their existing instances (and
selected settings) into Dyad, with real per-item control over what comes over — not an
all-or-nothing copy.

**Not the same thing as** `packages/app-lib/src/state/legacy_converter.rs` — that's a different,
unrelated, fully-automatic one-time migration from a defunct pre-rewrite "Theseus" app format
(`com.modrinth.theseus`), with no user choice involved. It's referenced here only because it shows
the general shape of "read another app's on-disk state and convert it," not because it's reusable
for this goal.

**Why this isn't a plain file copy:** instances aren't just folders. Data directories are keyed by
the Tauri app identifier — the official Modrinth App uses `%APPDATA%\ModrinthApp\` on Windows,
Dyad uses `%APPDATA%\DyadLauncher\` (`packages/app-lib/src/state/dirs.rs:38`; confirmed via the
`identifier` field's history in `apps/app/tauri.conf.json`), with the equivalent `dirs::data_dir()`
convention on macOS/Linux. Each instance also has a row in that app's **SQLite database** (loader,
Minecraft version, Java args, icon, etc.), not just a folder under `profiles/<name>/` on disk. So
import has to read the source app's DB and register each instance through Dyad's own
instance-creation code path — copying the instance folder alone would leave Dyad with an orphaned
folder it doesn't recognize.

**Decided approach:**

- Auto-detect the official Modrinth App's data directory (`ModrinthApp` under the platform's data
  dir), with a manual folder picker as a fallback for nonstandard locations or if detection fails.
- Require the official Modrinth App to be fully closed before importing — detect a locked database
  file and show a clear "close Modrinth App first" message, rather than trying to support
  concurrent access to the same SQLite file.
- **Copy by default**, never modifying or deleting anything in the source install; offer an opt-in
  "delete from source after import" per instance for users who want to reclaim disk space
  immediately and are confident in the result.
- **Per-instance selection**, and within each selected instance, **per-content-category
  selection** (mods, resource packs, shader packs, config, worlds/saves, screenshots, logs) —
  reusing the same selection-tree UI pattern as the existing mrpack-export feature
  (`packages/app-lib/src/api/instance/export_mrpack.rs`) rather than inventing a new one.
- Also offer importing **global launcher settings**, separate from per-instance content: Java
  installation paths, default memory/JVM args, and pre-launch/post-exit hooks. (Nothing to import
  for accounts/telemetry — those don't exist in Dyad per goal 3.)
- **Entry point, both places:** offered automatically during first-run onboarding when Dyad
  detects an existing Modrinth App install, and separately available anytime as a manual action in
  Settings (e.g. an "Import from Modrinth App" page) for re-runs or if declined on first run.
- A **preview step** before committing: show exactly which instances and settings will be
  imported, and roughly how much disk space it'll use, before anything is written.

Not started, no implementation timeline yet. Needs a technical design pass on reading the source
app's SQLite schema safely — it may have drifted from Dyad's fork point over time — before
implementation starts.

**Phase 0 (schema discovery/strategy) — done as of 2026-09-13:** see
[goal-6-import-design.md](goal-6-import-design.md) for the full write-up. Headline finding: the
tables this feature actually needs (`instances`, `instance_content_sets`,
`instance_launch_overrides`, `instance_icon_configs`, `java_versions`) are currently identical
between Dyad and current upstream `modrinth/code`; `settings` differs only by columns outside our
import allowlist. Reader strategy decided: named-column `SELECT`s only (never `SELECT *` /
bulk-copy), `PRAGMA table_info` presence checks with named errors on an unrecognized schema, and
`minecraft_users` (account credentials) explicitly never read. Planned as incremental PRs: Phase 1
(read-only detection/preview, no writes) is next.

**Phase 1 (read-only detection/preview) — done as of 2026-09-13:** implemented in
`packages/app-lib/src/api/migrate_modrinth_app/{mod,source_db}.rs`. Validated against a real
official Modrinth App install (not just fixtures), which surfaced and fixed two real bugs (wrong
instance-folder resolution when `custom_dir` is set; `saves/` sizing that took 80+ seconds against
a real 330k-file world, now a per-world listing with deferred sizing instead) and replaced a
fundamentally broken lock-detection approach (a `BEGIN IMMEDIATE` DB probe, which SQLite's WAL
mode made a false negative even with the app confirmed running) with a process-list check. Full
writeup in [goal-6-import-design.md](goal-6-import-design.md).

**Phase 2 (actual instance creation and content copying) — done as of 2026-09-13:** implemented as
a new `InstallRequest::ImportModrinthApp` variant in Dyad's existing install-job engine
(`packages/app-lib/src/install/`), reusing its progress reporting, crash recovery, and
rollback-on-failure rather than building bespoke versions. Copies only the user-selected
categories/worlds (not "copy everything" like the legacy multi-launcher importer), with an opt-in
per-instance "delete from source after import" that only ever removes what was actually copied.
Settings/Java-path import is a separate, simple settings edit.

**Phase 3 (frontend UI) — done as of 2026-09-15:** a dedicated import modal
(detect/preview/per-instance/per-category/per-world selection, live per-job progress with
cancel/cancel-all), a Settings-page entry, and a welcome-screen entry. Fixed two real bugs only
caught once this actually ran through the Tauri IPC layer for the first time (missing capability
grants for the `migrate_modrinth_app` plugin and the new install command; the plugin's identifier
was also invalid, since Tauri disallows underscores). Also fixed `last_played`/playtime not being
carried over to imported instances - previously read but never applied (`last_played`), or not read
at all (playtime). See [goal-6-import-design.md](goal-6-import-design.md) for the full writeup.

Phase 4 (ts-rs/postcard bindings, further tests, docs) turned out to already be complete on
inspection as of 2026-09-16 - this codebase's binding generation only ever covers backend→frontend
event types, never command types, so none of goal 6's types were ever going to need it; the one
bound type goal 6 does touch (`InstallJobKind`) already picked up its new variant automatically the
first time the app ran.

**Follow-ups — done as of 2026-09-16:** symlink/junction handling (a whole symlinked category or
world folder is detected and offered as copy/recreate/ignore per item, defaulting to recreate;
Windows recreates a real NTFS junction rather than a Windows symlink, since junctions need no
elevation); empty directories inside a copied category are now recreated instead of silently
dropped; and per-instance launch overrides (JVM args, memory, hooks, a specific Java path) are now
read from the source and offered as a per-instance opt-in, applied the same way `last_played`/
playtime already were. No known gaps remain for goal 6 beyond real-world Windows validation - the
junction-creation path and the whole Phase 3 UI have so far only run on Linux.

**Real-install bug fixes — done as of 2026-09-16 (`00366d529`):** testing against a real install
turned up three more bugs, now fixed:

- Instances with no applied content set (e.g. an orphaned/null `applied_content_set_id`) were
  silently unselectable and skipped by "import all". They now fall back to the instance's most
  recently modified content set as a best-effort guess, flagged in the preview and left opt-in
  rather than auto-included.
- Only the seven curated content-category folders were copied, dropping loose per-instance data at
  the instance root (`servers.dat`, `usercache.json`, `hotbar.nbt`, `waypoints/`, `backups/`, ...).
  The copy now also sweeps everything else at the root except regenerable loader/asset caches
  (`.fabric`, `.cache`). The app-level synced-options store (shared server list/hotbars/command
  history) can also be migrated now, seeding Dyad's own store when it's still empty.
- Re-importing an already-imported source instance silently created a second, disconnected
  instance. A new `modrinth_app_import_sources` table tracks prior imports so the UI can warn and
  offer skip/copy/overwrite; a deliberate "copy" gets an "(import copy)" name.

The same commit also moved the import modal out of `WelcomeScreen` into `App.vue`'s persistent
root — it was being torn down mid-import the moment the first imported instance flipped
`hasCreatedInstance` and unmounted its `v-if` parent.

### 7. Network & tracking audit (debloating follow-up)

Goal 3 removed the telemetry, ads and account systems it knew about, but a code audit on 2026-09-21
found that a fair amount of Modrinth-, Stripe- and third-party-bound traffic and dead plumbing is
still left. This goal finishes the job and then makes it stay finished.

**Audit findings (static, from reading the code — not yet confirmed by capturing real traffic):**

*Fires at every launch, without any user action:*

- `App.vue` prefetches the Modrinth Servers list (`archon.modrinth.com`) and billing
  subscriptions/payments (labrinth `billing_internal`) as soon as the app state is ready. They carry
  no token, so they 401, but the requests still go out. Part 2 of goal 3 was supposed to have removed
  Modrinth Servers and billing.
- `providers/setup/user-country.ts` does a GeoIP country lookup against labrinth.
- `AccountsCard.vue` fetches avatar heads from `mc-heads.net`, a **third party** (not Modrinth),
  keyed by the Minecraft profile UUID or skin texture key. This is the worst of the list from a
  privacy point of view.
- The Minecraft fonts (`global.scss`) load from `cdn-raw.modrinth.com`, and the fallback images in
  `AccountsCard.vue`/`LegacyProjectCard.vue` load from `launcher-files.modrinth.com`.
- `index.html` still loads `https://tally.so/widgets/embed.js`. The survey popups were removed but
  the loader was not. The CSP `script-src` probably blocks it, but it is dead either way.
- The `@stripe/stripe-js` loader and `js.stripe.com` are present in the built frontend bundle (pulled
  in through `packages/ui`, which the app doesn't use for billing), and the CSP explicitly allows
  `js.stripe.com`. Nothing in the app mounts a billing UI, so this should be dead, but
  `@stripe/stripe-js` may inject Stripe.js as an import-time side effect. To be confirmed by the
  traffic capture below.
- The version check against GitHub (goal 4: `providers/app-update.ts`, started from `App.vue`) runs
  at every launch and hourly, with **no setting to turn it off** — the opt-in `check_for_updates`
  toggle went away when the self-updater was replaced in `9500ef50d`. Whether to add an off switch
  needs a decision, see below.

*Sent with every download / API request:*

- The `modrinth-download-meta` header (`packages/app-lib/src/util/fetch.rs`) attaches the download
  reason, game version, loader and the dependent project ID to each mod/pack/file download. It is
  Modrinth's download-attribution analytics, and Part 1 of goal 3 missed it.
- The User-Agent is `modrinth/theseus/<version> (<os>; support@modrinth.com)` in both Rust
  (`launcher_user_agent()` in `lib.rs`) and the frontend (`App.vue`), so every request to Modrinth
  still identifies the app as the official Modrinth App.

*Dormant code and config (no live request, but still there):*

- Account-dependent Rust: `api/users.rs` and `api/reports.rs` in `packages/app-lib`, plus their Tauri
  commands in `apps/app/src/api/`. Frontend: `helpers/user-preferences.ts`, `helpers/user-campaigns.ts`,
  the `Skins.vue` campaigns query (gated on a session token that can no longer exist), and
  `providers/setup/server-install-content.ts` (the Modrinth Servers "install to server" flow, wired
  into `Browse.vue` and `project/Index.vue`).
- CSP entries in `apps/app/tauri.conf.json` for Stripe (`js.stripe.com`, `*.stripe.com`,
  `wss://*.stripe.com`, `hooks.stripe.com`), Modrinth's Tailscale dev nodes (`*.ts.net`),
  `*.nodes.modrinth.com` (http and wss), and the stale `$DATA/ModrinthApp/caches/icons/*` asset
  scope. The `frame-src` entries for YouTube and Discord are used by rendered project descriptions
  and are kept.
- Config: `stripePublishableKey` (a hardcoded test-mode key) plus the archon and shared-instances
  base URLs in `apps/app-frontend/src/config.ts`; the same URLs and `MODRINTH_SOCKET_URL` in the
  `packages/app-lib/.env*` files.
- Dependencies: `@tauri-apps/plugin-updater` in `apps/app-frontend/package.json` (the Rust side is
  already gone), and the `sync_theme_across_devices` / `sync_behavior_across_devices` settings columns
  left over from the account removal. The workspace-level `async-stripe` and `sentry` crates and the
  `strip = false # Keep debug symbols for Sentry` profile setting are not used by the desktop crates
  and should be checked and dropped if so; `@stripe/stripe-js` is a dependency of `packages/ui`, which
  is shared with the website, so it stays in that package's `package.json` and only needs to stop
  reaching the app bundle.

**Deliberately kept (not bloat, documented as allowed hosts):**

- Modrinth's content API and CDN (browsing/searching/downloading, per goal 3).
- **`launcher-meta.modrinth.com`** — Modrinth's mirror of the Minecraft and loader version
  manifests, which the launcher needs in order to create and launch instances. Replacing it would
  mean talking to Mojang, Fabric, Forge, Quilt and NeoForge directly, which is a large separate
  project. Decided 2026-09-21: keep it and document it; it is not tracking.
- Mojang/Microsoft/Xbox authentication and skins, Minecraft textures, and Azul Java downloads.
- Discord Rich Presence (opt-in, goal 3), `mclo.gs` log sharing (only when the user clicks it), and
  the PaperMC/Purpur jar downloads.
- `img-src https:` in the CSP stays, because project descriptions render arbitrary remote images.
  That is an accepted tradeoff of anonymous browsing: an image host can see the request.

**Decided approach:**

1. **Remove** everything under "dormant" and "fires at every launch" that has no live purpose: the
   Servers/billing prefetch, the GeoIP lookup, the tally.so script, the account-only Rust and
   frontend code above, the Stripe/Tailscale/nodes CSP entries, the stale asset scope, the dead config
   and env values, the unused dependencies and the leftover settings columns (via a migration).
   Removing the Servers install flow touches `Browse.vue` and `project/Index.vue`, which are also the
   main content-browsing path, so that part is done carefully and checked against the browse and
   project pages.
2. **Bundle what can be bundled.** Ship the Minecraft fonts and the fallback images inside the app
   instead of fetching them from Modrinth's CDN, and render account heads locally from the skin
   texture the launcher already has (there is already a `helpers/storage/head-storage.ts`) rather
   than sending the UUID to `mc-heads.net`. Only the account avatar needs the skin data; if a head
   can't be rendered offline, show a default instead of making a third-party request.
3. **Drop the `modrinth-download-meta` header** and its `DownloadMeta` plumbing.
4. **Rebrand the User-Agent** to something like `DyadLauncher/<version> (github.com/creulcat/DyadLauncher)`
   in both Rust and TypeScript.
5. **Version check opt-out (open, needs a decision):** goal 4 now documents the check as it is —
   always on, no toggle. Either accept that (it is one read of a public file on github.com, no
   identifiers sent beyond the User-Agent from item 4) or add an off switch in Settings, wired to the
   existing but currently unused `check_for_updates` setting, so the launcher can be made completely
   silent toward the network. If a switch is added, goal 4 needs a matching update.
6. **Verify with real traffic, not just code reading.** Run a fresh profile through a proxy
   (mitmproxy or Fiddler) for a full session: startup, browsing, installing a mod, launching, signing
   in. From that, write `docs/NETWORK.md`: one row per host, with the reason it is contacted and
   whether it is opt-in.
7. **Add a guard so it stays clean.** A CI check (script in `scripts/`) that fails when a hostname
   appears in the app's source, config or CSP that is not in the `docs/NETWORK.md` allowlist, and
   the CSP is tightened to exactly those hosts (except `img-src`, above).

Known tradeoffs and notes:

- The traffic capture may find things the static audit didn't (or show that some of the above never
  fires); the list above is the starting point, not the final word.
- The allowlist guard is a text/hostname check. It catches new hard-coded hosts, not URLs built at
  runtime from API responses.
- Cosmetic "Modrinth App" strings (`index.html` `<title>`, `support.modrinth.com` links in the error
  modals, the WebView2 error dialog in `main.rs`) are a separate rebrand pass and are not part of
  this goal, except where they are a live request.

Not started. Scoped 2026-09-21.

### 8. Launcher backgrounds, with per-instance overrides

Let the user set a background for the launcher window, and let individual instances override it.

**Feasibility:** medium effort. Storing and showing an image is easy; keeping the UI readable over
one is the real work.

- The app grid paints `--color-raised-bg` behind everything, and pages use opaque `bg-bg` /
  `bg-bg-raised` surfaces, so an image would only show in the nav and status bars unless panels get
  a translucent mode. Those tokens live in `packages/ui`, which the website shares, so the
  translucency is scoped to a `.has-background` class set by the app and the website is untouched.
- `.app-contents { filter: brightness(1.1) }` in `App.vue` creates a backdrop root, which stops
  `backdrop-filter` blur from working on descendants. It has to be removed or moved when a background
  is active.
- Blur over large surfaces can be slow in WebView2, so the existing **Advanced rendering** setting
  (`advancedRendering`) is respected: with it off, blur is disabled and the background falls back to
  a plain dim.

**Decided approach (2026-09-21):**

- **Sources:** the user's **own image files** (png, jpg or webp) and built-in **solid colours and
  gradients**. Explicitly *not* in scope: a bundled art pack (it would need license-safe art with no
  Mojang assets and would grow the installer), an online background gallery (it would break goal 7's
  "no unnecessary network calls"), and animated or video backgrounds.
- **Storage:** a chosen image is copied into the app data dir under `caches/backgrounds/` (the same
  pattern as instance icons) and downscaled to a sensible maximum (about 2560px on the long side) to
  bound disk and memory use; that path is added to the Tauri asset-protocol scope in
  `apps/app/tauri.conf.json`. The original file is never referenced in place, so moving or deleting
  it doesn't break the launcher.
- **Global background:** picked in Settings (a new "Appearance"/background section next to the
  existing theme settings). It is stored in the `settings` table, via a migration.
- **Per-instance override:** a per-instance setting with three states — *inherit* (use the global
  background, the default), *none*, or *custom* (its own image or colour/gradient). It is stored with
  the instance's launch overrides JSON, like `hide_from_discord` (goal 3), so it needs no migration,
  and is edited in that instance's settings and applied through the existing `edit_instance` patch
  flow. The override applies to that instance's own pages (`/instance/…`); everywhere else in the app
  shows the global background. It does **not** change what the game shows.
- **Readability controls:** translucent panels with **blur**, plus **dim** and **blur** sliders, so
  the user can trade image visibility for legibility. Panel opacity follows the dim setting. Both
  light and dark themes are supported.
- **Cleanup:** deleting an instance deletes its background file; changing a background deletes the
  old file. An image that can't be read any more falls back to no background rather than an error.

Known tradeoffs and notes:

- Legibility over busy images is the main risk. The dim slider and the translucent panels are the
  mitigation; there is no automatic contrast detection in v1.
- Image dimension/size limits and the exact downscale target are implementation details to settle
  when building it.
- Fully local: this feature makes no network calls.

Not started. Scoped 2026-09-21.

### 9. Instance comparison

Let the user select **two or more** instances and get a report of how they differ.

**Feasibility:** medium effort. Most of the data is already in the launcher's database: `InstanceFile`
has each content file's `sha1`, `size` and `enabled` state, and `ContentEntry` has `project_id` and
`version_id`. So the comparison can be built as a pure function in `packages/app-lib`,
`compare(instance_ids) -> ComparisonReport`, testable without any UI.

**Decided scope for v1 (2026-09-21): metadata and content only.** Config-file comparison, world
comparison and any sync/copy actions are deliberately left out for now.

- **Metadata compared:** Minecraft version, loader and loader version, update channel, the
  per-instance launch overrides (Java path, memory, JVM args, environment variables, resolution and
  fullscreen, hooks), and the linked modpack and its version, if any.
- **Content compared:** mods, resource packs, shader packs and data packs. Entries are matched by
  `project_id` when the launcher knows it, which lets the report say "same project, different
  version" and "enabled in one, disabled in the other". Files with no known project (local or
  unknown files) fall back to matching by file name, then `sha1`. Each row ends up as one of: in all
  instances and identical, only in some, version differs, or enabled state differs.
- **Offline by design:** version numbers come from the launcher's cache only. If a version isn't
  cached, the report shows the file name instead of making a network request. Comparing never causes
  network traffic (goal 7).
- **Entry point:** multi-select in the library grid with a "Compare" action; a comparison page shows
  a matrix with one row per item and one column per instance, an "only show differences" filter,
  grouping by content type, and summary counts. Two instances get a clear side-by-side reading;
  three or more use the same matrix.
- **Export:** copy or save the report as **Markdown** or **JSON**.
- **Symlinks (goal 2):** a folder that is a symlink or Windows junction to another compared
  instance's folder is reported as *shared*, not as "identical", so the report can't mistake a
  shared mods folder for two coincidentally equal ones. Goal 6's symlink/junction handling
  (`migrate_modrinth_app`) is the reference for detecting them on Windows.

Known tradeoffs and notes:

- Anything not tracked as content (loose files in `config/`, `options.txt`, worlds, `saves/`) is not
  in the report in v1. That is a deliberate scope cut, not an oversight; config comparison
  (file-level hashing on demand with a size cap, then text diffs of `options.txt` and small config
  files) is the most likely next step.
- No actions in v1: the report is read-only. "Copy this mod to the other instance" and similar sync
  actions overlap with goal 2 and are left for later.
- The report reflects what the launcher's database last recorded for each instance. Whether to force
  a re-scan of each instance's files before comparing (so a mod dropped into a folder by hand is
  seen) is an implementation detail to settle when building it.

Not started. Scoped 2026-09-21.

## Status

Last reviewed 2026-09-21.

| # | Goal | Status |
|---|---|---|
| 1 | Concurrent multi-account launches | Done (2026-09-04) |
| 2 | Symlink-based resource sharing | **Not started** |
| 3 | Debloating | Done — removals 2026-09-04/05, Discord Rich Presence rework 2026-09-20 |
| 4 | Update notifications | Done — Modrinth's updater disabled 2026-09-04; self-updater built 2026-09-14, replaced by a download-link version check 2026-09-21; release-pipeline fixes 2026-09-16/17. Self-updater leftovers not yet cleaned up |
| 5 | Windows installer trust warning | **Partly done** — CI fallback fix and installer metadata landed 2026-09-13; installer is still unsigned, SignPath application not yet submitted |
| 6 | Migrate-from-Modrinth-App import | Done (2026-09-16), pending real-world Windows validation |
| 7 | Network & tracking audit | **Not started** — scoped 2026-09-21; static audit done, traffic capture and cleanup outstanding |
| 8 | Launcher backgrounds + per-instance overrides | **Not started** — scoped 2026-09-21 |
| 9 | Instance comparison | **Not started** — scoped 2026-09-21 (metadata + content only in v1) |

Goals 1-3 were agreed direction as of 2026-09-02; goal 4 was added on 2026-09-04. Goal 1
(concurrent multi-account launches) is implemented as of 2026-09-04. Goal 3 is fully implemented:
part 1 (telemetry, ads, promos, news/friends UI) landed 2026-09-04, and part 2 (Modrinth account
removal, sign-in/OAuth, cloud shared instances, and hosting/billing — which turned out to be
necessary during scoping and was a larger, separate pass) landed 2026-09-05. The other half, the
Discord Rich Presence rework, landed 2026-09-20 after being force-disabled as a stopgap since
2026-09-07: a Dyad-branded, opt-in presence with per-instance opt-out, multi-instance handling and
page-aware idle text — see goal 3's "Discord Rich Presence" section above. Goal 4 is implemented, in
its final form as update *notifications*: phase 1 (disabling Modrinth's updater) landed 2026-09-04,
and phase 2 (a fork-owned, opt-in Tauri self-updater on GitHub Releases) landed 2026-09-14 alongside
a rework of the build/release workflows onto standard GitHub-hosted runners (discovered to be
non-functional on Namespace Cloud runners this fork doesn't have) and a pruning of workflows scoped
to the web frontend/`labrinth` backend that this fork doesn't develop. The release pipeline was
then debugged through its first tagged releases on 2026-09-16/17. On 2026-09-21 the self-updater
was dropped as broken in production (the signed update bundles were never uploaded to releases) and
replaced with a plain version check that shows a banner and opens the installer download in the
browser; it runs unconditionally, and a few self-updater leftovers remain — see goal 4's "Current
behavior" and "Leftovers" sections. Goal 2 is not implemented yet.

Goals 5 (unsigned Windows installer/SmartScreen) and 6 (migrate-from-Modrinth-App import tool)
were added on 2026-09-13 after a scoping discussion with the user. Goal 5 is only partly
implemented: its two independent smaller fixes (the CI tag-signing fallback bug and the installer
metadata gap) landed 2026-09-13, but the installer is still unsigned, and the next concrete step
for the goal itself is still submitting a SignPath.io OSS-signing application. Goal 6 is
implemented end-to-end as of 2026-09-16, including Phase 4, the symlink-handling/empty-directory/
launch-overrides follow-ups, and a later round of real-install bug fixes — see goal 6's own
section above for the full phase breakdown and [goal-6-import-design.md](goal-6-import-design.md)
for the detailed writeup. No known gaps remain beyond real-world Windows validation, which this
fork's other machine should pick up next.

Goals 7-9 were added on 2026-09-21 after a scoping discussion with the user, none implemented yet.
Goal 7 is a follow-up to goal 3: a code audit found leftover Modrinth Servers/billing prefetches, a
GeoIP lookup, a third-party avatar service, the download-attribution header and dead Stripe/account
plumbing (see its findings list). Whether the always-on version check from goal 4 should get an off
switch is a decision item inside goal 7. Goal 8 (backgrounds) is scoped to user-supplied images and colours/gradients with
translucent-panel legibility controls; goal 9 (instance comparison) to metadata and content in v1.

This document should be updated as scope changes — treat it as the source of truth for what this fork
is trying to do, ahead of any individual issue or PR.
