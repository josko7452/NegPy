//! Neighbors in the Film Strip display order, for preview prefetching.
//!
//! Mirrors `negpy/desktop/prefetch_logic.py`.

/// The asset indices immediately before and after `current_index` in `display_order`.
#[must_use]
pub fn display_neighbor_indices(display_order: &[usize], current_index: usize) -> Vec<usize> {
    let Some(position) = display_order
        .iter()
        .position(|index| *index == current_index)
    else {
        return Vec::new();
    };
    let mut neighbors = Vec::with_capacity(2);
    if position > 0 {
        neighbors.push(display_order[position - 1]);
    }
    if position + 1 < display_order.len() {
        neighbors.push(display_order[position + 1]);
    }
    neighbors
}

/// The previous and next assets in Film Strip order.
#[must_use]
pub fn neighbor_assets<'a, T>(
    files: &'a [T],
    display_order: &[usize],
    current_index: usize,
) -> Vec<&'a T> {
    display_neighbor_indices(display_order, current_index)
        .into_iter()
        .filter_map(|index| files.get(index))
        .collect()
}
