use std::collections::{BinaryHeap, HashMap};
use std::hash::Hash;


pub(crate) enum SortedMapOrder {
    Ascending,
    Descending,
}

pub(crate) struct SortedMap<K, V> {
    map: HashMap<K, V>,
    keys: BinaryHeap<K>,
    order: SortedMapOrder,
}

impl<K: Ord + Clone + Hash, V: Clone> SortedMap<K, V> {
    pub(crate) fn new(order: SortedMapOrder) -> Self {
        Self {
            map: HashMap::new(),
            keys: BinaryHeap::<K>::new(),
            order,
        }
    }

    pub(crate) fn peek(&self) -> Option<(K, V)> {
        let option = self.keys.peek();
        if option.is_none() {
            return None;
        }
        let key = option.unwrap();
        let value = self.map.get(key);
        if value.is_none() {
            return None;
        }
        Some(((*key).clone(), value.unwrap().clone()))
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.map.insert(key.clone(), value);
        self.keys.push(key);
    }
    
    pub(crate) fn remove(&mut self, key: K) {
        self.map.remove(&key);
        self.keys.retain(|x| x != &key);
    }
}