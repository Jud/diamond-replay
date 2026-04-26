pub(crate) fn current_index_after_squash(
    current_index: usize,
    size: usize,
    removed_index: usize,
) -> usize {
    let new_size = size.saturating_sub(1);
    if new_size == 0 {
        0
    } else if current_index >= size {
        current_index % new_size
    } else if current_index > removed_index {
        current_index - 1
    } else if current_index >= new_size {
        0
    } else {
        current_index
    }
}
