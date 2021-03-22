use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub struct CacheIsland<T>(Option<CacheIslandInternal<T>>);

struct CacheIslandInternal<T> {
    age: AtomicU64,
    value: T,
}

impl<T> CacheIslandInternal<T> {
    fn with_value(value: T) -> Self {
        Self {
            age: AtomicU64::new(CACHE_ISLAND_AGE.fetch_add(1, Relaxed)),
            value,
        }
    }
}

impl<T> CacheIsland<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_value(value: T) -> Self {
        Self(Some(CacheIslandInternal::with_value(value)))
    }

    pub fn clear(&mut self) {
        self.0 = None;
    }

    pub fn clear_if_untouched_since(&mut self, age: u64) {
        if self.0.as_mut().map_or(false, |v| *v.age.get_mut() <= age) {
            self.0 = None;
        }
    }

    pub fn get(&self) -> Option<&T> {
        match self.0.as_ref() {
            Some(v) => {
                v.age.store(CACHE_ISLAND_AGE.fetch_add(1, Relaxed), Relaxed);
                Some(&v.value)
            }
            None => None,
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self.0.as_mut() {
            Some(v) => {
                v.age.store(CACHE_ISLAND_AGE.fetch_add(1, Relaxed), Relaxed);
                Some(&mut v.value)
            }
            None => None,
        }
    }

    pub fn set(&mut self, value: T) {
        self.0 = Some(CacheIslandInternal::with_value(value))
    }
}

impl<T> Clone for CacheIsland<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self(match self.0.as_ref() {
            Some(v) => Some(CacheIslandInternal::with_value(v.value.clone())),
            None => None,
        })
    }
}

impl<T> Default for CacheIsland<T> {
    fn default() -> Self {
        Self(None)
    }
}

static CACHE_ISLAND_AGE: AtomicU64 = AtomicU64::new(0);
