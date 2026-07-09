use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub struct WorkQueue<K, V> {
    working: Arc<Mutex<BTreeMap<K, V>>>,
    complete: Arc<Mutex<BTreeMap<K, V>>>,
}

impl<K, V> WorkQueue<K, V>
where
    K: Ord,
{
    pub fn new() -> Self {
        Self {
            working: Arc::new(Mutex::new(BTreeMap::new())),
            complete: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn complete_list(&self) -> Vec<V>
    where
        V: Clone,
    {
        self.complete
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .cloned()
            .collect()
    }

    pub fn working_list(&self) -> Vec<V>
    where
        V: Clone,
    {
        self.working
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .cloned()
            .collect()
    }

    pub fn complete_size(&self) -> usize {
        self.complete
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }

    pub fn working_size(&self) -> usize {
        self.working
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }

    pub fn size(&self) -> usize {
        self.working_size() + self.complete_size()
    }

    pub fn clear(&self) {
        self.working
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
        self.complete
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }

    pub fn insert(&self, key: K, value: V) {
        let mut working = self
            .working
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if working.contains_key(&key)
            || self
                .complete
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .contains_key(&key)
        {
            panic!("work queue key already exists")
        }
        working.insert(key, value);
    }

    pub fn insert_or_update(&self, key: K, value: V) -> bool {
        let mut working = self
            .working
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let updated = working.contains_key(&key);
        working.insert(key, value);
        updated
    }

    pub fn get(&self, key: K) -> V
    where
        V: Clone,
    {
        if let Some(value) = self
            .working
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&key)
            .cloned()
        {
            return value;
        }
        self.complete
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&key)
            .cloned()
            .unwrap_or_else(|| panic!("work queue key does not exist"))
    }

    pub fn with<R>(&self, key: &K, function: impl FnOnce(&V) -> R) -> Option<R> {
        if let Some(value) = self
            .working
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(key)
        {
            return Some(function(value));
        }
        self.complete
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(key)
            .map(function)
    }

    pub fn with_mut<R>(&self, key: &K, function: impl FnOnce(&mut V) -> R) -> Option<R> {
        if let Some(value) = self
            .working
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(key)
        {
            return Some(function(value));
        }
        self.complete
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(key)
            .map(function)
    }

    pub fn complete(&self, key: K) {
        let value = self
            .working
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&key)
            .unwrap_or_else(|| panic!("work queue key is not working"));
        self.complete
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(key, value);
    }
}
