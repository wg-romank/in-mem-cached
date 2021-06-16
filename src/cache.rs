use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::result::Result;
use std::time::Duration;
use std::time::Instant;

struct CacheConfig {
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
        now - self.created < ttl
    }
}

enum CacheSlot {
    Empty,
    Occupied(CacheEntry),
}

struct Cache<T: Time> {
    cache_config: CacheConfig,
    storage: Vec<CacheSlot>,
    time: T,
    num_occupied: u64,
}

trait Time {
    fn get_time(&self) -> Instant;
}

impl<T: Time> Cache<T> {
    pub fn new(cache_config: CacheConfig, time: T) -> Cache<T> {
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

        let mut idx = (hashed % self.cache_config.capacity) as usize;

        idx
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), String> {
        if self.num_occupied < self.cache_config.capacity {
            let created = self.time.get_time();
            let mut idx = self.find_idx(&key);

            while let CacheSlot::Occupied(item) = &self.storage[idx] {
                if item.key == key || item.is_expired(created, self.cache_config.ttl) {
                    break
                } else {
                    idx = idx + 1
                }
            }

            let entry = CacheEntry { key, value, created };
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

    pub fn get(&self, key: String) -> Option<String> {
        let idx = self.find_idx(&key);
        let now = self.time.get_time();

        let mut result = None;

        while let CacheSlot::Occupied(item) = &self.storage[idx] {
            if item.key == key || !item.is_expired(now, self.cache_config.ttl) {
                result = Some(item.value.clone());
                break;
            }
        };

        result
    }
}

struct TestTime {
    now: Instant
}

impl TestTime {
    fn new(now: Instant) -> TestTime { TestTime { now } }

    fn add(&mut self, duration: Duration) {
        match self.now.checked_add(duration) {
            Some(new_now) => self.now = new_now,
            None => panic!("failed to add {:#?}", duration)
        };
    }
}

impl Time for TestTime {
    fn get_time(&self) -> Instant { self.now }
}

#[test]
fn simple_operations() {
    let mut time = TestTime::new(Instant::now());
    let config = CacheConfig { ttl: Duration::from_millis(10), capacity: 2 };
    let mut cache = Cache::new(config, time);

    let key = String::from("key: String");
    let value = String::from("value: String");

    assert!(cache.set(key.clone(), value.clone()).is_ok());

    match cache.get(key) {
        Some(v) => assert_eq!(v, value),
        None => assert!(false),
    }

    // todo: typeclasses
    // time.add(Duration::from_millis(11));
}