mod atomic_ext;
mod cache_island;
mod util;

use crate::atomic_ext::AtomicU64Ext;
use std::{
    borrow::Borrow,
    cmp::{max, min},
    collections::hash_map::{self, HashMap, RandomState},
    fmt::{self, Debug},
    hash::{BuildHasher, Hash},
    mem::replace,
    ops::RangeInclusive,
    sync::atomic::AtomicU64,
};

pub use cache_island::CacheIsland;
pub use util::find_lru_item_to_remove;

pub struct Cache<K, V, S = RandomState> {
    capacity: RangeInclusive<usize>,
    lru: AtomicU64,
    map: HashMap<K, Rec<V>, S>,
    remove_touched: u64,
}

impl<K, V> Cache<K, V, RandomState> {
    pub fn new() -> Self {
        Self::with_capacity(usize::MAX..=usize::MAX)
    }

    pub fn with_capacity(capacity: RangeInclusive<usize>) -> Self {
        Self::with_capacity_and_hasher(capacity, Default::default())
    }
}

impl<K, V, S> Cache<K, V, S> {
    pub fn with_capacity_and_hasher(capacity: RangeInclusive<usize>, hash_builder: S) -> Self {
        Self {
            capacity,
            lru: AtomicU64::new(0),
            map: HashMap::with_hasher(hash_builder),
            remove_touched: 0,
        }
    }

    pub fn with_hasher(hash_builder: S) -> Self {
        Self::with_capacity_and_hasher(usize::MAX..=usize::MAX, hash_builder)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            it: self.map.iter(),
            lru: &self.lru,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            it: self.map.iter_mut(),
            lru: &self.lru,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn values(&self) -> Values<K, V> {
        Values {
            it: self.map.values(),
            lru: &self.lru,
        }
    }

    pub fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut {
            it: self.map.values_mut(),
            lru: &self.lru,
        }
    }
}

impl<K, V, S> Cache<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.map.contains_key(key)
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        match self.map.get(key) {
            Some(rec) => {
                rec.lru.set(self.lru.inc());
                Some(&rec.val)
            }
            None => None,
        }
    }

    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        match self.map.get_mut(key) {
            Some(rec) => {
                rec.lru.set_mut(self.lru.inc_mut());
                Some(&mut rec.val)
            }
            None => None,
        }
    }

    /// Insert an item in the cache
    ///
    /// Examples
    ///
    /// ```
    /// use cache::Cache;
    ///
    /// let mut cache = Cache::new();
    /// cache.insert(0, 0);
    /// assert_eq!(cache.get(&0), Some(&0));
    /// assert_eq!(cache.get(&1), None);
    /// ```
    pub fn insert(&mut self, key: K, val: V) -> Option<V>
    where
        K: Clone,
    {
        let lru = self.lru.inc_mut();

        if let Some(item) = self.map.get_mut(&key) {
            item.lru.set_mut(lru);
            return Some(replace(&mut item.val, val));
        }

        self.optimize_capacity();

        let lru = AtomicU64::new(lru);
        let rec = Rec { lru, val };

        self.map.insert(key, rec);

        None
    }

    fn optimize_capacity(&mut self)
    where
        K: Clone,
    {
        let end = *self.capacity.end();

        if self.map.len() >= end {
            let end = max(self.map.len(), end);
            let start = *self.capacity.start();
            let remove_count = max(end.saturating_sub(start), 1);

            self.remove_lru(remove_count);
        }
    }

    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.map.remove(key).map(|r| r.val)
    }

    pub fn remove_lru(&mut self, remove_count: usize)
    where
        K: Clone,
    {
        let remove_count = min(self.map.len(), remove_count);

        let it = self
            .map
            .iter_mut()
            .map(|(k, rec)| (k.clone(), *rec.lru.get_mut()));

        let page = find_lru_item_to_remove(it, remove_count, |(_, lru)| *lru);

        for (k, _) in &page {
            self.map.remove(k);
        }
    }

    /// Removes all items that have not been accessed since the last call of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use cache::Cache;
    ///
    /// let mut cache = Cache::new();
    /// cache.insert(1, 1); // value 1 touched.
    /// cache.insert(2, 2); // value 2 touched.
    /// cache.remove_untouched();
    ///
    /// assert_eq!(cache.get(&1), Some(&1)); // value 1 touched.
    ///
    /// cache.remove_untouched();
    /// assert_eq!(cache.get(&2), None); // value 2 is now removed.
    /// assert_eq!(cache.get(&1), Some(&1)); // value 1 is still there.
    /// ```
    pub fn remove_untouched(&mut self) {
        self.remove_untouched_if(|_, _| true)
    }

    /// Removes all items that have not been accessed since the last call
    /// of this method an that match the if cond.
    ///
    /// # Examples
    ///
    /// ```
    /// use cache::Cache;
    ///
    /// let mut cache = Cache::new();
    /// cache.insert(1, 1); // value 1 touched.
    /// cache.insert(2, 2); // value 2 touched.
    /// cache.remove_untouched();
    ///
    /// cache.remove_untouched_if(|_k, v| *v > 1);
    /// assert_eq!(cache.get(&2), None); // value 2 is now removed.
    /// assert_eq!(cache.get(&1), Some(&1)); // value 1 is still there.
    /// ```
    pub fn remove_untouched_if<F>(&mut self, mut cond: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        let lru = *self.lru.get_mut();
        let remove_touched = self.remove_touched;

        self.map
            .retain(|k, r| *r.lru.get_mut() >= remove_touched || !cond(k, &mut r.val));

        self.remove_touched = lru;
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.map.retain(|k, r| f(k, &mut r.val));
    }

    /// Changes the capacity causing removal of items that
    /// do not conform the new capacity.
    ///
    /// A range allow batch remove of items.
    /// `2..=5` 5 allow a maximum of 5 items. When the maximum is reached, the cache will
    /// keep only 2 most recently used items.
    /// Examples
    ///
    /// ```
    /// use cache::Cache;
    ///
    /// let mut cache = Cache::new();   // no capacity limit
    /// cache.insert(0, 0);
    /// cache.insert(1, 1);
    /// cache.insert(2, 2);
    ///
    /// cache.set_capacity(1..=2);
    /// assert!(cache.get(&0).is_none());
    /// assert!(cache.get(&1).is_none());
    /// assert!(cache.get(&2).is_some());
    /// ```
    pub fn set_capacity(&mut self, capacity: RangeInclusive<usize>)
    where
        K: Clone,
    {
        self.capacity = capacity;
        self.optimize_capacity();
    }

    /// Shrinks the capacity of the cache as much as possible.
    /// It will drop down as much as possible while maintaining the internal rules and possibly
    /// leaving some space in accordance with the resize policy.
    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }
}

