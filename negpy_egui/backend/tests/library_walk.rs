//! Ports `tests/test_library_search.py`.

use std::collections::HashMap;
use std::path::PathBuf;

use negpy_backend::domain::models::{MetadataConfig, WorkspaceConfig};
use negpy_backend::services::assets::library::{
    LibraryWalkCache, iter_library_files, search_library,
};
use negpy_backend::services::assets::search::parse_query;

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap().keep();
        let roll_a = root.join("roll_a");
        let roll_b = root.join("roll_b").join("nested");
        std::fs::create_dir_all(&roll_a).unwrap();
        std::fs::create_dir_all(&roll_b).unwrap();
        std::fs::write(roll_a.join("IMG_0001.NEF"), b"a").unwrap();
        std::fs::write(roll_a.join("IMG_0002.NEF"), b"b").unwrap();
        std::fs::write(roll_b.join("scan.tif"), b"c").unwrap();
        std::fs::write(roll_b.join("scan_ir.tif"), b"d").unwrap();
        std::fs::write(roll_a.join("notes.txt"), b"not an image").unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::write(root.join(".hidden").join("IMG_9999.NEF"), b"e").unwrap();
        Self { root }
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn names(tree: &Tree) -> Vec<String> {
    let mut names: Vec<String> = iter_library_files(&[tree.root.clone()])
        .into_iter()
        .map(|file| file.name)
        .collect();
    names.sort();
    names
}

#[test]
fn the_walk_finds_images_recursively() {
    let tree = Tree::new();
    assert_eq!(
        names(&tree),
        vec!["IMG_0001.NEF", "IMG_0002.NEF", "scan.tif"]
    );
}

#[test]
fn the_walk_carries_stat_data() {
    let tree = Tree::new();
    let entry = iter_library_files(&[tree.root.clone()])
        .into_iter()
        .find(|file| file.name == "IMG_0001.NEF")
        .unwrap();
    assert_eq!(entry.size, 1);
    assert!(entry.mtime > 0.0);
}

#[test]
fn a_nested_root_is_not_walked_twice() {
    let tree = Tree::new();
    let files = iter_library_files(&[tree.root.clone(), tree.root.join("roll_a")]);
    let mut unique: Vec<PathBuf> = files.iter().map(|file| file.path.clone()).collect();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), files.len());
}

#[test]
fn a_missing_root_is_skipped_not_fatal() {
    let tree = Tree::new();
    let mut names: Vec<String> =
        iter_library_files(&[tree.root.join("does_not_exist"), tree.root.join("roll_a")])
            .into_iter()
            .map(|file| file.name)
            .collect();
    names.sort();
    assert_eq!(names, vec!["IMG_0001.NEF", "IMG_0002.NEF"]);
}

#[test]
fn a_search_matches_a_filename_without_any_edits() {
    let tree = Tree::new();
    let files = iter_library_files(&[tree.root.clone()]);
    let hits = search_library(&files, &parse_query("img_0001"), &HashMap::new(), None);
    assert_eq!(
        hits.iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["IMG_0001.NEF"]
    );
}

#[test]
fn a_search_joins_edit_metadata_by_path() {
    let tree = Tree::new();
    let files = iter_library_files(&[tree.root.clone()]);
    let edited = tree.root.join("roll_a").join("IMG_0002.NEF");
    let mut config = WorkspaceConfig::default();
    config.metadata = MetadataConfig {
        film: "Portra 400".to_string(),
        film_iso: Some(400),
        ..MetadataConfig::default()
    };
    let configs = HashMap::from([(edited.to_string_lossy().into_owned(), config)]);
    assert_eq!(
        search_library(&files, &parse_query("film:portra"), &configs, None),
        vec![edited.clone()]
    );
    assert_eq!(
        search_library(&files, &parse_query("iso:>=400"), &configs, None),
        vec![edited]
    );
}

#[test]
fn a_search_resolves_marks_by_path() {
    let tree = Tree::new();
    let files = iter_library_files(&[tree.root.clone()]);
    let keeper = tree.root.join("roll_a").join("IMG_0001.NEF");
    let marks = HashMap::from([(keeper.to_string_lossy().into_owned(), "keeper".to_string())]);
    assert_eq!(
        search_library(
            &files,
            &parse_query("keeper:"),
            &HashMap::new(),
            Some(&marks)
        ),
        vec![keeper]
    );
}

#[test]
fn an_empty_query_returns_nothing_not_the_whole_archive() {
    let tree = Tree::new();
    let files = iter_library_files(&[tree.root.clone()]);
    assert!(search_library(&files, &parse_query(""), &HashMap::new(), None).is_empty());
}

#[test]
fn the_walk_cache_reuses_one_traversal_until_the_roots_change() {
    let tree = Tree::new();
    let mut cache = LibraryWalkCache::new();
    let first = cache.files(&[tree.root.clone()], None).len();
    assert_eq!(cache.files(&[tree.root.clone()], None).len(), first);
    assert!(
        cache
            .files(&[tree.root.join("roll_a")], None)
            .iter()
            .all(|file| file.name.starts_with("IMG"))
    );
}

#[test]
fn the_walk_cache_sees_new_files_after_invalidate() {
    let tree = Tree::new();
    let mut cache = LibraryWalkCache::new();
    let before = cache.files(&[tree.root.clone()], None).len();
    std::fs::write(tree.root.join("roll_a").join("IMG_0003.NEF"), b"f").unwrap();
    assert_eq!(cache.files(&[tree.root.clone()], None).len(), before);
    cache.invalidate();
    assert_eq!(cache.files(&[tree.root.clone()], None).len(), before + 1);
}
