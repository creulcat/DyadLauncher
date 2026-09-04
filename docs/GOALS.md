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

### 3. Debloating the desktop app

Remove:

- Telemetry/analytics calls
- Account/login promos & ads
- News/Discover/social panels

Explicitly **keep** Discord Rich Presence, but revisit/tweak its behavior (specifics TBD).

### 4. Auto-update mechanism

The desktop app's auto-update mechanism (`tauri-plugin-updater`, wired up in
`apps/app/src/updater_impl.rs`) is only compiled in behind the `updater` Cargo feature, and is only
pointed at a real endpoint via `apps/app/tauri-release.conf.json`, which targets Modrinth's own
update feed (`https://launcher-files.modrinth.com/updates.json`) and Modrinth's signing pubkey.

**Phase 1 — done:** `apps/app/tauri-release.conf.json` no longer enables the `updater` Cargo
feature/capability or the `plugins.updater` block, so this fork's release builds never compile in
or point at Modrinth's update endpoint/pubkey — they behave the same as a plain local build in this
respect. `.github/workflows/theseus-build.yml`'s Windows step was updated to stop requesting the
now-unconfigured `updater` bundle target. `.github/workflows/theseus-release.yml` (which uploads
signed update manifests to Modrinth's own S3 bucket) was left untouched — it already can't run
meaningfully here (no Modrinth secrets in this fork's repo) and rebranding/reworking the fork's own
release-publishing pipeline is separate, unscoped follow-up work.

A second, independent check also had to be removed: `apps/app-frontend/src/App.vue` had a
`checkLinuxUpdates()` fallback that did a raw `fetch('https://launcher-files.modrinth.com/updates.json')`
whenever `areUpdatesEnabled()` was false on Linux — i.e. it was written to activate in exactly the
"updater feature disabled" state this phase now puts every platform in. It has been deleted (call
site, function, and its now-unused `linuxBody` i18n message) so no code path hits Modrinth's update
endpoint any more.

- Known tradeoff: without any updater active, users of the fork get no in-app notice of new fork
  releases and must check manually (e.g. GitHub releases) until phase 2 lands.

**Phase 2 — future, not yet scoped:** Build a fork-owned auto-updater that is **opt-in** (off by
default) and backed by **GitHub Releases** instead of Modrinth's infrastructure. This needs its own
design pass later — at minimum: this fork's own signing keypair, an update manifest generated from
GitHub Releases (or a compatible static feed), and a user-facing setting to turn it on. Not
started, no implementation timeline yet.

## Status

Goals 1-3 were agreed direction as of 2026-09-02; goal 4 was added on 2026-09-04. Goal 1
(concurrent multi-account launches) is implemented as of 2026-09-04. Goal 4's phase 1
(disabling Modrinth's updater) is implemented as of 2026-09-04 — phase 2 (the opt-in GitHub-Releases
updater) is a future idea, not yet scoped or started. Goals 2-3 are not implemented yet. This
document should be updated as scope changes — treat it as the source of truth for what this fork is
trying to do, ahead of any individual issue or PR.
