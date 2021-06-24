use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    pub ttl: Duration,
    pub capacity: Option<usize>,
    pub eviction_number: usize,
    pub eviction_ratio: f32,
    pub eviction_every: Duration,
}

#[cfg(test)]
pub const TEST_CONFIG_SINGLE_ITEM: Config = Config {
    ttl: Duration::from_secs(10),
    capacity: Some(1),
    eviction_number: 20,
    eviction_ratio: 0.25,
    eviction_every: Duration::from_millis(250),
};
