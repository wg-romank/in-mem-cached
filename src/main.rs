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

type ServiceQueue = mpsc::UnboundedSender<ServiceMessage>;

async fn read(queue: ServiceQueue, key: String) -> Result<impl warp::Reply, warp::Rejection> {
    let (tx, rx)= oneshot::channel::<Option<String>>();

    queue.send(ServiceMessage::Read(key, tx))
        .map_err(|_| warp::reject::reject())?;

    match rx.await.ok().flatten() {
        Some(v) => Ok(v),
        None => todo!(),
    }
}

// async fn write(queue: ServiceQueue, key: String, value: String) -> Result<impl warp::Reply, warp::Rejection> {
//     let (tx, rx)= oneshot::channel::<Result<(), String>>();

//     //todo:

//     // queue.send(ServiceMessage::Write(key, value, tx))
//     //     .map_err(|e| Err(e))?;

//     // rx.await.map_err(|e| format!("{}", e)).flatten()
//     unimplemented!()
// }

fn with_cache_tx(tx: ServiceQueue) -> impl Filter<Extract = (ServiceQueue,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || tx.clone())
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

    tokio::spawn(async move {
        service.run().await
    });

    let hello = warp::get()
        .and(warp::path("health-check"))
        .map(|| "Ok");

    // let set = warp::post()
    //     .and(warp::path("set"))
    //     .and(warp::body::json())
    //     .and_then(|kv: KeyValue, tx: ServiceQueue| async move {
    //         write(tx.clone(), kv.key, kv.value)
    //     });

    let get = warp::get()
        .and(warp::path("get"))
        .and(warp::path::param::<String>())
        .and(with_cache_tx(tx.clone()))
        .and_then(|key: String, tx: ServiceQueue|  async move {
            read(tx.clone(), key).await
        });

    let routes = hello.or(get);

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}