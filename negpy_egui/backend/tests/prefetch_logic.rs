//! Ports `tests/test_prefetch_logic.py`.

use negpy_backend::session::prefetch_logic::{display_neighbor_indices, neighbor_assets};

#[test]
fn neighbors_follow_the_visible_display_order() {
    let files = ["0", "1", "2", "3", "4"];
    let order = [4, 1, 3];
    assert_eq!(display_neighbor_indices(&order, 1), [4, 3]);
    assert_eq!(neighbor_assets(&files, &order, 1), [&"4", &"3"]);
}

#[test]
fn neighbors_at_each_end_of_the_display_order() {
    assert_eq!(display_neighbor_indices(&[0, 1, 2], 0), [1]);
    assert_eq!(display_neighbor_indices(&[0, 1, 2], 1), [0, 2]);
    assert_eq!(display_neighbor_indices(&[0, 1, 2], 2), [1]);
}

#[test]
fn a_missing_selection_has_no_neighbors() {
    assert!(display_neighbor_indices(&[0, 1], 9).is_empty());
    let empty: [&str; 0] = [];
    assert!(neighbor_assets(&empty, &[], usize::MAX).is_empty());
}
