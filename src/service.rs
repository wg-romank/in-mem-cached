use std::time::Instant;
use crate::config::Config;
use crate::time::Time;
use crate::cache::TtlCache;

use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub enum ServiceMessage {
    Read(String, oneshot::Sender<Option<String>>),
    Write(String, String, oneshot::Sender<Result<(), String>>),
}

pub type ServiceQueue = mpsc::UnboundedSender<ServiceMessage>;

pub struct TtlCacheService<'a, T: Time> {
    config: Config,
    queue: mpsc::UnboundedReceiver<ServiceMessage>,
    ttl_cache: TtlCache<'a, T>,
    last_eviction_ran: Instant,
    time: &'a T,
}

impl<'a, T: Time> TtlCacheService<'a, T> {
    pub fn new(
        cache_config: Config,
        queue: mpsc::UnboundedReceiver<ServiceMessage>,
        time: &'a T,
    ) -> TtlCacheService<'a, T> {
        TtlCacheService {
            config: cache_config.clone(),
            queue: queue,
            ttl_cache: TtlCache::new(cache_config, time),
            last_eviction_ran: time.get_time(),
            time: time,
        }
    }

    pub async fn run(&mut self) {
        loop {
            if self.last_eviction_ran.elapsed() > self.config.eviction_every {
                self.ttl_cache.evict_expired();
                self.last_eviction_ran = self.time.get_time();
            }
            // todo: timeout is still needed
            if let Some(msg) = self.queue.recv().await {
                match msg {
                    ServiceMessage::Read(key, sender) => {
                        let key = self.ttl_cache.get(&key);
                        sender.send(key);
                    }
                    ServiceMessage::Write(key, value, sender) => {
                        let result = self.ttl_cache.set(key, value);
                        sender.send(result);
                    }
                }
            } else {
                break
            }
        }
    }
}
