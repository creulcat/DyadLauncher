-- sync_theme_across_devices and sync_behavior_across_devices (added 20260818120000) only ever
-- meant anything for the Modrinth-account-based settings sync feature, which goal 3 removed on
-- 2026-09-05 along with the rest of the account system. Nothing has read or written these columns
-- since; goal 7 drops them as leftover dead config.
ALTER TABLE settings DROP COLUMN sync_theme_across_devices;
ALTER TABLE settings DROP COLUMN sync_behavior_across_devices;
