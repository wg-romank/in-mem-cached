use std::collections::HashMap;
use std::ops::Add;
use std::result::Result;
use std::time::Duration;
use std::time::Instant;

use rand::prelude::*;

pub struct CacheConfig {
    pub ttl: Duration,
    pub capacity: usize,
    pub eviction_nuber: usize,
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
    cache_config: CacheConfig,
    cache: HashMap<String, CacheEntry>,
    time: &'a T,
}

impl<'a, T: Time> TtlCache<'a, T> {
    pub fn new(cache_config: CacheConfig, t: &'a T) -> TtlCache<'a, T> {
        let capacity = cache_config.capacity;
        TtlCache {
            cache_config,
            cache: HashMap::with_capacity(capacity),
            time: &t,
        }
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), String> {
        let created = self.time.get_time();
        self.cache
            .insert(key.clone(), CacheEntry { value, created });

        Ok(())
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
                    None
                }
            }
            None => None,
        }
    }

    pub fn evict_expired(&mut self) {
        let now = self.time.get_time();
        let ttl = self.cache_config.ttl;
        let total_lookup = self.cache_config.eviction_nuber;

        let mut removed: f32 = 0.;

        while removed / (total_lookup as f32) >= self.cache_config.eviction_ratio {
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
                    removed += 1.;
                }
            }
        }
    }
}

pub trait Time {
    fn get_time(&self) -> Instant;
}

#[cfg(test)]
mod cache_tests {
    use crate::cache::CacheConfig;
    use crate::cache::Time;
    use crate::cache::TtlCache;

    use std::ops::Add;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use std::time::Instant;

    struct TestTime {
        now: Instant,
        seconds_passed: AtomicUsize,
    }

    impl TestTime {
        fn new(now: Instant) -> TestTime {
            TestTime {
                now,
                seconds_passed: AtomicUsize::new(0),
            }
        }

        fn add_secs(&self, duration: Duration) {
            self.seconds_passed
                .store(duration.as_secs() as usize, Ordering::SeqCst);
        }
    }

    impl Time for TestTime {
        fn get_time(&self) -> Instant {
            self.now.add(Duration::from_secs(
                self.seconds_passed.load(Ordering::SeqCst) as u64,
            ))
        }
    }

    #[test]
    fn ttl() {
        let time = TestTime::new(Instant::now());
        let config = CacheConfig {
            ttl: Duration::from_secs(10),
            capacity: 2,
            eviction_nuber: 20,
            eviction_ratio: 0.25,
        };
        let mut cache = TtlCache::new(config, &time);

        let key = String::from("key: String");
        let value = String::from("value: String");

        assert!(cache.set(key.clone(), value.clone()).is_ok());

        match cache.get(&key) {
            Some(v) => assert_eq!(v, value),
            None => assert!(false),
        }

        time.add_secs(Duration::from_secs(11));

        assert!(cache.get(&key).is_none());
    }
}
