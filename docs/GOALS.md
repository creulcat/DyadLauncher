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

### 3. Debloating the desktop app — removal done, Discord Rich Presence still open

Remove:

- Telemetry/analytics calls
- Account/login promos & ads
- News/Discover/social panels

Explicitly **keep** Discord Rich Presence, but revisit/tweak its behavior (specifics TBD). Explicitly
**keep** anonymous content browsing/searching/downloading against Modrinth's API (no login required) —
that's the actual point of the launcher, not bloat.

**Discord Rich Presence — still to do.** The removal work below is complete, but this half of the
goal is not: Rich Presence is currently a stopgap, not the finished feature. As of 2026-09-07
(commit `01e887690`) it is force-disabled — `Settings::get()` in
`packages/app-lib/src/state/settings.rs` hardcodes `discord_rpc: false` regardless of the stored
value, and the Privacy settings tab (its only setting) is hidden via `hidden: true` in
`AppSettingsModal.vue`. The reason is that the presence still shows Modrinth branding (the
registered Discord Application's name and icon), which this fork doesn't want to display. The
code, the settings column, and its write path were deliberately left intact so it can be turned
back on. Remaining work to actually deliver on "keep and tweak":

- Register a Dyad-owned Discord Application (name/icon/assets) and point the client ID at it.
- Decide and implement the tweaked behavior (specifics still TBD — ask the user before starting).
- Remove the `discord_rpc: false` override in `Settings::get()` and the `hidden` flag on the
  Privacy tab, restoring the toggle.

Until then, users have no way to enable Rich Presence.

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

### 4. Auto-update mechanism — done

The desktop app's auto-update mechanism (`tauri-plugin-updater`, wired up in
`apps/app/src/updater_impl.rs`) is only compiled in behind the `updater` Cargo feature, and is only
pointed at a real endpoint via `apps/app/tauri-release.conf.json`, which targets Modrinth's own
update feed (`https://launcher-files.modrinth.com/updates.json`) and Modrinth's signing pubkey.

