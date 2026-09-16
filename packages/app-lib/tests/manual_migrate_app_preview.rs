//! Manual, ignored-by-default check against a real official Modrinth App
//! install, for validating Phase 1 of goal 6 (see
//! `docs/goal-6-import-design.md`). Not part of the normal test suite since
//! it depends on machine-local state (a real install) that CI and other
//! machines won't have. Run with:
//!
//!   cargo test -p theseus --test manual_migrate_app_preview -- --ignored --nocapture

#[tokio::test]
#[ignore]
async fn preview_against_real_install() {
    let source = theseus::migrate_modrinth_app::detect(None)
        .await
        .expect("detect should not error")
        .expect("expected a real Modrinth App install to be detected");

    println!("Detected source: {source:?}");

    let running = theseus::migrate_modrinth_app::is_source_running();
    println!("Source running: {running}");

    if running {
        println!(
            "Modrinth App appears to be running - skipping preview build."
        );
        return;
    }

    let preview = theseus::migrate_modrinth_app::build_preview(&source)
        .await
        .expect("build_preview should succeed against a real install");

    println!("Instances found: {}", preview.instances.len());
    for instance in &preview.instances {
        println!(
            "  - {:?} at {:?} (loader={:?} raw={:?}, mc={:?}, last_played={:?}) icon={:?}",
            instance.name,
            instance.instance_dir,
            instance.loader,
            instance.raw_loader,
            instance.game_version,
            instance.last_played,
            instance.icon_path,
        );
        for category in &instance.categories {
            println!(
                "      {:?}: {} files, {:?} bytes, default_selected={}",
                category.category,
                category.file_count,
                category.total_size,
                category.default_selected
            );
        }
        for world in &instance.worlds {
            println!(
                "      world: {:?} (modified={:?})",
                world.folder_name, world.modified
            );
        }
    }

    println!("Settings candidate: {:?}", preview.settings);
    println!("Java versions: {:?}", preview.java_versions);
    println!("Compatibility notes: {:?}", preview.compatibility_notes);
}
