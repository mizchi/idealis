//! An engine-side cache store must answer exactly like threading the
//! snapshot through every request (plan 0076).
use rusmt::Engine;
use serde_json::{Value, json};

const BUILD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const DECLARATIONS: &str = "(declare-const x Int)(declare-const y Int)(declare-const z Int)(declare-const p Bool)(declare-const q Bool)";
const TEMPLATES: [&str; 12] = [
    "(<= (- x y) C)",
    "(< y z)",
    "(= y C)",
    "(= z C)",
    "(= x (+ y C))",
    "(or p (<= x y))",
    "(=> q (> z x))",
    "(= (ite p x y) (+ z C))",
    "(distinct x y z)",
    "(not (= y C))",
    "false",
    "(< x undeclared)",
];
const SCRIPTS: [&str; 3] = [
    include_str!("../benches/fixtures/repeat-64.smt2"),
    include_str!("../benches/fixtures/scopes-32.smt2"),
    "(declare-const a Int)(declare-const b Bool)(assert (<= a 3))(check-sat)(push 1)(assert (> a 5))(check-sat)(pop 1)(check-sat-assuming (b))(check-sat-assuming (b))",
];

fn send(engine: &mut Engine, request: Value) -> Value {
    let bytes = serde_json::to_vec(&json!({"version":1,"request":request})).unwrap();
    engine.dispatch(&bytes)
}
fn open(engine: &mut Engine, max_entries: u32, cache: Option<&Value>) -> Value {
    let mut request = json!({"kind":"cache-open","buildDigest":BUILD,"maxEntries":max_entries});
    if let Some(cache) = cache {
        request["cache"] = cache.clone();
    }
    send(engine, request)
}
fn export(engine: &mut Engine, handle: &Value) -> Value {
    let reply = send(engine, json!({"kind":"cache-export","cacheHandle":handle}));
    assert_eq!(reply["kind"], "cache-snapshot", "{reply}");
    reply["cache"].clone()
}

struct Lcg(u32);
impl Lcg {
    fn next(&mut self, bound: u32) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 16) % bound
    }
}

/// One generated request without the cache fields; `None` ends nothing.
fn operation(random: &mut Lcg, handle: &Value) -> Value {
    match random.next(7) {
        0 | 1 => {
            let template = TEMPLATES[random.next(TEMPLATES.len() as u32) as usize];
            let constant = random.next(9) as i64 - 4;
            let constant = if constant < 0 {
                format!("(- {})", -constant)
            } else {
                constant.to_string()
            };
            json!({"kind":"assert","handle":handle,"expression":template.replace('C', &constant)})
        }
        2 => json!({"kind":"push","handle":handle,"count":random.next(3)}),
        3 => json!({"kind":"pop","handle":handle,"count":random.next(3)}),
        4 => json!({
            "kind":"run-script-incremental",
            "source":SCRIPTS[random.next(SCRIPTS.len() as u32) as usize],
            "maxWork":([0, 300, 2_000_000][random.next(3) as usize]),
        }),
        _ => {
            let assumptions: Vec<Value> = (0..random.next(3))
                .map(|_| {
                    json!({
                        "symbol":(["p","q","missing"][random.next(3) as usize]),
                        "positive":random.next(2) == 0,
                    })
                })
                .collect();
            json!({
                "kind":"check-incremental","handle":handle,
                "maxWork":([0, 40, 2_000_000][random.next(3) as usize]),
                "assumptions":assumptions,
            })
        }
    }
}

fn is_cached(request: &Value) -> bool {
    matches!(
        request["kind"].as_str(),
        Some("check-incremental" | "run-script-incremental")
    )
}

#[test]
fn the_store_answers_like_the_threaded_snapshot() {
    let mut requests = 0;
    let (mut hits, mut writes, mut errors) = (0, 0, 0);
    for seed in 0..60_u32 {
        let mut random = Lcg(seed.wrapping_mul(2_654_435_761));
        let capacity = [4, 16, 4096][seed as usize % 3];
        let mut threaded = Engine::default();
        let mut stored = Engine::default();
        let opened = open(&mut stored, capacity, None);
        assert_eq!(opened["kind"], "cache-opened", "{opened}");
        assert_eq!(opened["entries"], 0);
        assert_eq!(opened["invalidated"], false);
        let store = opened["cacheHandle"].clone();
        let mut snapshot = export(&mut stored, &store);
        let prepare = json!({"kind":"prepare","source":DECLARATIONS});
        let handle = send(&mut threaded, prepare.clone())["handle"].clone();
        assert_eq!(send(&mut stored, prepare)["handle"], handle);
        for _ in 0..40 {
            let request = operation(&mut random, &handle);
            requests += 1;
            if !is_cached(&request) {
                assert_eq!(
                    send(&mut threaded, request.clone()),
                    send(&mut stored, request)
                );
                continue;
            }
            let mut with_snapshot = request.clone();
            with_snapshot["enabled"] = json!(true);
            with_snapshot["buildDigest"] = json!(BUILD);
            with_snapshot["maxEntries"] = json!(capacity);
            with_snapshot["cache"] = snapshot.clone();
            let mut with_store = request;
            with_store["cacheHandle"] = store.clone();
            let mut expected = send(&mut threaded, with_snapshot);
            let mut actual = send(&mut stored, with_store);
            if expected["kind"] == "error" {
                errors += 1;
                assert_eq!(actual, expected, "seed {seed}");
                continue;
            }
            hits += expected["stats"]["hits"].as_u64().unwrap();
            writes += expected["stats"]["writes"].as_u64().unwrap();
            snapshot = expected
                .as_object_mut()
                .unwrap()
                .remove("cache")
                .expect("snapshot reply");
            assert_eq!(
                actual.as_object_mut().unwrap().remove("cacheHandle"),
                Some(store.clone()),
                "seed {seed}"
            );
            assert_eq!(actual, expected, "seed {seed}");
            assert_eq!(export(&mut stored, &store), snapshot, "seed {seed}");
        }
    }
    assert_eq!(requests, 2400);
    // The comparison must exercise hits, writes and errors.
    assert!(
        hits > 100 && writes > 100 && errors > 10,
        "{hits} {writes} {errors}"
    );
}

