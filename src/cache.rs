use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::result::Result;
use std::time::Duration;
use std::time::Instant;
use std::ops::Add;

pub struct CacheConfig {
    ttl: Duration,
    capacity: u64,
}

struct CacheEntry {
    key: String,
    value: String,
    created: Instant,
}

impl CacheEntry {
    fn is_expired(&self, now: Instant, ttl: Duration) -> bool {
        self.created.add(ttl) < now
    }
}

enum CacheSlot {
    Empty,
    Occupied(CacheEntry),
}

pub struct Cache<'a, T: Time> {
    cache_config: CacheConfig,
    storage: Vec<CacheSlot>,
    time: &'a T,
    num_occupied: u64,
}

pub trait Time {
    fn get_time(&self) -> Instant;
}

impl<'a, T: Time> Cache<'a, T> {
    pub fn new(cache_config: CacheConfig, time: &'a T) -> Cache<'a, T> {
        let mut storage = Vec::with_capacity(cache_config.capacity as usize);
        for _ in 0..cache_config.capacity {
            storage.push(CacheSlot::Empty);
        }
        Cache {
            cache_config,
            storage,
            time,
            num_occupied: 0,
        }
    }

    fn find_idx(&self, key: &String) -> usize {
        let mut hasher = DefaultHasher::new();
        hasher.write(key.as_bytes());
        let hashed = hasher.finish();

        (hashed % self.cache_config.capacity) as usize
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), String> {
        if self.num_occupied < self.cache_config.capacity {
            let created = self.time.get_time();
            let mut idx = self.find_idx(&key);

            while let CacheSlot::Occupied(item) = &self.storage[idx] {
                let overwrite = item.key == key;
                let expired = item.is_expired(created, self.cache_config.ttl);
                match (expired, overwrite) {
                    (true, false) => self.storage[idx] = CacheSlot::Empty,
                    (false, false) => break,
                    (_, true) => break,
                }
                idx = (idx + 1) % self.cache_config.capacity as usize;
            }

            let entry = CacheEntry {
                key,
                value,
                created,
            };
            self.storage[idx] = CacheSlot::Occupied(entry);
            // todo: bug below
            self.num_occupied += 1;
            Ok(())
        } else {
            Err(format!(
                "cache is out of capacity {}/{}",
                self.num_occupied, self.cache_config.capacity
            ))
        }
    }

    pub fn get(&mut self, key: &String) -> Option<String> {
        let mut idx = self.find_idx(&key);
        let now = self.time.get_time();

        let mut result = None;

        while let CacheSlot::Occupied(item) = &self.storage[idx] {
            let found = item.key == *key;
            let expired = item.is_expired(now, self.cache_config.ttl);
            match (expired, found) {
                (true, f) => {
                    self.storage[idx] = CacheSlot::Empty;
                    if f { break };
                }
                (false, false) => (),
                (false, true) => result = Some(item.value.clone()),
            }
            if item.key == *key {
                self.storage[idx] = CacheSlot::Empty;
                break;
            }
            idx = (idx + 1) % self.cache_config.capacity as usize;
        }

        result
    }
}

#[cfg(test)]
mod cache_tests {
    use crate::cache::Cache;
    use crate::cache::CacheConfig;
    use crate::cache::Time;

    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use std::time::Instant;
    use std::ops::Add;

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
    fn simple_operations() {
        let time = TestTime::new(Instant::now());
        let config = CacheConfig {
            ttl: Duration::from_secs(10),
            capacity: 2,
        };
        let mut cache = Cache::new(config, &time);

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
