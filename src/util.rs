/// Generates a Vec of the expected items to be removed from the collection
/// based on their age. The item with the smallest counter are to be expected
/// since there used are the oldest.
pub fn find_lru_item_to_remove<AGE, F, I>(it: I, remove_count: usize, mut f: F) -> Vec<I::Item>
where
    AGE: Ord,
    F: FnMut(&I::Item) -> AGE,
    I: IntoIterator,
{
    if remove_count == 0 {
        return Vec::new();
    }

    let mut iter = it.into_iter();
    let mut page = Vec::with_capacity(remove_count);

    while page.len() < remove_count {
        match iter.next() {
            Some(item) => {
                let age = f(&item);
                page.push((item, age))
            }
            None => {
                // Not enough items — sort and return all
                page.sort_unstable_by(|a, b| a.1.cmp(&b.1));
                return page.into_iter().map(|t| t.0).collect();
            }
        }
    }

    // We have exactly `remove_count` items — sort them
    page.sort_unstable_by(|a, b| a.1.cmp(&b.1));

    // Process remaining items
    for item in iter {
        let age = f(&item);

        if age < page.last().unwrap().1 {
            let index = match page.binary_search_by(|probe| probe.1.cmp(&age)) {
                Ok(idx) | Err(idx) => idx,
            };

            page.pop(); // remove youngest (last) and do not extend the capacity.
            page.insert(index, (item, age)); // insert in sorted position
        }
    }

    page.into_iter().map(|t| t.0).collect()
}
