-- Tracks which Dyad instance (if any) a given official-Modrinth-App source
-- instance was already imported into, so goal 6's importer can warn on a
-- re-import instead of silently creating a duplicate instance.
CREATE TABLE modrinth_app_import_sources (
	instance_id TEXT NOT NULL,
	source_settings_dir TEXT NOT NULL,
	source_instance_id TEXT NOT NULL,
	imported_at INTEGER NOT NULL,

	PRIMARY KEY (instance_id),
	FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE INDEX modrinth_app_import_sources_source
	ON modrinth_app_import_sources(source_settings_dir, source_instance_id);
