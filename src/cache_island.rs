use async_cell_lock::AsyncOnceCell;
use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

pub struct CacheIsland<T>(AsyncOnceCell<CacheIslandInternal<T>>);

struct CacheIslandInternal<T> {
    touched: AtomicBool,
    value: T,
}

impl<T> CacheIslandInternal<T> {
    fn with_value(value: T) -> Self {
        Self {
            touched: AtomicBool::new(true),
            value,
        }
    }
}

impl<T> PartialEq for CacheIslandInternal<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
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

    pub fn get(&self) -> Option<&T> {
        match self.0.get() {
            Some(v) => {
                v.touched.store(true, Relaxed);
                Some(&v.value)
            }
            None => None,
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self.0.get_mut() {
            Some(v) => {
                v.touched.store(true, Relaxed);
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

        v.touched.store(true, Relaxed);
        Ok(&v.value)
    }

    pub fn replace(&mut self, value: T) -> Option<T> {
        self.0
            .swap(Some(CacheIslandInternal::with_value(value)))
            .map(|v| v.value)
    }

    pub fn take(&mut self) -> Option<T> {
        self.0.take().map(|inner| inner.value)
    }

    /// Make the value untouched and returns true if the value was touched.
    pub fn untouch(&self) -> bool {
        self.0
            .get()
            .map_or(false, |v| v.touched.swap(false, Relaxed))
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
