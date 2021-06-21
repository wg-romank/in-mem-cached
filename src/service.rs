use crate::config::Config;
use crate::time::Time;
use crate::cache::TtlCache;

use std::time::Instant;

use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::instrument;

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

    #[instrument(skip(self))]
    pub async fn run(&mut self) {
        loop {
            if self.last_eviction_ran.elapsed() > self.config.eviction_every {
                self.ttl_cache.evict_expired();
                self.last_eviction_ran = self.time.get_time();
            }
            // todo: future is blocked on the queue here
            // so we won't be expiring stuff in case service is idling
            if let Some(msg) = self.queue.recv().await {
                match msg {
                    ServiceMessage::Read(key, cb) => {
                        let key = self.ttl_cache.get(&key);
                        let _ = cb.send(key)
                            .map_err(|e| tracing::error!("[read] failed sending callback: {:?}", e));
                    }
                    ServiceMessage::Write(key, value, cb) => {
                        let result = self.ttl_cache.set(key, value);
                        let _ = cb.send(result)
                            .map_err(|e| tracing::error!("[write] failed sending callback: {:?}", e));
                    }
                }
            } else {
                break
            }
        }
    }
}
