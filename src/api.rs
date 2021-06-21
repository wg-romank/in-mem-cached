use crate::service::ServiceMessage;
use crate::service::ServiceQueue;

use warp::http::status::StatusCode;
use warp::Filter;

use tokio::sync::mpsc;
use tokio::sync::oneshot;

async fn read(
    queue: ServiceQueue,
    key: String,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let (tx, rx) = oneshot::channel::<Option<String>>();

    // todo: warp does not allow any types apart from Infallible and Rejection
    // thus it is a big ugly instead of using much more ergonomic '?' op
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

async fn write(
    queue: ServiceQueue,
    key: String,
    value: warp::hyper::body::Bytes,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let (tx, rx) = oneshot::channel::<Result<(), String>>();

    match String::from_utf8(Vec::from_iter(value.into_iter())) {
        Ok(v) => match queue.send(ServiceMessage::Write(key, v, tx)) {
            Ok(_) => match rx.await {
                Ok(res) => match res {
                    Ok(_) => Ok(warp::reply::with_status(String::new(), StatusCode::OK)),
                    Err(e) => Ok(warp::reply::with_status(e, StatusCode::BAD_REQUEST)),
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
        },

        Err(e) => Ok(warp::reply::with_status(
            format!("Could not decode utf-8: {}", e),
            StatusCode::BAD_REQUEST,
        )),
    }
}

fn with_cache_tx(
    tx: ServiceQueue,
) -> impl Filter<Extract = (ServiceQueue,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || tx.clone())
}

use std::iter::FromIterator;

pub fn make_api(
    tx: mpsc::UnboundedSender<ServiceMessage>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let hello = warp::get().and(warp::path("health-check")).map(|| "Ok");

    let set = warp::post()
        .and(warp::path("set"))
        .and(warp::path::param::<String>())
        .and(warp::body::bytes())
        .and(with_cache_tx(tx.clone()))
        .and_then(
            |key: String, value: warp::hyper::body::Bytes, tx: ServiceQueue| async move {
                write(tx.clone(), key, value).await
            },
        );

    let get = warp::get()
        .and(warp::path("get"))
        .and(warp::path::param::<String>())
        .and(with_cache_tx(tx.clone()))
        .and_then(|key: String, tx: ServiceQueue| async move { read(tx, key).await });

    hello.or(get).or(set)
}

#[cfg(test)]
mod api_tests {
    use crate::time::Time;
    use crate::api::make_api;
    use crate::config::TEST_CONFIG_SINGLE_ITEM;
    use crate::time::time_fixtures::TestTime;
    use crate::service::ServiceMessage;
    use crate::service::TtlCacheService;

    use std::time::Duration;
    use std::time::Instant;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use tokio::sync::mpsc;
    use warp::Filter;

    impl Time for Arc<Mutex<TestTime>> {
        fn get_time(&self) -> std::time::Instant {
            loop {
                if let Ok(inner) = self.try_lock() {
                    break inner.get_time()
                }
            }
        }
    }

    fn init() -> (
        Arc<Mutex<TestTime>>,
        impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone,
    ) {
        let (tx, rx) = mpsc::unbounded_channel::<ServiceMessage>();

        let time = Arc::new(Mutex::new(TestTime::new(Instant::now())));

        let time_for_svc = time.clone();
        tokio::spawn(async move {
            TtlCacheService::new(TEST_CONFIG_SINGLE_ITEM, rx, &time_for_svc).run().await
        });

        (time, make_api(tx))
    }

    fn api_set_request(key: &str, value: &str) -> warp::test::RequestBuilder {
        warp::test::request()
            .method("POST")
            .path(format!("/set/{}", key).as_str())
            .body(value)
    }

    fn api_get_request(key: &str) -> warp::test::RequestBuilder {
        warp::test::request()
            .method("GET")
            .path(format!("/get/{}", key).as_str())
    }

    #[tokio::test]
    async fn non_existent_keys_return_not_found() {
        let (_, api) = init();

        let get_res = api_get_request("abcda").reply(&api).await;

        assert_eq!(get_res.status(), 404);
    }

    #[tokio::test]
    async fn able_to_set_value() {
        let (_, api) = init();

        let set_res = api_set_request("abcda", "bcda").reply(&api).await;

        assert_eq!(set_res.status(), 200);
    }

    #[tokio::test]
    async fn able_to_get_back_set_values() {
        let (_, api) = init();

        let set_res = api_set_request("abcda", "bcda").reply(&api).await;

        assert_eq!(set_res.status(), 200);

        let get_res = api_get_request("abcda").reply(&api).await;

        assert_eq!(get_res.status(), 200);
        assert_eq!(get_res.body(), "bcda");
    }

    #[tokio::test]
    async fn set_values_have_capacity() {
        let (_, api) = init();

        let set_res = api_set_request("abcda", "bcda").reply(&api).await;
        assert_eq!(set_res.status(), 200);
        let set_res = api_set_request("abcda2", "bcda").reply(&api).await;
        assert_eq!(set_res.status(), 400);
    }

    #[tokio::test]
    async fn set_values_expire() {
        let (time, api) = init();

        let set_res = api_set_request("abcda", "bcda").reply(&api).await;
        assert_eq!(set_res.status(), 200);

        // not pretty, but we have to make sure value is dropped so
        // the lock is released and we can get time again
        tokio::spawn(async move {
            let lock = time.lock().await;
            lock.add_secs(Duration::from_secs(11));
        });

        let get_res = api_get_request("abcda").reply(&api).await;
        assert_eq!(get_res.status(), 404);
    }
}
