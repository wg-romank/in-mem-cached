use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::oneshot;
use warp::Filter;

mod cache;
mod service;

use cache::CacheConfig;
use service::ServiceMessage;
use service::TtlCacheService;
use service::REALTIME;

use serde::{Deserialize, Serialize};

use warp::http::status::StatusCode;

#[derive(Serialize, Deserialize)]
struct KeyValue {
    key: String,
    value: String,
}

type ServiceQueue = mpsc::UnboundedSender<ServiceMessage>;

async fn read(
    queue: ServiceQueue,
    key: String,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let (tx, rx) = oneshot::channel::<Option<String>>();

    // todo: fix this mess
    match queue.send(ServiceMessage::Read(key, tx)) {
        Ok(_) => match rx.await {
            Ok(v) => match v {
                Some(vv) => Ok(warp::reply::with_status(vv, StatusCode::OK)),
                None => Ok(warp::reply::with_status(
                    String::from("Not found"),
                    StatusCode::NOT_FOUND,
                )),
            },
            Err(e) => Ok(warp::reply::with_status(
                format!("{}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
            )),
        },
        Err(e) => Ok(warp::reply::with_status(
            format!("{}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn write(queue: ServiceQueue, key: String, value: String) -> Result<impl warp::Reply, std::convert::Infallible> {
    let (tx, rx)= oneshot::channel::<Result<(), String>>();

    match queue.send(ServiceMessage::Write(key, value, tx)) {
        Ok(_) => match rx.await {
            Ok(_) => Ok(warp::reply::with_status(String::new(), StatusCode::OK)),
            Err(e) => Ok(warp::reply::with_status(format!("{}", e), StatusCode::INTERNAL_SERVER_ERROR))
        },
        Err(e) => Ok(warp::reply::with_status(format!("{}", e), StatusCode::INTERNAL_SERVER_ERROR))
    }
}

fn with_cache_tx(
    tx: ServiceQueue,
) -> impl Filter<Extract = (ServiceQueue,), Error = std::convert::Infallible> + Clone {
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
    let mut service = TtlCacheService::new(cache_config, rx, &REALTIME);

    tokio::spawn(async move { service.run().await });

    let hello = warp::get().and(warp::path("health-check")).map(|| "Ok");

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
        .and_then(|key: String, tx: ServiceQueue| async move { read(tx, key).await });

    let routes = hello.or(get);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
