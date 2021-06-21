#in-mem-cached

To build & run

```bash
cargo build
cargo run
```

Tested with rustup 1.52.1.
Running will start a service on port `8080`.

Service has following endpoints:
- GET - `/health-check` - returns "Ok"
- POST - `/set/<key:string>` - takes bytes payload and tries to decode it to UTF-8, sets value to the cache
- GET - `/get/<key:string>` - reads value from the cache using key

Service configuration is stored in `Config` struct, that includes few values like cache maximum capacity, ttl, parameters for cache eviction mechanism. Defaults are set in `main.rs`. `capacity` parameters governs total entries in the cache. It is optional and `None` by default, but can be used to minimize allocations during runtime.

To run tests

```bash
cargo test
```

##loadtest

<TBD>