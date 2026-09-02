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

## Status

All four goals above are agreed direction as of 2026-09-02; none are implemented yet. This
document should be updated as scope changes — treat it as the source of truth for what this fork
is trying to do, ahead of any individual issue or PR.
