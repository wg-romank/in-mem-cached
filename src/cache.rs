use crate::time::Time;

use std::collections::HashMap;
use std::ops::Add;
use std::result::Result;
use std::time::Duration;
use std::time::Instant;

use rand::prelude::*;

pub struct CacheConfig {
    pub ttl: Duration,
    pub capacity: usize,
    pub eviction_number: usize,
    pub eviction_ratio: f32,
}

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
    keys_total: usize,
    cache_config: CacheConfig,
    cache: HashMap<String, CacheEntry>,
    time: &'a T,
}

impl<'a, T: Time> TtlCache<'a, T> {
    pub fn new(cache_config: CacheConfig, t: &'a T) -> TtlCache<'a, T> {
        let capacity = cache_config.capacity;
        TtlCache {
            keys_total: 0,
            cache_config,
            // TODO:
            // we use hash-map here with default hasher
            // since we do not have specific requirements for keys
            // but it is possible to tune performance by switching caching algorithm
            // for short/long keys, see docs https://doc.rust-lang.org/std/collections/struct.HashMap.html
            cache: HashMap::with_capacity(capacity),
            time: &t,
        }
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), String> {
        if self.keys_total < self.cache_config.capacity || self.cache.contains_key(&key) {
            let created = self.time.get_time();
            let new_entry = CacheEntry { value, created };
            if !self.cache.contains_key(&key) {
                self.cache.insert(key.clone(), new_entry);
                self.keys_total += 1;
            } else {
                self.cache.entry(key).and_modify(|v| *v = new_entry);
            }

            Ok(())
        } else {
            Err(format!("out of capacity: {}", self.cache_config.capacity))
        }
    }

    pub fn get(&mut self, key: &String) -> Option<String> {
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

        let mut removed: usize = 0;

        while (removed as f32) / (total_lookup as f32) >= self.cache_config.eviction_ratio {
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
                    .filter(|v| v.is_expired(now, ttl))
                    .is_none()
                {
                    self.cache.remove(&k);
                    removed += 1;
                }
            }
            self.keys_total -= removed;
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use std::time::Instant;
    use std::time::Duration;

    use crate::time::test_time::TestTime;
    use crate::cache::CacheConfig;
    use crate::cache::TtlCache;

    pub fn init_cache<'a>(time: &'a TestTime) -> TtlCache<'a, TestTime> {
        let config = CacheConfig {
            ttl: Duration::from_secs(10),
            capacity: 1,
            eviction_number: 20,
            eviction_ratio: 0.25,
        };

        TtlCache::new(config, time)
    }

    #[test]
    fn items_can_be_set() {
        let time = TestTime::new(Instant::now());
        let mut cache = init_cache(&time);

        let key = String::from("key: String");
        let value = String::from("value: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());

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

        time.add_secs(Duration::from_secs(11));

        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn capacity_is_checked_before_adding_new_items() {
        let time = TestTime::new(Instant::now());
        let mut cache = init_cache(&time);

        let key = String::from("key: String");
        let key2 = String::from("key2: String");
        let value = String::from("value: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());
        assert!(cache.set(key2.clone(), value.clone()).is_err());

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
        assert!(cache.set(key.clone(), value2.clone()).is_ok());

        match cache.get(&key) {
            Some(v) => assert_eq!(v, value2),
            None => assert!(false),
        }
    }
}
