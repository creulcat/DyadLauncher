-- Dyad Launcher does not report telemetry/analytics or show personalized ads,
-- so these settings no longer have any effect. Drop the columns entirely.
ALTER TABLE settings DROP COLUMN telemetry;
ALTER TABLE settings DROP COLUMN personalized_ads;
