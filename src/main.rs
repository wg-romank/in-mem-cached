use std::time::Duration;

use tokio::sync::mpsc;

mod cache;
mod service;
mod api;
mod time;

use cache::CacheConfig;
use service::ServiceMessage;
use service::TtlCacheService;
use time::REALTIME;
use api::api;

#[tokio::main]
async fn main() {
    let cache_config = CacheConfig {
        ttl: Duration::from_secs(30 * 60 * 60),
        // todo: unused
        capacity: 10000,
        eviction_number: 20,
        eviction_ratio: 0.25,
    };

    let (tx, rx) = mpsc::unbounded_channel::<ServiceMessage>();
    let mut service = TtlCacheService::new(cache_config, rx, &REALTIME);

    tokio::spawn(async move { service.run().await });

    let routes = api(tx);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
