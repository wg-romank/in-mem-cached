use std::time::Duration;

use warp::Filter;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

mod cache;
mod service;

use cache::CacheConfig;
use service::ServiceMessage;
use service::TtlCacheService;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct KeyValue {
    key: String,
    value: String,
}

async fn read(queue: mpsc::UnboundedSender<ServiceMessage>, key: String) -> Option<String> {
    let (tx, rx)= oneshot::channel::<Option<String>>();

    queue.send(ServiceMessage::Read(key, tx)).ok()?;

    rx.await.ok().flatten()
}

async fn write(queue: mpsc::UnboundedSender<ServiceMessage>, key: String, value: String) -> Result<(), String> {
    let (tx, rx)= oneshot::channel::<Result<(), String>>();

    queue.send(ServiceMessage::Write(key, value, tx))
        .map_err(|e| format!("{}", e))?;

    unimplemented!()
    // rx.await.map_err(|e| format!("{}", e)).flatten()
}

#[tokio::main]
async fn main() {
    let cache_config = CacheConfig {
        ttl: Duration::from_secs(30 * 60 * 60),
        // todo: unused
        capacity: 10000,
        eviction_nuber: 20,
        eviction_ratio: 0.25,
    };

    let (tx, rx) = mpsc::unbounded_channel::<ServiceMessage>();
    let mut service = TtlCacheService::new(cache_config, rx);

    service.run().await;

    let hello = warp::get()
        .and(warp::path("health-check"))
        .map(|| "Ok");

    let set = warp::post()
        .and(warp::path("set"))
        .and(warp::body::json())
        .and_then(|kv: KeyValue| {
            write(tx.clone(), kv.key, kv.value)
        });

    let get = warp::get()
        .and(warp::path("get"))
        .and(warp::path::param::<String>())
        .map(|key: String| {
            todo!()
        });

    let routes = hello.or(get).or(set);

    warp::serve(hello)
        .run(([127, 0, 0, 1], 3030))
        .await;
}