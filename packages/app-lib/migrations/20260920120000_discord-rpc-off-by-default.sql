-- Discord Rich Presence is opt-in in this fork. The column was created with DEFAULT TRUE
-- (inherited from upstream Modrinth), so a fresh install would start with it on, and any
-- database that predates the temporary force-disable in Settings::get could still hold TRUE.
-- Reset the single settings row so presence is only ever on after an explicit opt-in.
UPDATE settings SET discord_rpc = FALSE;