**Phase 1 — done:** `apps/app/tauri-release.conf.json` no longer enables the `updater` Cargo
feature/capability or the `plugins.updater` block, so this fork's release builds never compile in
or point at Modrinth's update endpoint/pubkey — they behave the same as a plain local build in this
respect. `.github/workflows/theseus-build.yml`'s Windows step was updated to stop requesting the
now-unconfigured `updater` bundle target. `.github/workflows/theseus-release.yml` (which uploaded
signed update manifests to Modrinth's own S3 bucket) was left untouched at the time — rebranding/
reworking the fork's own release-publishing pipeline was called out as separate, unscoped
follow-up work. (That follow-up landed as part of phase 2, below; that workflow has since been
folded into `theseus-build.yml` and no longer exists as a separate file.)

A second, independent check also had to be removed: `apps/app-frontend/src/App.vue` had a
`checkLinuxUpdates()` fallback that did a raw `fetch('https://launcher-files.modrinth.com/updates.json')`
whenever `areUpdatesEnabled()` was false on Linux — i.e. it was written to activate in exactly the
"updater feature disabled" state this phase now puts every platform in. It has been deleted (call
site, function, and its now-unused `linuxBody` i18n message) so no code path hits Modrinth's update
endpoint any more.

- Known tradeoff: without any updater active, users of the fork get no in-app notice of new fork
  releases and must check manually (e.g. GitHub releases) until phase 2 lands.

**Phase 2 — done:** Built a fork-owned auto-updater that is **opt-in** (off by default, per-user
setting under Behavior settings → Updates) and backed by **GitHub Releases** instead of Modrinth's
infrastructure.

**Implemented as of 2026-09-14:**

- Generated a fork-owned Ed25519 update-signing keypair (unrelated to goal 5's Authenticode/
  SmartScreen signing — this one covers update-payload integrity, not binary trust). The public
  key is committed in `apps/app/tauri-release.conf.json`'s new `plugins.updater` block; the
  private key and its password live only in this repo's `TAURI_PRIVATE_KEY`/`TAURI_KEY_PASSWORD`
  Actions secrets, which `.github/workflows/theseus-build.yml` already passed to every `tauri
  build` invocation (inherited from upstream, previously unused since this fork had no key of its
  own to put there).
- Re-enabled the `updater` Cargo feature (via `tauri-release.conf.json`'s `build.features`) and
  the `updater` capability, and pointed the update feed at
  `https://github.com/creulcat/DyadLauncher/releases/latest/download/updates.json` — GitHub always
  resolves that alias to whichever release is newest, so publishing a release *is* going live for
  the updater; there's no separate publish step.
- New off-by-default `check_for_updates` setting (`packages/app-lib/src/state/settings.rs`,
  migration `20260914120000_add-check-for-updates-setting.sql`), surfaced as a toggle in
  `BehaviorSettings.vue`. It gates `App.vue`'s `checkUpdates()` on top of the existing build-time
  `areUpdatesEnabled()` check, so update checks stay off until a user explicitly turns them on.
  Known tradeoff: toggling it on takes effect on the next launch, not immediately, since the
  check-scheduling loop is only started once at startup.
- Fixed the hardcoded "Modrinth App" branding in the update-download popup copy in `App.vue` (and
  regenerated `apps/app-frontend/src/locales/en-US/index.json` via the project's own
  `intl:extract` script), which had never been updated after the product was renamed.

**Prerequisite fix, discovered while scoping this phase:** `.github/workflows/theseus-build.yml`
and `.github/workflows/theseus-release.yml` ran on `namespace-profile-*` runners — Modrinth's own
paid Namespace Cloud pool, not available to this fork — so neither workflow could actually run
here at all; jobs would sit queued indefinitely. Both were rewritten onto standard GitHub-hosted
runners (`macos-latest`/`windows-latest`/`ubuntu-22.04`), dropping the Namespace-specific caching
steps in favor of each action's own built-in caching. `theseus-release.yml` was further reworked
to drop its Modrinth-S3 upload (no secrets for it in this fork) in favor of publishing
`updates.json` and the signed bundles as GitHub Release assets directly — since GitHub replaces
whitespace in uploaded asset filenames with `.`, every bundle is staged into a flat,
space-free-named `release-assets/` directory first so the uploaded name matches the name embedded
in the manifest exactly. Both workflows also had a dormant bug from the same rename — hardcoded
"Modrinth App" bundle filename patterns that never matched the actual "Dyad Launcher"-named build
output — fixed alongside the runner migration. `manual-build.yml`, a stopgap manual-trigger-only
workflow added to have *something* that reliably built the app on standard runners, is now
redundant and was deleted. Twelve other workflows scoped to the web frontend/`labrinth`
backend/Modrinth-account-specific infra (Crowdin i18n sync, `labrinth` Docker builds, the
`daedalus` metadata service, the ArgoCD `/deploy` slash command, etc.) were deleted as out of
scope for a desktop-app-only fork per this document's own framing.

No beta/prerelease channel distinction was added — every pushed tag is published as a full
release and becomes "latest" for the update feed, by deliberate choice.

**Release-pipeline fixes, 2026-09-16/17** (found while getting the first tagged release through
CI):

- The release job was folded into `theseus-build.yml` (commit `2fcb325fa`), replacing the separate
  `theseus-release.yml`: `workflow_run` doesn't reliably fire for workflows triggered by a tag
  push, so the separate release workflow never actually ran for tagged builds. The release job is
  now gated on the build job succeeding for a tag ref.
- Bundle staging now locates each release file by name under the downloaded artifact directory
  instead of a hardcoded nested path (`d68398da5`), which didn't match `upload-artifact`'s
  directory layout for wildcarded multi-path uploads.
- `bundle.createUpdaterArtifacts` is now set to `"v1Compatible"` in
  `apps/app/tauri-release.conf.json` (`6bfc74044`). Tauri v2 only builds the updater bundles and
  `.sig` files when that is set explicitly — a signing key alone isn't enough — and the release
  workflow expects the v1-style `.nsis.zip`/`.AppImage.tar.gz` filenames.
- **The update-signing keypair was rotated** (`4e2028464`). The `TAURI_PRIVATE_KEY`/
  `TAURI_KEY_PASSWORD` secrets from the initial phase 2 setup turned out not to match each other;
  it went unnoticed because signing was never exercised until `createUpdaterArtifacts` was turned
  on, at which point the build hard-failed with "incorrect updater private key password". Since no
  release had shipped with working updater artifacts yet, rotating was safe. The public key in
  `tauri-release.conf.json` was replaced with the new one; the new private key and password were
  set directly as repo secrets and are not committed. (So the "generated a fork-owned keypair"
  bullet above describes the original keypair, which is no longer the one in use.)
- macOS code signing is now skipped on forks without Apple secrets (`b4ed482bf`), via separate
  signed/unsigned build steps gated on `secrets.APPLE_CERTIFICATE` — the same fallback pattern the
  Windows signing step already uses.

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

## Status

Last reviewed 2026-09-20.

| # | Goal | Status |
|---|---|---|
| 1 | Concurrent multi-account launches | Done (2026-09-04) |
| 2 | Symlink-based resource sharing | **Not started** |
| 3 | Debloating | Removal done (2026-09-04/05); **Discord Rich Presence rework still open** |
| 4 | Auto-update mechanism | Done (phase 1 2026-09-04, phase 2 2026-09-14, release-pipeline fixes 2026-09-16/17) |
| 5 | Windows installer trust warning | **Partly done** — CI fallback fix and installer metadata landed 2026-09-13; installer is still unsigned, SignPath application not yet submitted |
| 6 | Migrate-from-Modrinth-App import | Done (2026-09-16), pending real-world Windows validation |

Goals 1-3 were agreed direction as of 2026-09-02; goal 4 was added on 2026-09-04. Goal 1
(concurrent multi-account launches) is implemented as of 2026-09-04. Goal 3's removal work is
implemented: part 1 (telemetry, ads, promos, news/friends UI) landed 2026-09-04, and part 2
(Modrinth account removal, sign-in/OAuth, cloud shared instances, and hosting/billing — which
turned out to be necessary during scoping and was a larger, separate pass) landed 2026-09-05. The
other half of goal 3 is not done: Discord Rich Presence is currently force-disabled and hidden as a
stopgap (2026-09-07) and still needs to be properly rebranded and re-enabled — see goal 3's
"Discord Rich Presence — still to do" section above. Goal 4 is implemented: phase 1 (disabling
Modrinth's updater) landed 2026-09-04, and phase 2 (the opt-in, GitHub-Releases-backed updater)
landed 2026-09-14, alongside a rework of the build/release workflows onto standard GitHub-hosted
runners (discovered to be non-functional on Namespace Cloud runners this fork doesn't have) and a
pruning of workflows scoped to the web frontend/`labrinth` backend that this fork doesn't develop.
The release pipeline was then debugged through its first tagged releases on 2026-09-16/17,
including folding the release job into `theseus-build.yml` and rotating the update-signing keypair
— see goal 4's "Release-pipeline fixes" section. Goal 2 is not implemented yet.

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

This document should be updated as scope changes — treat it as the source of truth for what this fork
is trying to do, ahead of any individual issue or PR.
