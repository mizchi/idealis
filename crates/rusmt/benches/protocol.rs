//! Cost of passing the whole check cache through the JSON protocol, against
//! the same cache used in place. Cases fixed in
//! evals/plans/0075-smt-cache-transport.md.
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rusmt::{CacheOptions, CheckCache, Engine, Solver};
use serde_json::{Value, json};
use std::{hint::black_box, time::Duration};

const BUILD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OPTIONS: CacheOptions<'static> = CacheOptions {
    enabled: true,
    build_digest: BUILD,
    max_entries: 8192,
};

fn chain(count: usize) -> String {
    let mut source = (0..count)
        .map(|index| format!("(declare-const x{index} Int)"))
        .collect::<String>();
    source.push_str("(assert (= x0 0))");
    for index in 1..count {
        source.push_str(&format!("(assert (= x{index} (+ x{} 1)))", index - 1));
    }
    source
}

/// N entries for other sources; the measured query is never among them.
fn fillers(count: usize) -> CheckCache {
    let mut cache = CheckCache::default();
    for bound in 0..count {
        Solver::parse(&format!("(declare-const x Int)(assert (<= x {bound}))"))
            .unwrap()
            .check_incremental(&[], 2_000_000, &mut cache, OPTIONS)
            .unwrap();
    }
    cache
}

fn request(handle: &Value, cache: Option<&CheckCache>) -> Vec<u8> {
    let request = match cache {
        None => json!({"kind":"check","handle":handle,"maxWork":2_000_000}),
        Some(cache) => json!({
            "kind":"check-incremental","handle":handle,"maxWork":2_000_000,
            "enabled":true,"buildDigest":BUILD,"maxEntries":8192,"cache":cache,
        }),
    };
    serde_json::to_vec(&json!({"version":1,"request":request})).unwrap()
}

fn command(request: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"version":1,"request":request})).unwrap()
}

fn round_trip(engine: &mut Engine, bytes: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&engine.dispatch(black_box(bytes))).unwrap()
}

fn cases(criterion: &mut Criterion) {
    let source = chain(32);
    let solver = Solver::parse(&source).unwrap();
    let mut engine = Engine::default();
    let prepared = engine.dispatch(
        &serde_json::to_vec(&json!({"version":1,"request":{"kind":"prepare","source":source}}))
            .unwrap(),
    );
    let handle = prepared["handle"].clone();
    assert_eq!(prepared["kind"], "prepared");
    let plain = request(&handle, None);
    assert!(
        String::from_utf8(round_trip(&mut engine, &plain))
            .unwrap()
            .contains("\"solution\"")
    );
    criterion.bench_function("protocol-check", |bench| {
        bench.iter(|| round_trip(&mut engine, &plain))
    });
    for count in [0, 64, 512, 4096] {
        let miss = fillers(count);
        let mut hit = miss.clone();
        solver
            .check_incremental(&[], 2_000_000, &mut hit, OPTIONS)
            .unwrap();
        for (name, cache, hits) in [("miss", &miss, 0), ("hit", &hit, 1)] {
            let bytes = request(&handle, Some(cache));
            let reply: Value = serde_json::from_slice(&round_trip(&mut engine, &bytes)).unwrap();
            assert_eq!(reply["kind"], "incremental-solution");
            assert_eq!(reply["stats"]["hits"], hits);
            criterion.bench_function(&format!("protocol-{name}-{count}"), |bench| {
                bench.iter(|| round_trip(&mut engine, &bytes))
            });
        }
        // Plan 0076: the same snapshots kept in an engine-side store.
        let opened = engine.dispatch(&command(json!({
            "kind":"cache-open","buildDigest":BUILD,"maxEntries":8192,"cache":hit,
        })));
        assert_eq!(opened["kind"], "cache-opened");
        let by_handle = command(json!({
            "kind":"check-incremental","handle":handle,"maxWork":2_000_000,
            "cacheHandle":opened["cacheHandle"],
        }));
        let reply: Value = serde_json::from_slice(&round_trip(&mut engine, &by_handle)).unwrap();
        assert_eq!(reply["stats"]["hits"], 1);
        criterion.bench_function(&format!("handle-hit-{count}"), |bench| {
            bench.iter(|| round_trip(&mut engine, &by_handle))
        });
        let open_miss = command(json!({
            "kind":"cache-open","buildDigest":BUILD,"maxEntries":8192,"cache":miss,
        }));
        criterion.bench_function(&format!("handle-miss-{count}"), |bench| {
            bench.iter_batched(
                || {
                    let mut fresh = Engine::default();
                    let prepared =
                        fresh.dispatch(&command(json!({"kind":"prepare","source":source})));
                    let opened = fresh.dispatch(&open_miss);
                    let request = command(json!({
                        "kind":"check-incremental","handle":prepared["handle"],
                        "maxWork":2_000_000,"cacheHandle":opened["cacheHandle"],
                    }));
                    (fresh, request)
                },
                |(mut fresh, request)| {
                    let reply = round_trip(&mut fresh, &request);
                    (reply, fresh)
                },
                BatchSize::LargeInput,
            )
        });
        let mut store = hit.clone();
        assert_eq!(
            solver
                .check_incremental(&[], 2_000_000, &mut store, OPTIONS)
                .unwrap()
                .stats
                .hits,
            1
        );
        criterion.bench_function(&format!("store-hit-{count}"), |bench| {
            bench.iter(|| {
                solver
                    .check_incremental(&[], black_box(2_000_000), &mut store, OPTIONS)
                    .unwrap()
            })
        });
        criterion.bench_function(&format!("store-miss-{count}"), |bench| {
            bench.iter_batched(
                || miss.clone(),
                |mut cache| {
                    let reply = solver
                        .check_incremental(&[], black_box(2_000_000), &mut cache, OPTIONS)
                        .unwrap();
                    (reply, cache)
                },
                BatchSize::LargeInput,
            )
        });
    }
}
criterion_group! {name=benches;config=Criterion::default().warm_up_time(Duration::from_secs(1)).measurement_time(Duration::from_secs(2)).sample_size(20);targets=cases}
criterion_main!(benches);
