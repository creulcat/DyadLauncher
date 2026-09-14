//! Schema-defensive reads against the source Modrinth-App database.
//!
//! Every query here is a named-column `SELECT` (never `SELECT *`, never a
//! bulk table copy), and every table is checked with `PRAGMA table_info`
//! before it's queried - see the compatibility strategy in
//! `docs/goal-6-import-design.md`. A required table/column missing produces
//! a specific `InputError` naming exactly what wasn't found; an optional
//! `settings` column missing is just skipped and reported back as a
//! compatibility note instead of failing the whole import.

use super::ImportSettingsCandidate;
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashSet;
use std::path::PathBuf;

/// Tables (and, for `instances`/`instance_content_sets`/`java_versions`,
/// their required columns) this reader can't do without. `settings` is
/// handled separately below since every column on it is best-effort.
const REQUIRED_TABLES_AND_COLUMNS: &[(&str, &[&str])] = &[
    (
        "instances",
        &[
            "id",
            "path",
            "name",
            "icon_path",
            "created",
            "modified",
            "last_played",
            "applied_content_set_id",
        ],
    ),
    (
        "instance_content_sets",
        &["id", "game_version", "loader", "loader_version"],
    ),
    (
        "java_versions",
        &["major_version", "full_version", "architecture", "path"],
    ),
    ("settings", &[]),
];

/// Global-settings columns worth offering for import. Every one of these is
/// optional: a source schema missing one just means that field isn't
/// offered, not that the whole import fails.
const SETTINGS_ALLOWLIST: &[&str] = &[
    "extra_launch_args",
    "custom_env_vars",
    "mc_memory_max",
    "mc_force_fullscreen",
    "mc_game_resolution_x",
    "mc_game_resolution_y",
    "hook_pre_launch",
    "hook_wrapper",
    "hook_post_exit",
];

async fn table_columns(
    pool: &Pool<Sqlite>,
    table: &str,
) -> crate::Result<HashSet<String>> {
    // `table` only ever comes from the hardcoded lists above, never from
    // user input, so building the PRAGMA statement with format! is safe.
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("name"))
        .collect())
}

fn unsupported_schema_error(detail: &str) -> crate::Error {
    crate::ErrorKind::InputError(format!(
        "This Modrinth App install's database {detail} - it may be too old \
         or too new a version to import from."
    ))
    .into()
}

pub(super) async fn check_required_schema(
    pool: &Pool<Sqlite>,
) -> crate::Result<()> {
    for (table, required_columns) in REQUIRED_TABLES_AND_COLUMNS {
        let existing = table_columns(pool, table).await?;
        if existing.is_empty() {
            return Err(unsupported_schema_error(&format!(
                "is missing the expected `{table}` table"
            )));
        }
        for column in *required_columns {
            if !existing.contains(*column) {
                return Err(unsupported_schema_error(&format!(
                    "is missing the expected `{table}.{column}` column"
                )));
            }
        }
    }
    Ok(())
}

pub(super) struct RawInstance {
    pub id: String,
    pub path: String,
    pub name: String,
    pub icon_path: Option<String>,
    pub created: i64,
    pub modified: i64,
    pub last_played: Option<i64>,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
}

pub(super) async fn fetch_instances(
    pool: &Pool<Sqlite>,
) -> crate::Result<Vec<RawInstance>> {
    let rows = sqlx::query(
        "
        SELECT
            i.id, i.path, i.name, i.icon_path, i.created, i.modified, i.last_played,
            cs.game_version AS game_version, cs.loader AS loader,
            cs.loader_version AS loader_version
        FROM instances i
        LEFT JOIN instance_content_sets cs ON cs.id = i.applied_content_set_id
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| RawInstance {
            id: row.get("id"),
            path: row.get("path"),
            name: row.get("name"),
            icon_path: row.get("icon_path"),
            created: row.get("created"),
            modified: row.get("modified"),
            last_played: row.get("last_played"),
            game_version: row.get("game_version"),
            loader_version: row.get("loader_version"),
            loader: row.get("loader"),
        })
        .collect())
}

pub(super) struct RawJavaVersion {
    pub major_version: i64,
    pub full_version: String,
    pub architecture: String,
    pub path: String,
}

pub(super) async fn fetch_java_versions(
    pool: &Pool<Sqlite>,
) -> crate::Result<Vec<RawJavaVersion>> {
    let rows = sqlx::query(
        "SELECT major_version, full_version, architecture, path FROM java_versions",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| RawJavaVersion {
            major_version: row.get("major_version"),
            full_version: row.get("full_version"),
            architecture: row.get("architecture"),
            path: row.get("path"),
        })
        .collect())
}

