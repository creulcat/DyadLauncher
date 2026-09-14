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

Explicitly **keep** Discord Rich Presence, but revisit/tweak its behavior (specifics TBD). Explicitly
**keep** anonymous content browsing/searching/downloading against Modrinth's API (no login required) —
that's the actual point of the launcher, not bloat.

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

### 4. Auto-update mechanism

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
follow-up work. (That follow-up landed as part of phase 2, below.)

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

### 5. Windows installer trust warning (SmartScreen)

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

**Separate fix, not blocked on the above:** `.github/workflows/theseus-build.yml`'s Windows
signing step currently takes the "sign" branch on any `refs/tags/v*` push and would fail outright
against this fork's empty DigiCert secrets, instead of degrading to an unsigned build the way
normal branch builds already do. This should be fixed regardless of which signing path is chosen,
so a tag/release build never hard-fails just because signing secrets aren't configured yet.

**Independent cheap improvement:** `apps/app/tauri.conf.json`'s `copyright`, `shortDescription`,
and `longDescription` fields are all empty strings, and no NSIS publisher is set. Filling these in
won't remove the SmartScreen prompt by itself, but makes the warning dialog identify a real app
instead of a blank one — and it's metadata a certificate would need attached anyway. Worth doing
regardless of signing status/timeline.

Not started, no implementation timeline yet — next step is submitting the SignPath application.

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

## Status

Goals 1-3 were agreed direction as of 2026-09-02; goal 4 was added on 2026-09-04. Goal 1
(concurrent multi-account launches) is implemented as of 2026-09-04. Goal 3 is fully implemented:
part 1 (telemetry, ads, promos, news/friends UI) landed 2026-09-04, and part 2 (Modrinth account
removal, sign-in/OAuth, cloud shared instances, and hosting/billing — which turned out to be
necessary during scoping and was a larger, separate pass) landed 2026-09-05. Goal 4 is fully
implemented: phase 1 (disabling Modrinth's updater) landed 2026-09-04, and phase 2 (the opt-in,
GitHub-Releases-backed updater) landed 2026-09-14, alongside a rework of `theseus-build.yml`/
`theseus-release.yml` onto standard GitHub-hosted runners (discovered to be non-functional on
Namespace Cloud runners this fork doesn't have) and a pruning of workflows scoped to the web
frontend/`labrinth` backend that this fork doesn't develop. Goal 2 is not implemented yet.

Goals 5 (unsigned Windows installer/SmartScreen) and 6 (migrate-from-Modrinth-App import tool)
were added on 2026-09-13 after a scoping discussion with the user. Neither is implemented yet.
Goal 5's next concrete step is submitting a SignPath.io OSS-signing application; the CI
tag-signing fallback bug and installer metadata gap are independent smaller fixes noted alongside
it. Goal 6 needs a technical design pass on reading the official Modrinth App's SQLite schema
safely before implementation can start.

This document should be updated as scope changes — treat it as the source of truth for what this fork
is trying to do, ahead of any individual issue or PR.
