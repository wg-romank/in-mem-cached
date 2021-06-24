use crate::config::Config;
use crate::time::Time;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::Add;
use std::result::Result;
use std::time::Duration;
use std::time::Instant;

use rand::prelude::*;

struct CacheEntry {
    value: String,
    created: Instant,
}

impl CacheEntry {
    fn is_expired(&self, now: Instant, ttl: Duration) -> bool {
        self.created.add(ttl) < now
    }
}

pub struct TtlCache<'a, T: Time> {
    pub keys_total: usize,
    cache_config: Config,
    cache: HashMap<String, CacheEntry>,
    time: &'a T,
}

impl<'a, T: Time> TtlCache<'a, T> {
    pub fn new(cache_config: Config, t: &'a T) -> TtlCache<'a, T> {
        let capacity = cache_config.capacity;
        TtlCache {
            keys_total: 0,
            cache_config,
            // TODO:
            // we use hash-map here with default hasher since we do not have specific requirements for keys
            // but it is possible to tune performance by switching hashing algorithm
            // for short/long keys, see docs https://doc.rust-lang.org/std/collections/struct.HashMap.html
            cache: capacity
                .map(HashMap::with_capacity)
                .unwrap_or_else(HashMap::new),
            time: &t,
        }
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), String> {
        if self
            .cache_config
            .capacity
            .map(|c| self.keys_total < c)
            .unwrap_or(true)
            || self.cache.contains_key(&key)
        {
            let created = self.time.get_time();
            let new_entry = CacheEntry { value, created };
            match self.cache.entry(key) {
                Entry::Occupied(mut e) => *e.get_mut() = new_entry,
                Entry::Vacant(e) => {
                    self.keys_total += 1;
                    e.insert(new_entry);
                }
            };

            Ok(())
        } else {
            Err(format!("out of capacity: {:?}", self.cache_config.capacity))
        }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        let now = self.time.get_time();
        let ttl = self.cache_config.ttl;

        match self.cache.get(key) {
            Some(e) => {
                if !e.is_expired(now, ttl) {
                    Some(e.value.clone())
                } else {
                    self.cache.remove(key);
                    self.keys_total -= 1;
                    None
                }
            }
            None => None,
        }
    }

    // an attempt to implement simplified version of what Redis has
    // see for reference https://redis.io/commands/expire
    pub fn evict_expired(&mut self) {
        let now = self.time.get_time();
        let ttl = self.cache_config.ttl;
        let total_lookup = self.cache_config.eviction_number;

        loop {
            let mut removed: usize = 0;
            let random_keys: Vec<String> = self
                .cache
                .keys()
                .choose_multiple(&mut rand::thread_rng(), total_lookup)
                .into_iter()
                .cloned()
                .collect();

            for k in random_keys {
                if self
                    .cache
                    .get(&k)
                    .filter(|v| !v.is_expired(now, ttl))
                    .is_none()
                {
                    self.cache.remove(&k);
                    removed += 1;
                }
            }
            self.keys_total -= removed;
            if (removed as f32) / (total_lookup as f32) <= self.cache_config.eviction_ratio {
                break;
            }
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use std::time::Duration;
    use std::time::Instant;

    use crate::cache::TtlCache;
    use crate::config::TEST_CONFIG_SINGLE_ITEM;
    use crate::time::time_fixtures::TestTime;

    pub fn init_cache<'a>(time: &'a TestTime) -> TtlCache<'a, TestTime> {
        TtlCache::new(TEST_CONFIG_SINGLE_ITEM, time)
    }

    #[test]
    fn items_can_be_set() {
        let time = TestTime::new(Instant::now());
        let mut cache = init_cache(&time);

        let key = String::from("key: String");
        let value = String::from("value: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());
        assert_eq!(cache.keys_total, 1);

        match cache.get(&key) {
            Some(v) => assert_eq!(v, value),
            None => assert!(false),
        }
    }

    #[test]
    fn items_expire() {
        let time = TestTime::new(Instant::now());
        let mut cache = init_cache(&time);

        let key = String::from("key: String");
        let value = String::from("value: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());
        assert_eq!(cache.keys_total, 1);

        time.add_secs(Duration::from_secs(11));

        assert!(cache.get(&key).is_none());
        assert_eq!(cache.keys_total, 0);
    }

    #[test]
    fn expired_keys_gets_evicted_with_eviction_call() {
        let time = TestTime::new(Instant::now());
        let mut cache = init_cache(&time);

        let key = String::from("key: String");
        let value = String::from("value: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());
        assert_eq!(cache.keys_total, 1);

        time.add_secs(Duration::from_secs(11));

        cache.evict_expired();
        assert_eq!(cache.keys_total, 0);
    }

    #[test]
    fn capacity_is_checked_before_adding_new_items() {
        let time = TestTime::new(Instant::now());
        let mut cache = init_cache(&time);

        let key = String::from("key: String");
        let key2 = String::from("key2: String");
        let value = String::from("value: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());
        assert_eq!(cache.keys_total, 1);
        assert!(cache.set(key2.clone(), value.clone()).is_err());
        assert_eq!(cache.keys_total, 1);

        match cache.get(&key) {
            Some(v) => assert_eq!(v, value),
            None => assert!(false),
        }
    }

    #[test]
    fn capacity_check_allows_overwrite() {
        let time = TestTime::new(Instant::now());
        let mut cache = init_cache(&time);

        let key = String::from("key: String");
        let value = String::from("value: String");
        let value2 = String::from("value2: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());
        assert_eq!(cache.keys_total, 1);
        assert!(cache.set(key.clone(), value2.clone()).is_ok());
        assert_eq!(cache.keys_total, 1);

        match cache.get(&key) {
            Some(v) => assert_eq!(v, value2),
            None => assert!(false),
        }
    }
}