impl<K, V, S> Debug for Cache<K, V, S>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.iter().map(|i| (i.key(), i.value())))
            .finish()
    }
}

impl<K, V, S: Default> Default for Cache<K, V, S> {
    fn default() -> Self {
        Self::with_hasher(S::default())
    }
}

impl<'a, K, V, S> IntoIterator for &'a Cache<K, V, S> {
    type Item = CacheItem<'a, K, V>;
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, S> IntoIterator for &'a mut Cache<K, V, S> {
    type Item = CacheItemMut<'a, K, V>;
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

pub struct CacheItem<'a, K, V> {
    item: (&'a K, &'a Rec<V>),
    lru: &'a AtomicU64,
}

impl<'a, K, V> CacheItem<'a, K, V> {
    pub fn key(&self) -> &'a K {
        self.item.0
    }

    pub fn touch_value(&self) -> &'a V {
        self.item.1.lru.set(self.lru.inc());
        &self.item.1.val
    }

    pub fn value(&self) -> &'a V {
        &self.item.1.val
    }
}

impl<'a, K, V> Clone for CacheItem<'a, K, V> {
    fn clone(&self) -> Self {
        Self {
            item: self.item,
            lru: self.lru,
        }
    }
}

pub struct CacheItemMut<'a, K, V> {
    item: (&'a K, &'a mut Rec<V>),
    lru: &'a AtomicU64,
}

impl<'a, K, V> CacheItemMut<'a, K, V> {
    pub fn key(&self) -> &'a K {
        self.item.0
    }

    pub fn touch_value(&mut self) -> &mut V {
        self.item.1.lru.set(self.lru.inc());
        &mut self.item.1.val
    }

    pub fn value(&mut self) -> &mut V {
        &mut self.item.1.val
    }
}

pub struct CacheValue<'a, V> {
    lru: &'a AtomicU64,
    rec: &'a Rec<V>,
}

impl<'a, V> CacheValue<'a, V> {
    pub fn touch_value(&self) -> &'a V {
        self.rec.lru.set(self.lru.inc());
        self.value()
    }

    pub fn value(&self) -> &'a V {
        &self.rec.val
    }
}

impl<'a, V> Clone for CacheValue<'a, V> {
    fn clone(&self) -> Self {
        Self {
            lru: self.lru,
            rec: self.rec,
        }
    }
}

pub struct CacheValueMut<'a, V> {
    lru: &'a AtomicU64,
    rec: &'a mut Rec<V>,
}

impl<'a, V> CacheValueMut<'a, V> {
    pub fn touch_value(&mut self) -> &mut V {
        self.rec.lru.set_mut(self.lru.inc());
        self.value()
    }

    pub fn value(&mut self) -> &mut V {
        &mut self.rec.val
    }
}

pub struct Iter<'a, K, V> {
    it: hash_map::Iter<'a, K, Rec<V>>,
    lru: &'a AtomicU64,
}

impl<'a, K, V> Clone for Iter<'a, K, V> {
    fn clone(&self) -> Self {
        Self {
            it: self.it.clone(),
            lru: self.lru,
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = CacheItem<'a, K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(CacheItem {
            item: self.it.next()?,
            lru: self.lru,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.it.count()
    }
}

pub struct IterMut<'a, K, V> {
    it: hash_map::IterMut<'a, K, Rec<V>>,
    lru: &'a AtomicU64,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = CacheItemMut<'a, K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(CacheItemMut {
            item: self.it.next()?,
            lru: self.lru,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.it.count()
    }
}

struct Rec<V> {
    lru: AtomicU64,
    val: V,
}

pub struct Values<'a, K, V> {
    it: hash_map::Values<'a, K, Rec<V>>,
    lru: &'a AtomicU64,
}

impl<'a, K, V> Clone for Values<'a, K, V> {
    fn clone(&self) -> Self {
        Self {
            it: self.it.clone(),
            lru: self.lru,
        }
    }
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = CacheValue<'a, V>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(CacheValue {
            lru: self.lru,
            rec: self.it.next()?,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.it.count()
    }
}

pub struct ValuesMut<'a, K, V> {
    it: hash_map::ValuesMut<'a, K, Rec<V>>,
    lru: &'a AtomicU64,
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = CacheValueMut<'a, V>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(CacheValueMut {
            lru: self.lru,
            rec: self.it.next()?,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.it.count()
    }
}

#[test]
fn it_works() {
    let mut map = Cache::with_capacity(2..=3);

    map.insert(0, 0);
    map.insert(1, 1);
    map.insert(2, 2);

    map.insert(3, 3);

    let mut actual = map.values().map(|v| *v.value()).collect::<Vec<_>>();

    actual.sort_unstable();

    assert_eq!(actual, vec![1, 2, 3]);
}
