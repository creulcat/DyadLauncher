//! Reads and writes against Dyad's *own* database for goal 6's "was this
//! source instance already imported before" check (see
//! `modrinth_app_import_sources` in the migrations). Unlike `source_db`
//! (which only ever reads the official Modrinth App's database), everything
//! here operates on Dyad's own `state.pool` - a source instance is
//! identified by the pair `(source_settings_dir, source_instance_id)` since
//! more than one official-app-shaped install could in principle be imported
//! from (a custom `source_dir`, or a reinstalled official app with a fresh
//! database), and a bare source instance id alone isn't guaranteed unique
//! across those.
//!
//! Plain runtime-checked `sqlx::query` (not the `query!` macro) throughout,
//! matching `source_db.rs`'s approach - simplest way to avoid this one small
//! feature needing its own offline-query-cache regeneration step.

use sqlx::{Row, SqlitePool};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingImport {
    pub instance_id: String,
    /// `None` if the linked instance has since been deleted out from under
    /// this record - callers should treat that the same as "not imported"
    /// for warning purposes, but the stale row itself is left for
    /// `record_import`'s `INSERT OR REPLACE` to clean up on the next import.
    pub instance_name: Option<String>,
    pub imported_at: i64,
}

fn source_key(source_settings_dir: &Path) -> String {
    source_settings_dir.to_string_lossy().into_owned()
}

/// Looks up whether `source_instance_id` (from `source_settings_dir`) has
/// already been imported into an instance that still exists.
pub(super) async fn find_existing_import(
    pool: &SqlitePool,
    source_settings_dir: &Path,
    source_instance_id: &str,
) -> crate::Result<Option<ExistingImport>> {
    let row = sqlx::query(
        "
        SELECT
            s.instance_id AS instance_id,
            i.name AS instance_name,
            s.imported_at AS imported_at
        FROM modrinth_app_import_sources s
        LEFT JOIN instances i ON i.id = s.instance_id
        WHERE s.source_settings_dir = ?1 AND s.source_instance_id = ?2
        ",
    )
    .bind(source_key(source_settings_dir))
    .bind(source_instance_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| ExistingImport {
        instance_id: row.get("instance_id"),
        instance_name: row.get("instance_name"),
        imported_at: row.get("imported_at"),
    }))
}

/// Records that `instance_id` was just created by importing
/// `source_instance_id` from `source_settings_dir`, so a later import of the
/// same source instance can warn instead of silently duplicating it.
/// `INSERT OR REPLACE` on the primary key (`instance_id`) so re-recording
/// for the same destination instance (e.g. a retried job) never conflicts.
pub(crate) async fn record_import(
    pool: &SqlitePool,
    instance_id: &str,
    source_settings_dir: &Path,
    source_instance_id: &str,
) -> crate::Result<()> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "
        INSERT OR REPLACE INTO modrinth_app_import_sources
            (instance_id, source_settings_dir, source_instance_id, imported_at)
        VALUES (?1, ?2, ?3, ?4)
        ",
    )
    .bind(instance_id)
    .bind(source_key(source_settings_dir))
    .bind(source_instance_id)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}
