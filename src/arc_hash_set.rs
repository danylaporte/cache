use std::{
    borrow::Cow,
    collections::{hash_map::RandomState, HashSet},
    hash::{BuildHasher, Hash},
    sync::Arc,
};

/// Dedup items inserted in the set and returns `Arc`.
pub struct ArcHashSet<T: ?Sized, S = RandomState>(HashSet<Arc<T>, S>);

impl<T: ?Sized> ArcHashSet<T> {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashSet::with_capacity(capacity))
    }
}

impl<T, S> ArcHashSet<T, S>
where
    S: BuildHasher,
    T: Eq + Hash + ?Sized,
{
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        Self(HashSet::with_capacity_and_hasher(capacity, hash_builder))
    }

    pub fn with_hasher(hasher: S) -> Self {
        Self(HashSet::with_hasher(hasher))
    }

    pub fn get(&self, v: &T) -> Option<&Arc<T>> {
        self.0.get(v)
    }

    pub fn get_or_init(&mut self, v: Cow<T>) -> &Arc<T>
    where
        T: Clone,
    {
        if self.0.contains(&*v) {
            self.0.get(&*v).unwrap()
        } else {
            let v = Arc::new(v.into_owned());
            self.0.insert(Arc::clone(&v));
            self.0.get(&*v).unwrap()
        }
    }

    pub fn remove(&mut self, v: &T) {
        self.0.remove(v);
    }

    pub fn remove_if_unused(&mut self, v: &T) {
        if let Some(v) = self.get(v).filter(|a| Arc::strong_count(a) == 1).cloned() {
            self.0.remove(&*v);
        }
    }

    pub fn remove_unreferenced(&mut self) {
        self.0.retain(|v| Arc::strong_count(v) > 1);
    }

    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }
}

impl<T, S> Default for ArcHashSet<T, S>
where
    S: Default,
{
    fn default() -> Self {
        Self(Default::default())
    }
}
