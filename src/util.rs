use paged::PagedIterExt;

/// Generates a Vec of the expected items to be removed from the collection
/// based on their age. The item with the smallest counter are to be expected
/// since there used are the oldest.
pub fn find_lru_item_to_remove<AGE, F, I>(it: I, remove_count: usize, f: F) -> Vec<I::Item>
where
    AGE: Ord,
    F: FnMut(&I::Item) -> AGE,
    I: IntoIterator,
{
    if remove_count == 1 {
        it.into_iter().min_by_key(f).into_iter().collect()
    } else {
        it.into_iter().page_by_key(remove_count, f)
    }
}
