use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub struct WorkQueue<K, V> {
    state: Arc<Mutex<WorkQueueState<K, V>>>,
}

impl<K, V> WorkQueue<K, V>
where
    K: Ord + Clone,
{
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(WorkQueueState {
                order: Vec::new(),
                working: BTreeMap::new(),
                complete: BTreeMap::new(),
            })),
        }
    }

    pub fn complete_list(&self) -> Vec<V>
    where
        V: Clone,
    {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state
            .order
            .iter()
            .filter_map(|key| state.complete.get(key).cloned())
            .collect()
    }

    pub fn list(&self) -> Vec<V>
    where
        V: Clone,
    {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state
            .order
            .iter()
            .map(|key| {
                state
                    .working
                    .get(key)
                    .or_else(|| state.complete.get(key))
                    .unwrap_or_else(|| {
                        panic!("work queue order key does not exist")
                    })
                    .clone()
            })
            .collect()
    }

    pub fn entries(&self) -> Vec<(K, V)>
    where
        V: Clone,
    {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state
            .order
            .iter()
            .map(|key| {
                let value = state
                    .working
                    .get(key)
                    .or_else(|| state.complete.get(key))
                    .unwrap_or_else(|| {
                        panic!("work queue order key does not exist")
                    })
                    .clone();
                (key.clone(), value)
            })
            .collect()
    }

    pub fn working_list(&self) -> Vec<V>
    where
        V: Clone,
    {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state
            .order
            .iter()
            .filter_map(|key| state.working.get(key).cloned())
            .collect()
    }

    pub fn complete_size(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .complete
            .len()
    }

    pub fn working_size(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .working
            .len()
    }

    pub fn size(&self) -> usize {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.working.len() + state.complete.len()
    }

    pub fn clear(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.order.clear();
        state.working.clear();
        state.complete.clear();
    }

    pub fn insert(&self, key: K, value: V) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.working.contains_key(&key)
            || state.complete.contains_key(&key)
        {
            panic!("work queue key already exists")
        }
        state.order.push(key.clone());
        state.working.insert(key, value);
    }

    pub fn insert_or_update(&self, key: K, value: V) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(stored) = state.working.get_mut(&key) {
            *stored = value;
            return true;
        }
        if let Some(stored) = state.complete.get_mut(&key) {
            *stored = value;
            return true;
        }
        state.order.push(key.clone());
        state.working.insert(key, value);
        false
    }

    pub fn get(&self, key: K) -> V
    where
        V: Clone,
    {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(value) = state.working.get(&key).cloned() {
            return value;
        }
        state
            .complete
            .get(&key)
            .cloned()
            .unwrap_or_else(|| panic!("work queue key does not exist"))
    }

    pub fn with<R>(&self, key: &K, function: impl FnOnce(&V) -> R) -> Option<R> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(value) = state.working.get(key) {
            return Some(function(value));
        }
        state.complete.get(key).map(function)
    }

    pub fn with_mut<R>(&self, key: &K, function: impl FnOnce(&mut V) -> R) -> Option<R> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(value) = state.working.get_mut(key) {
            return Some(function(value));
        }
        state.complete.get_mut(key).map(function)
    }

    pub fn complete(&self, key: K) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let value = state
            .working
            .remove(&key)
            .unwrap_or_else(|| panic!("work queue key is not working"));
        state.complete.insert(key, value);
    }
}

// -- Private -- //

struct WorkQueueState<K, V> {
    order: Vec<K>,
    working: BTreeMap<K, V>,
    complete: BTreeMap<K, V>,
}
