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

### 1. Concurrent multi-account launches

Allow the same instance to be launched more than once at a time, each under a different
Microsoft account, as long as the instance has opted into this via a per-instance setting.

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

**Phase 1 — near-term, active:** Disable Modrinth's updater for this fork. A plain local build
already excludes it (feature-gated), but this fork's own release/CI pipeline must not reuse
`tauri-release.conf.json` as-is — it should never check against or advertise itself to Modrinth's
update infrastructure using Modrinth's endpoint/pubkey.

- Known tradeoff: without any updater active, users of the fork get no in-app notice of new fork
  releases and must check manually (e.g. GitHub releases) until phase 2 lands.

**Phase 2 — future, not yet scoped:** Build a fork-owned auto-updater that is **opt-in** (off by
default) and backed by **GitHub Releases** instead of Modrinth's infrastructure. This needs its own
design pass later — at minimum: this fork's own signing keypair, an update manifest generated from
GitHub Releases (or a compatible static feed), and a user-facing setting to turn it on. Not
started, no implementation timeline yet.

## Status

Goals 1-3 were agreed direction as of 2026-09-02; goal 4 was added on 2026-09-04, with only its
phase 1 (disabling Modrinth's updater) currently active — phase 2 (the opt-in GitHub-Releases
updater) is a future idea, not yet scoped or started. None of the four goals are implemented yet.
This document should be updated as scope changes — treat it as the source of truth for what this
fork is trying to do, ahead of any individual issue or PR.
