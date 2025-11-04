use std::{
    mem::replace,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

pub(crate) trait AtomicU64Ext {
    /// Adds to the current value, returning the previous value.
    fn inc(&self) -> u64;

    /// Adds to the current value, returning the previous value.
    fn inc_mut(&mut self) -> u64;

    /// Set the value.
    fn set(&self, val: u64);

    /// Set the value.
    fn set_mut(&mut self, val: u64);
}

impl AtomicU64Ext for AtomicU64 {
    fn inc(&self) -> u64 {
        self.fetch_add(1, Relaxed)
    }

    fn inc_mut(&mut self) -> u64 {
        let lru = *self.get_mut() + 1;
        replace(self.get_mut(), lru)
    }

    fn set(&self, val: u64) {
        self.store(val, Relaxed);
    }

    fn set_mut(&mut self, val: u64) {
        *self.get_mut() = val;
    }
}
