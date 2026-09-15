//! Manual, ignored-by-default end-to-end check of goal 6 Phase 2 (actual
//! instance creation + content copy) against a real official Modrinth App
//! install. Not part of the normal test suite: it depends on machine-local
//! state (a real install, network access to download the game) and writes
//! a real instance into an isolated Dyad data directory.
//!
//! Uses a dedicated `app_identifier` so nothing here touches a real Dyad
//! install's data - safe to delete `~/.local/share/DyadLauncherManualTest`
//! (or platform equivalent) afterward. Run with:
//!
//!   cargo test -p theseus --test manual_migrate_app_import -- --ignored --nocapture

use std::time::Duration;
use theseus::install::InstallJobStatus;
use theseus::migrate_modrinth_app::execute::ImportSelection;
use theseus::migrate_modrinth_app::ContentCategory;

const TEST_APP_IDENTIFIER: &str = "DyadLauncherManualTest";

#[tokio::test]
#[ignore]
async fn import_real_instance_end_to_end() {
    theseus::start_logger(TEST_APP_IDENTIFIER);

    theseus::State::init(TEST_APP_IDENTIFIER.to_string())
        .await
        .expect("State::init should succeed");

    let source = theseus::migrate_modrinth_app::detect(None)
        .await
        .expect("detect should not error")
        .expect("expected a real Modrinth App install to be detected");

    assert!(
        !theseus::migrate_modrinth_app::is_source_running(),
        "close the official Modrinth App before running this test"
    );

    let preview = theseus::migrate_modrinth_app::build_preview(&source)
        .await
        .expect("build_preview should succeed against a real install");

    let candidate = preview
        .instances
        .first()
        .expect("expected at least one real instance to import");

    println!("Importing real instance: {:?}", candidate.name);

    let selection = ImportSelection {
        categories: vec![ContentCategory::Mods, ContentCategory::Config],
        worlds: vec![],
    };

    let snapshot = theseus::install::import_modrinth_app_instance(
        candidate.instance_dir.clone(),
        candidate.name.clone(),
        candidate
            .game_version
            .clone()
            .expect("expected a resolved game version"),
        candidate.loader,
        candidate.loader_version.clone(),
        candidate.icon_path.clone(),
        selection,
        false,
    )
    .await
    .expect("import_modrinth_app_instance should start successfully");

    let job_id = snapshot
        .job_id
        .parse()
        .expect("job_id should be a valid uuid");
    let instance_id = snapshot
        .instance_id
        .clone()
        .expect("expected the initial instance to already be created");

    println!("Started install job {job_id} for instance {instance_id}");

    let final_snapshot = tokio::time::timeout(Duration::from_secs(900), async {
        loop {
            let job = theseus::install::get_job(job_id)
                .await
                .expect("get_job should succeed");
            println!(
                "  status={:?} phase={:?} progress={:?}",
                job.status, job.phase, job.progress
            );
            match job.status {
                InstallJobStatus::Succeeded
                | InstallJobStatus::Failed
                | InstallJobStatus::Canceled => break job,
                _ => tokio::time::sleep(Duration::from_secs(3)).await,
            }
        }
    })
    .await
    .expect("install job did not finish within 15 minutes");

    if let Some(error) = &final_snapshot.error {
        panic!("install job failed: {error:?}");
    }
    assert_eq!(final_snapshot.status, InstallJobStatus::Succeeded);

    let instances = theseus::instance::list()
        .await
        .expect("instance::list should succeed");
    let imported = instances
        .iter()
        .find(|metadata| metadata.instance.id == instance_id)
        .expect("imported instance should be registered");

    println!(
        "Imported instance registered: name={:?} path={:?} loader={:?} game_version={:?}",
        imported.instance.name,
        imported.instance.path,
        imported.applied_content_set.loader,
        imported.applied_content_set.game_version,
    );

    let instance_dir = theseus::instance::get_full_path(&instance_id)
        .await
        .expect("get_full_path should succeed for the imported instance");
    let mods_dir = instance_dir.join("mods");
    let config_dir = instance_dir.join("config");
    assert!(
        mods_dir.exists(),
        "expected mods/ to have been copied into the new instance"
    );
    assert!(
        config_dir.exists(),
        "expected config/ to have been copied into the new instance"
    );

    let source_mods_count = std::fs::read_dir(candidate.instance_dir.join("mods"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    let copied_mods_count = std::fs::read_dir(&mods_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    println!(
        "mods/: source had {source_mods_count} entries, copied instance has {copied_mods_count}"
    );
    assert_eq!(source_mods_count, copied_mods_count);

    println!(
        "Source instance dir untouched check: {:?} still exists = {}",
        candidate.instance_dir,
        candidate.instance_dir.exists()
    );
}
