use async_cell_lock::AsyncOnceCell;
use std::{
    future::Future,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

pub struct CacheIsland<T>(AsyncOnceCell<CacheIslandInternal<T>>);

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

impl<T> PartialEq for CacheIslandInternal<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        &self.value == &other.value
    }
}

impl<T> CacheIsland<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_value(value: T) -> Self {
        Self(AsyncOnceCell::with_val(CacheIslandInternal::with_value(
            value,
        )))
    }

    pub fn clear(&mut self) {
        self.0.take();
    }

    pub fn clear_if_untouched_since(&mut self, age: u64) {
        if self.0.get_mut().map_or(false, |v| *v.age.get_mut() <= age) {
            self.0.take();
        }
    }

    pub fn get(&self) -> Option<&T> {
        match self.0.get() {
            Some(v) => {
                v.age.store(CACHE_ISLAND_AGE.fetch_add(1, Relaxed), Relaxed);
                Some(&v.value)
            }
            None => None,
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self.0.get_mut() {
            Some(v) => {
                v.age.store(CACHE_ISLAND_AGE.fetch_add(1, Relaxed), Relaxed);
                Some(&mut v.value)
            }
            None => None,
        }
    }

    pub async fn get_or_try_init_async<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        let v = self
            .0
            .get_or_try_init(async { Ok(CacheIslandInternal::with_value(f.await?)) })
            .await?;

        Ok(&v.value)
    }

    pub fn replace(&mut self, value: T) -> Option<T> {
        self.0
            .swap(Some(CacheIslandInternal::with_value(value)))
            .map(|v| v.value)
    }
}

impl<T> Clone for CacheIsland<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self(match self.0.get() {
            Some(v) => AsyncOnceCell::with_val(CacheIslandInternal::with_value(v.value.clone())),
            None => AsyncOnceCell::new(),
        })
    }
}

impl<T> Default for CacheIsland<T> {
    fn default() -> Self {
        Self(AsyncOnceCell::default())
    }
}

impl<T> PartialEq for CacheIsland<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}

static CACHE_ISLAND_AGE: AtomicU64 = AtomicU64::new(0);

pub fn current_cache_island_age() -> u64 {
    CACHE_ISLAND_AGE.load(Relaxed)
}