/// Reads the source app's `settings.custom_dir` override - its *config* dir
/// (equivalent to Dyad's own `DirectoryInfo.config_dir`), under which
/// `profiles/`, `caches/`, etc. all live, if the user pointed it somewhere
/// nonstandard. `None` means "use the source app's settings dir itself" -
/// either because the column doesn't exist on this schema, or because it was
/// never set.
pub(super) async fn read_custom_config_dir(
    pool: &Pool<Sqlite>,
) -> crate::Result<Option<PathBuf>> {
    let existing = table_columns(pool, "settings").await?;
    if !existing.contains("custom_dir") {
        return Ok(None);
    }

    let row = sqlx::query("SELECT custom_dir FROM settings")
        .fetch_one(pool)
        .await?;
    let custom_dir: Option<String> = row.try_get("custom_dir").ok().flatten();

    Ok(custom_dir.filter(|dir| !dir.is_empty()).map(PathBuf::from))
}

pub(super) async fn fetch_settings_candidate(
    pool: &Pool<Sqlite>,
) -> crate::Result<(ImportSettingsCandidate, Vec<String>)> {
    let existing = table_columns(pool, "settings").await?;

    let mut notes = Vec::new();
    let mut present = Vec::new();
    for column in SETTINGS_ALLOWLIST {
        if existing.contains(*column) {
            present.push(*column);
        } else {
            notes.push(format!(
                "source `settings` table has no `{column}` column - that field won't be offered"
            ));
        }
    }

    let mut candidate = ImportSettingsCandidate::default();
    if present.is_empty() {
        return Ok((candidate, notes));
    }

    let select_list = present
        .iter()
        .map(|column| {
            if matches!(*column, "extra_launch_args" | "custom_env_vars") {
                format!("json({column}) AS {column}")
            } else {
                (*column).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let row = sqlx::query(&format!("SELECT {select_list} FROM settings"))
        .fetch_one(pool)
        .await?;

    let has = |column: &str| present.contains(&column);

    if has("extra_launch_args") {
        let raw: Option<String> = row.try_get("extra_launch_args").ok();
        candidate.extra_launch_args =
            raw.and_then(|json| serde_json::from_str(&json).ok());
    }
    if has("custom_env_vars") {
        let raw: Option<String> = row.try_get("custom_env_vars").ok();
        candidate.custom_env_vars =
            raw.and_then(|json| serde_json::from_str(&json).ok());
    }
    if has("mc_memory_max")
        && let Ok(value) = row.try_get::<i64, _>("mc_memory_max")
    {
        candidate.memory_maximum_mb = Some(value as u32);
    }
    if has("mc_force_fullscreen")
        && let Ok(value) = row.try_get::<i64, _>("mc_force_fullscreen")
    {
        candidate.force_fullscreen = Some(value == 1);
    }
    if has("mc_game_resolution_x")
        && has("mc_game_resolution_y")
        && let (Ok(x), Ok(y)) = (
            row.try_get::<i64, _>("mc_game_resolution_x"),
            row.try_get::<i64, _>("mc_game_resolution_y"),
        )
    {
        candidate.game_resolution = Some((x as u16, y as u16));
    }
    if has("hook_pre_launch") {
        candidate.hook_pre_launch = row
            .try_get::<Option<String>, _>("hook_pre_launch")
            .ok()
            .flatten();
    }
    if has("hook_wrapper") {
        candidate.hook_wrapper = row
            .try_get::<Option<String>, _>("hook_wrapper")
            .ok()
            .flatten();
    }
    if has("hook_post_exit") {
        candidate.hook_post_exit = row
            .try_get::<Option<String>, _>("hook_post_exit")
            .ok()
            .flatten();
    }

    Ok((candidate, notes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    /// A minimal in-memory database with just the tables/columns this
    /// reader cares about - standing in for a real official-app `app.db`
    /// without needing one checked into the repo. `extra_sql` lets a test
    /// simulate schema drift (a missing column, a renamed table, etc.).
    async fn fixture_db(extra_sql: &str) -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().in_memory(true))
            .await
            .expect("in-memory sqlite should always open");

        sqlx::query(
            "
            CREATE TABLE instances (
                id TEXT NOT NULL,
                path TEXT NOT NULL,
                name TEXT NOT NULL,
                icon_path TEXT,
                created INTEGER NOT NULL,
                modified INTEGER NOT NULL,
                last_played INTEGER,
                applied_content_set_id TEXT
            );
            ",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "
            CREATE TABLE instance_content_sets (
                id TEXT NOT NULL,
                game_version TEXT NOT NULL,
                loader TEXT NOT NULL,
                loader_version TEXT
            );
            ",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "
            CREATE TABLE java_versions (
                major_version INTEGER NOT NULL,
                full_version TEXT NOT NULL,
                architecture TEXT NOT NULL,
                path TEXT NOT NULL
            );
            ",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "
            CREATE TABLE settings (
                mc_memory_max INTEGER NOT NULL DEFAULT 2048,
                mc_force_fullscreen INTEGER NOT NULL DEFAULT 0,
                mc_game_resolution_x INTEGER NOT NULL DEFAULT 854,
                mc_game_resolution_y INTEGER NOT NULL DEFAULT 480,
                hook_pre_launch TEXT,
                hook_wrapper TEXT,
                hook_post_exit TEXT,
                extra_launch_args JSONB NOT NULL DEFAULT '[]',
                custom_env_vars JSONB NOT NULL DEFAULT '[]',
                custom_dir TEXT
            );
            INSERT INTO settings DEFAULT VALUES;
            ",
        )
        .execute(&pool)
        .await
        .unwrap();

        if !extra_sql.is_empty() {
            sqlx::query(extra_sql).execute(&pool).await.unwrap();
        }

        pool
    }

    #[tokio::test]
    async fn accepts_a_matching_schema() {
        let pool = fixture_db("").await;
        check_required_schema(&pool)
            .await
            .expect("a schema with every expected table/column should pass");
    }

    #[tokio::test]
    async fn rejects_a_missing_required_column() {
        // Simulate an official-app version whose `instances` table never
        // grew a `last_played` column (or dropped it) - should fail with a
        // named error, not a generic SQL error.
        let pool = fixture_db(
            "
            CREATE TABLE instances_new (
                id TEXT NOT NULL, path TEXT NOT NULL, name TEXT NOT NULL,
                icon_path TEXT, created INTEGER NOT NULL, modified INTEGER NOT NULL,
                applied_content_set_id TEXT
            );
            DROP TABLE instances;
            ALTER TABLE instances_new RENAME TO instances;
            ",
        )
        .await;

        let err = check_required_schema(&pool)
            .await
            .expect_err("a missing required column should be rejected");
        let message = err.to_string();
        assert!(
            message.contains("instances.last_played"),
            "error should name the missing column, got: {message}"
        );
    }

    #[tokio::test]
    async fn rejects_a_missing_required_table() {
        let pool = fixture_db("DROP TABLE java_versions;").await;

        let err = check_required_schema(&pool)
            .await
            .expect_err("a missing required table should be rejected");
        assert!(err.to_string().contains("java_versions"));
    }

    #[tokio::test]
    async fn settings_candidate_defaults_missing_optional_columns() {
        // Simulate an official-app version that never grew `hook_wrapper` -
        // should not fail, just skip that one field and note it.
        let pool = fixture_db(
            "
            CREATE TABLE settings_new (
                mc_memory_max INTEGER NOT NULL DEFAULT 2048,
                mc_force_fullscreen INTEGER NOT NULL DEFAULT 0,
                mc_game_resolution_x INTEGER NOT NULL DEFAULT 854,
                mc_game_resolution_y INTEGER NOT NULL DEFAULT 480,
                hook_pre_launch TEXT,
                hook_post_exit TEXT,
                extra_launch_args JSONB NOT NULL DEFAULT '[]',
                custom_env_vars JSONB NOT NULL DEFAULT '[]',
                custom_dir TEXT
            );
            INSERT INTO settings_new DEFAULT VALUES;
            DROP TABLE settings;
            ALTER TABLE settings_new RENAME TO settings;
            ",
        )
        .await;

        let (candidate, notes) = fetch_settings_candidate(&pool).await.unwrap();

        assert_eq!(candidate.memory_maximum_mb, Some(2048));
        assert_eq!(candidate.hook_wrapper, None);
        assert!(
            notes.iter().any(|n| n.contains("hook_wrapper")),
            "should note the skipped column, got: {notes:?}"
        );
    }

    #[tokio::test]
    async fn fetch_instances_handles_unapplied_content_set() {
        let pool = fixture_db(
            "
            INSERT INTO instances
                (id, path, name, icon_path, created, modified, last_played, applied_content_set_id)
            VALUES
                ('a', 'a', 'Instance A', NULL, 0, 0, NULL, NULL),
                ('b', 'b', 'Instance B', NULL, 0, 0, NULL, 'cs-1');
            INSERT INTO instance_content_sets (id, game_version, loader, loader_version)
            VALUES ('cs-1', '1.20.1', 'fabric', '0.15.0');
            ",
        )
        .await;

        let instances = fetch_instances(&pool).await.unwrap();
        assert_eq!(instances.len(), 2);

        let a = instances.iter().find(|i| i.id == "a").unwrap();
        assert_eq!(a.game_version, None);
        assert_eq!(a.loader, None);

        let b = instances.iter().find(|i| i.id == "b").unwrap();
        assert_eq!(b.game_version.as_deref(), Some("1.20.1"));
        assert_eq!(b.loader.as_deref(), Some("fabric"));
    }
}
