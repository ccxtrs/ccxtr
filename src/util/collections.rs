use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::Hash;

#[derive(PartialEq)]
pub(crate) enum SortedMapOrder {
    Ascending,
    Descending,
}

pub(crate) struct SortedMap<K, V> {
    map: HashMap<K, V>,
    keys: BinaryHeap<K>,
    reverse_keys: BinaryHeap<Reverse<K>>,
    order: SortedMapOrder,
}

impl<K: Ord + Clone + Hash, V: Clone> SortedMap<K, V> {
    pub(crate) fn new(order: SortedMapOrder) -> Self {
        Self {
            map: HashMap::new(),
            keys: BinaryHeap::<K>::new(),
            reverse_keys: BinaryHeap::<Reverse<K>>::new(),
            order,
        }
    }

    pub(crate) fn peek(&self) -> Option<(K, V)> {
        if self.order == SortedMapOrder::Descending {
            self.keys.peek().and_then(|key| {
                self.map.get(key).map(|value| {
                    ((*key).clone(), value.clone())
                })
            })
        } else {
            self.reverse_keys.peek().and_then(|key| {
                self.map.get(&key.0).map(|value| {
                    ((key.0).clone(), value.clone())
                })
            })
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        if self.order == SortedMapOrder::Descending {
            self.keys.push(key.clone());
        } else {
            self.reverse_keys.push(Reverse(key.clone()));
        }
        self.map.insert(key.clone(), value);
    }

    pub(crate) fn remove(&mut self, key: K) {
        self.map.remove(&key);
        self.keys.retain(|x| x != &key);
        self.reverse_keys.retain(|x| x.0 != key);
    }
}