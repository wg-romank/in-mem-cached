use std::time::Duration;

use tokio::sync::mpsc;

mod api;
mod cache;
mod config;
mod service;
mod time;

use api::make_api;
use config::Config;
use service::ServiceMessage;
use service::TtlCacheService;
use time::REALTIME;

#[tokio::main]
async fn main() {
    let collector = tracing_subscriber::fmt().finish();
    tracing::subscriber::set_global_default(collector).expect("failed to subscribe tracer");

    let cache_config = Config {
        ttl: Duration::from_secs(30 * 60), // 30 minutes
        capacity: None,
        eviction_number: 20,
        eviction_ratio: 0.25,
        eviction_every: Duration::from_millis(250),
    };

    let (tx, rx) = mpsc::unbounded_channel::<ServiceMessage>();
    let mut service = TtlCacheService::new(cache_config, rx, &REALTIME);

    tokio::spawn(async move { service.run().await });

    let routes = make_api(tx);

    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
}
