-- Launcher background (goal 8): a JSON-encoded BackgroundConfig (source, dim, blur).
-- NULL means no background, the default.
ALTER TABLE settings
ADD COLUMN background TEXT;
