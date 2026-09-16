-- Opt-in (off by default) toggle for the fork-owned GitHub-Releases-backed auto-updater.
-- Unlike the upstream Modrinth App, update checks here are never on by default -- this column
-- gates whether the frontend ever calls the updater plugin's check command at all.
ALTER TABLE settings
ADD COLUMN check_for_updates INTEGER NOT NULL DEFAULT FALSE;