fn filled(engine: &mut Engine, build: &str) -> Value {
    let handle = send(engine, json!({"kind":"prepare","source":DECLARATIONS}))["handle"].clone();
    send(
        engine,
        json!({"kind":"check-incremental","handle":handle,"maxWork":100_000,
            "enabled":true,"buildDigest":build,"maxEntries":64}),
    )["cache"]
        .clone()
}

#[test]
fn opening_binds_the_snapshot_and_rejects_corrupt_entries() {
    let mut engine = Engine::default();
    let same = filled(&mut engine, BUILD);
    let opened = open(&mut engine, 64, Some(&same));
    assert_eq!(opened["entries"], 1);
    assert_eq!(opened["invalidated"], false);
    assert_eq!(export(&mut engine, &opened["cacheHandle"]), same);

    let foreign = filled(&mut engine, OTHER);
    let opened = open(&mut engine, 64, Some(&foreign));
    assert_eq!(opened["entries"], 0);
    assert_eq!(opened["invalidated"], true);

    let mut corrupt = same.clone();
    let entry = corrupt["entries"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    entry["digest"] = json!("0".repeat(64));
    let before = open(&mut engine, 64, None)["cacheHandle"].as_u64().unwrap();
    let rejected = open(&mut engine, 64, Some(&corrupt));
    assert_eq!(rejected["kind"], "error");
    assert_eq!(rejected["message"], "check cache digest mismatch");
    // A rejected open allocates no handle.
    assert_eq!(
        open(&mut engine, 64, None)["cacheHandle"].as_u64().unwrap(),
        before + 1
    );

    let bad = send(
        &mut engine,
        json!({"kind":"cache-open","buildDigest":"short","maxEntries":4}),
    );
    assert_eq!(bad["message"], "expected build SHA-256");
}

#[test]
fn capacity_set_at_open_bounds_the_store() {
    let mut engine = Engine::default();
    let store = open(&mut engine, 2, None)["cacheHandle"].clone();
    for bound in 0..5 {
        let handle = send(
            &mut engine,
            json!({"kind":"prepare","source":format!("(declare-const x Int)(assert (<= x {bound}))")}),
        )["handle"]
            .clone();
        let reply = send(
            &mut engine,
            json!({"kind":"check-incremental","handle":handle,"maxWork":100_000,"cacheHandle":store}),
        );
        assert_eq!(reply["kind"], "incremental-solution", "{reply}");
    }
    let snapshot = export(&mut engine, &store);
    assert_eq!(snapshot["entries"].as_object().unwrap().len(), 2);
}

#[test]
fn handles_are_separate_checked_and_closed() {
    let mut engine = Engine::default();
    let first = open(&mut engine, 8, None)["cacheHandle"].clone();
    let second = open(&mut engine, 8, None)["cacheHandle"].clone();
    assert_ne!(first, second);
    let handle =
        send(&mut engine, json!({"kind":"prepare","source":DECLARATIONS}))["handle"].clone();
    // Problem handles and cache handles are numbered independently.
    assert_eq!(handle, json!(1));
    assert_eq!(first, json!(1));
    let check =
        json!({"kind":"check-incremental","handle":handle,"maxWork":100_000,"cacheHandle":first});
    assert_eq!(send(&mut engine, check.clone())["stats"]["writes"], 1);
    assert_eq!(send(&mut engine, check.clone())["stats"]["hits"], 1);
    assert_eq!(
        export(&mut engine, &second)["entries"]
            .as_object()
            .unwrap()
            .len(),
        0
    );
    for field in [
        ("cache", export(&mut engine, &second)),
        ("enabled", json!(true)),
        ("buildDigest", json!(BUILD)),
        ("maxEntries", json!(4)),
    ] {
        let mut mixed = check.clone();
        mixed[field.0] = field.1;
        let reply = send(&mut engine, mixed);
        assert_eq!(
            reply["message"], "cacheHandle excludes snapshot fields",
            "{}",
            field.0
        );
    }
    let script = json!({"kind":"run-script-incremental","source":SCRIPTS[2],"maxWork":100_000,"cacheHandle":99});
    assert_eq!(send(&mut engine, script)["message"], "unknown cache handle");
    assert_eq!(
        send(
            &mut engine,
            json!({"kind":"cache-close","cacheHandle":first})
        )["kind"],
        "released"
    );
    for request in [
        check,
        json!({"kind":"cache-export","cacheHandle":first}),
        json!({"kind":"cache-close","cacheHandle":first}),
    ] {
        assert_eq!(
            send(&mut engine, request)["message"],
            "unknown cache handle"
        );
    }
}
