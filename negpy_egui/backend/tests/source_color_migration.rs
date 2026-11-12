//! Ports tests/test_source_color_migration.py.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::domain::models::{BwFilter, ProcessMode};
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::source_color::migrate_bw_source_color;

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn config(mode: ProcessMode, source_color: Option<bool>, filter: BwFilter) -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = mode;
    config.process.source_color = source_color;
    config.exposure.bw_filter = filter;
    config
}

#[test]
fn clears_source_color_on_a_bw_frame_without_a_filter() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    repo.save_file_settings(
        "h1",
        &config(ProcessMode::BW, Some(true), BwFilter::None),
        "/a.nef",
    )
    .unwrap();

    migrate_bw_source_color(&mut repo);

    assert_eq!(
        repo.load_file_settings("h1")
            .unwrap()
            .unwrap()
            .process
            .source_color,
        None
    );
}

#[test]
fn keeps_source_color_where_it_reaches_the_print() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    repo.save_file_settings(
        "h1",
        &config(ProcessMode::BW, Some(true), BwFilter::Red25),
        "/a.nef",
    )
    .unwrap();
    repo.save_file_settings(
        "h2",
        &config(ProcessMode::C41, Some(true), BwFilter::None),
        "/b.nef",
    )
    .unwrap();

    migrate_bw_source_color(&mut repo);

    assert_eq!(
        repo.load_file_settings("h1")
            .unwrap()
            .unwrap()
            .process
            .source_color,
        Some(true)
    );
    assert_eq!(
        repo.load_file_settings("h2")
            .unwrap()
            .unwrap()
            .process
            .source_color,
        Some(true)
    );
}

#[test]
fn runs_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    migrate_bw_source_color(&mut repo);
    repo.save_file_settings(
        "h1",
        &config(ProcessMode::BW, Some(true), BwFilter::None),
        "/a.nef",
    )
    .unwrap();

    migrate_bw_source_color(&mut repo);

    assert_eq!(
        repo.load_file_settings("h1")
            .unwrap()
            .unwrap()
            .process
            .source_color,
        Some(true)
    );
}
