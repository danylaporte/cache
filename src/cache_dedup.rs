use crate::{ArcHashSet, Cache};
use std::{
    borrow::Cow,
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hash},
    ops::RangeInclusive,
    sync::Arc,
};

pub struct CacheDedup<K, V: ?Sized, S = RandomState> {
    cache: Cache<K, Arc<V>, S>,
    groups: ArcHashSet<V, S>,
}

impl<K, V> CacheDedup<K, V, RandomState>
where
    K: Clone + Eq + Hash,
    V: Eq + Hash + ?Sized,
{
    pub fn new() -> Self {
        Self {
            cache: Cache::new(),
            groups: ArcHashSet::new(),
        }
    }

    pub fn with_capacity(capacity: RangeInclusive<usize>) -> Self {
        Self {
            cache: Cache::with_capacity(capacity),
            groups: ArcHashSet::new(),
        }
    }
}

impl<K, V, S> CacheDedup<K, V, S>
where
    K: Clone + Eq + Hash,
    V: Eq + Hash + ?Sized,
    S: BuildHasher + Clone,
{
    pub fn with_capacity_and_hasher(capacity: RangeInclusive<usize>, hash_builder: S) -> Self {
        Self {
            cache: Cache::with_capacity_and_hasher(capacity, hash_builder.clone()),
            groups: ArcHashSet::with_hasher(hash_builder),
        }
    }

    pub fn with_hasher(hash_builder: S) -> Self {
        Self {
            cache: Cache::with_hasher(hash_builder.clone()),
            groups: ArcHashSet::with_hasher(hash_builder),
        }
    }

    pub fn get(&self, key: &K) -> Option<&Arc<V>> {
        self.cache.get(key)
    }

    pub fn get_or_init(&mut self, key: &K, val: Cow<V>) -> &Arc<V>
    where
        V: Clone,
    {
        if !self.cache.contains_key(key) {
            let val = self.groups.get_or_init(val).clone();

            if let Some(v) = self.cache.insert(key.clone(), val) {
                if Arc::strong_count(&v) == 1 {
                    self.groups.remove(&v);
                }
            }
        }

        self.cache.get(&key).unwrap()
    }

    pub fn remove_lru(&mut self, remove_count: usize) {
        self.cache.remove_lru(remove_count);
        self.groups.remove_unreferenced();
    }

    pub fn remove_untouched(&mut self) {
        self.cache.remove_untouched();
        self.groups.remove_unreferenced();
    }

    pub fn remove_untouched_if<F>(&mut self, cond: F)
    where
        F: FnMut(&K, &mut Arc<V>) -> bool,
    {
        self.cache.remove_untouched_if(cond);
        self.groups.remove_unreferenced();
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&K, &mut Arc<V>) -> bool,
    {
        self.cache.retain(f);
        self.groups.remove_unreferenced();
    }

    pub fn set_capacity(&mut self, capacity: RangeInclusive<usize>) {
        self.cache.set_capacity(capacity);
        self.groups.remove_unreferenced();
    }

    pub fn shrink_to_fit(&mut self) {
        self.cache.shrink_to_fit();
        self.groups.shrink_to_fit();
    }
}

impl<K, V, S> Default for CacheDedup<K, V, S>
where
    S: Default,
{
    fn default() -> Self {
        Self {
            cache: Default::default(),
            groups: Default::default(),
        }
    }
}
