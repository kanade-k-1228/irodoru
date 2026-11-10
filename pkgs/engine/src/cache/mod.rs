use crate::model::Condition;
pub(crate) mod voices;
use std::collections::VecDeque;

pub(crate) trait CacheKey {
    fn byte_size(&self) -> usize;
}
impl CacheKey for String {
    fn byte_size(&self) -> usize {
        size_of::<Self>() + self.capacity()
    }
}
impl CacheKey for [u8; 32] {
    fn byte_size(&self) -> usize {
        size_of::<Self>()
    }
}

pub(crate) struct ConditionCache<K> {
    pub(crate) entries: VecDeque<(K, Condition)>,
    bytes: usize,
    max_bytes: usize,
    max_entries: usize,
}
impl<K: PartialEq + CacheKey> ConditionCache<K> {
    pub(crate) fn new(max_bytes: usize, max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            max_bytes,
            max_entries,
        }
    }
    pub(crate) fn get(&mut self, key: &K) -> Option<Condition> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let result = entry.1.clone();
        self.entries.push_back(entry);
        Some(result)
    }
    pub(crate) fn insert(&mut self, key: K, value: Condition) {
        let size = value.byte_size() + key.byte_size();
        if let Some(index) = self.entries.iter().position(|(k, _)| k == &key) {
            let (key, value) = self.entries.remove(index).unwrap();
            self.bytes -= key.byte_size() + value.byte_size();
        }
        if size > self.max_bytes || self.max_entries == 0 {
            return;
        }
        while self.bytes + size > self.max_bytes || self.entries.len() >= self.max_entries {
            let (key, value) = self.entries.pop_front().unwrap();
            self.bytes -= key.byte_size() + value.byte_size();
        }
        self.entries.push_back((key, value));
        self.bytes += size;
    }
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}
