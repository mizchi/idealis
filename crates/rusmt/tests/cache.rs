//! The check result cache is opt-in, reuses only complete answers and never
//! turns a corrupted or foreign snapshot into a result.
use rusmt::{Assumption, CacheOptions, CacheStats, CheckCache, Script, Solver};
use serde_json::{Value, json};

const BUILD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn enabled(build_digest: &str, max_entries: u32) -> CacheOptions<'_> {
    CacheOptions {
        enabled: true,
        build_digest,
        max_entries,
    }
}
fn answer(solution: &rusmt::Solution) -> Value {
    let mut value = serde_json::to_value(solution).unwrap();
    value.as_object_mut().unwrap().remove("stats");
    value
}
fn stats(hits: u32, misses: u32, writes: u32, invalidated: bool) -> CacheStats {
    CacheStats {
        hits,
        misses,
        writes,
        invalidated,
    }
}
fn context() -> Solver {
    let mut solver = Solver::parse(
        "(declare-const x Int)(declare-const y Int)(declare-const b Bool)(assert (=> b (< x y)))",
    )
    .unwrap();
    solver.assert("(<= (- y x) 3)").unwrap();
    solver
}

#[test]
fn disabled_options_delegate_to_check_and_leave_the_cache_untouched() {
    let solver = context();
    let mut cache = CheckCache::default();
    let warm = solver
        .check_incremental(&[], 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(warm.stats, stats(0, 1, 1, false));
    let before = serde_json::to_value(&cache).unwrap();
    let plain = serde_json::to_value(solver.check(100_000)).unwrap();
    let disabled = solver
        .check_incremental(&[], 100_000, &mut cache, CacheOptions::default())
        .unwrap();
    assert_eq!(serde_json::to_value(&disabled.result).unwrap(), plain);
    assert_eq!(disabled.stats, CacheStats::default());
    assert_eq!(serde_json::to_value(&cache).unwrap(), before);
}

#[test]
fn hits_return_the_fresh_answer_without_charging_work() {
    let solver = context();
    let mut cache = CheckCache::default();
    let assumptions = [Assumption {
        symbol: "b".into(),
        positive: true,
    }];
    let fresh = solver.check_assuming(&assumptions, 100_000).unwrap();
    let cold = solver
        .check_incremental(&assumptions, 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&cold.result).unwrap(),
        serde_json::to_value(&fresh).unwrap()
    );
    assert_eq!(cold.stats, stats(0, 1, 1, false));
    for budget in [100_000, 0] {
        let warm = solver
            .check_incremental(&assumptions, budget, &mut cache, enabled(BUILD, 8))
            .unwrap();
        assert_eq!(answer(&warm.result), answer(&fresh));
        assert_eq!(
            serde_json::to_value(&warm.result).unwrap()["stats"]["work"],
            0
        );
        assert_eq!(warm.stats, stats(1, 0, 0, false));
    }
    // Different assumptions, assertions and scopes are different queries.
    let other = solver
        .check_incremental(&[], 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(other.stats, stats(0, 1, 1, false));
    let mut changed = context();
    changed.push(1).unwrap();
    changed.assert("(< y x)").unwrap();
    let inside = changed
        .check_incremental(&assumptions, 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(inside.result.status, "unsat");
    assert_eq!(inside.stats, stats(0, 1, 1, false));
    changed.pop(1).unwrap();
    let restored = changed
        .check_incremental(&assumptions, 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(restored.stats, stats(1, 0, 0, false));
    // The key is the normalized source, so an equivalent prepared source hits.
    let reparsed = Solver::parse(
        "(declare-const x Int) (declare-const y Int) (declare-const b Bool)
         (assert (=> b (< x y))) ; comment
         (assert (<= (- y x) 3)) (check-sat)",
    )
    .unwrap();
    let shared = reparsed
        .check_incremental(&assumptions, 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(shared.stats, stats(1, 0, 0, false));
}

#[test]
fn unknown_results_and_errors_are_not_stored() {
    let solver = context();
    let mut cache = CheckCache::default();
    let cut = solver
        .check_incremental(&[], 3, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(cut.result.status, "unknown");
    assert_eq!(cut.stats, stats(0, 1, 0, false));
    let missing = [Assumption {
        symbol: "missing".into(),
        positive: true,
    }];
    assert!(
        solver
            .check_incremental(&missing, 100_000, &mut cache, enabled(BUILD, 8))
            .is_err()
    );
    let full = solver
        .check_incremental(&[], 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(full.result.status, "sat");
    assert_eq!(full.stats, stats(0, 1, 1, false));
}

#[test]
fn build_digest_capacity_and_snapshot_integrity_are_enforced() {
    let solver = context();
    let mut cache = CheckCache::default();
    assert!(
        solver
            .check_incremental(&[], 100_000, &mut cache, enabled("v1", 8))
            .is_err()
    );
    let none = solver
        .check_incremental(&[], 100_000, &mut cache, enabled(BUILD, 0))
        .unwrap();
    assert_eq!(none.stats, stats(0, 1, 0, false));
    solver
        .check_incremental(&[], 100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    let foreign = solver
        .check_incremental(&[], 100_000, &mut cache, enabled(OTHER, 8))
        .unwrap();
    assert_eq!(foreign.stats, stats(0, 1, 1, true));

    let snapshot = serde_json::to_value(&cache).unwrap();
    let restored: CheckCache = serde_json::from_value(snapshot.clone()).unwrap();
    let mut restored = restored;
    let hit = solver
        .check_incremental(&[], 100_000, &mut restored, enabled(OTHER, 8))
        .unwrap();
    assert_eq!(hit.stats, stats(1, 0, 0, false));

    let mut tampered = snapshot.clone();
    let entry = tampered["entries"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    entry["values"]["x"] = json!("12345");
    let mut tampered: CheckCache = serde_json::from_value(tampered).unwrap();
    let error = solver
        .check_incremental(&[], 100_000, &mut tampered, enabled(OTHER, 8))
        .err()
        .unwrap();
    assert_eq!(error.code, "protocol");
    let mut unknown_field = snapshot;
    unknown_field["extra"] = json!(1);
    assert!(serde_json::from_value::<CheckCache>(unknown_field).is_err());
}

#[test]
fn merge_combines_matching_namespaces_and_rejects_conflicts() {
    let mut left = CheckCache::default();
    let mut right = CheckCache::default();
    let solver = context();
    let mut other = context();
    other.assert("b").unwrap();
    solver
        .check_incremental(&[], 100_000, &mut left, enabled(BUILD, 8))
        .unwrap();
    other
        .check_incremental(&[], 100_000, &mut right, enabled(BUILD, 8))
        .unwrap();
    let mut merged = left.clone();
    merged.merge(right.clone(), 8).unwrap();
    for query in [&solver, &other] {
        let hit = query
            .check_incremental(&[], 100_000, &mut merged, enabled(BUILD, 8))
            .unwrap();
        assert_eq!(hit.stats, stats(1, 0, 0, false));
    }
    let mut truncated = left.clone();
    truncated.merge(right.clone(), 1).unwrap();
    assert_eq!(
        serde_json::to_value(&truncated).unwrap()["entries"]
            .as_object()
            .unwrap()
            .len(),
        1
    );

    let mut foreign = CheckCache::default();
    solver
        .check_incremental(&[], 100_000, &mut foreign, enabled(OTHER, 8))
        .unwrap();
    assert_eq!(
        left.clone().merge(foreign, 8).err().unwrap().code,
        "protocol"
    );
}

#[test]
fn scripts_share_the_cache_and_hits_leave_budget_for_later_checks() {
    let source = "(declare-const x Int)(declare-const b Bool)
        (assert (= x (ite b 3 4)))(check-sat-assuming (b))
        (push 1)(assert (= x 3))(check-sat-assuming ((not b)))(pop 1)
        (check-sat-assuming (b))(check-sat)";
    let script = Script::parse(source).unwrap();
    let plain = serde_json::to_value(script.solve(100_000)).unwrap();
    let mut cache = CheckCache::default();
    let cold = script
        .solve_incremental(100_000, &mut cache, enabled(BUILD, 16))
        .unwrap();
    assert_eq!(serde_json::to_value(&cold.results).unwrap()[0], plain[0]);
    // The third check repeats the first query inside the same run.
    assert_eq!(cold.stats, stats(1, 3, 3, false));
    let warm = script
        .solve_incremental(0, &mut cache, enabled(BUILD, 16))
        .unwrap();
    assert_eq!(warm.stats, stats(4, 0, 0, false));
    for (index, result) in warm.results.iter().enumerate() {
        let mut expected = plain[index].clone();
        expected.as_object_mut().unwrap().remove("stats");
        assert_eq!(answer(result), expected, "{index}");
    }
    let disabled = script
        .solve_incremental(100_000, &mut cache, CacheOptions::default())
        .unwrap();
    assert_eq!(serde_json::to_value(&disabled.results).unwrap(), plain);
    assert_eq!(disabled.stats, CacheStats::default());
}

#[test]
fn json_requests_carry_the_snapshot_and_default_to_disabled() {
    let mut engine = rusmt::Engine::default();
    let mut call = |request: Value| {
        engine.dispatch(&serde_json::to_vec(&json!({"version":1,"request":request})).unwrap())
    };
    let source = "(declare-const x Int)(assert (> x 2))(check-sat)(check-sat)";
    let cold = call(
        json!({"kind":"run-script-incremental","source":source,"maxWork":10000,
        "enabled":true,"buildDigest":BUILD,"maxEntries":4}),
    );
    assert_eq!(cold["kind"], "incremental-script-results", "{cold}");
    assert_eq!(
        cold["stats"],
        json!({"hits":1,"misses":1,"writes":1,"invalidated":false})
    );
    assert_eq!(cold["results"][1]["stats"]["work"], 0);
    let handle =
        call(json!({"kind":"prepare","source":"(declare-const x Int)(assert (> x 2))"}))["handle"]
            .clone();
    let warm = call(
        json!({"kind":"check-incremental","handle":handle,"maxWork":0,
        "cache":cold["cache"],"enabled":true,"buildDigest":BUILD,"maxEntries":4}),
    );
    assert_eq!(warm["kind"], "incremental-solution", "{warm}");
    assert_eq!(warm["result"]["values"]["x"], "3");
    assert_eq!(warm["stats"]["hits"], 1);
    assert_eq!(warm["cache"], cold["cache"]);
    let disabled = call(
        json!({"kind":"check-incremental","handle":handle,"maxWork":0,
        "cache":cold["cache"]}),
    );
    assert_eq!(disabled["result"]["status"], "unknown", "{disabled}");
    assert_eq!(
        disabled["stats"],
        json!({"hits":0,"misses":0,"writes":0,"invalidated":false})
    );
    assert_eq!(disabled["cache"], cold["cache"]);
    for request in [
        json!({"kind":"check-incremental","handle":handle,"maxWork":0,"extra":1}),
        json!({"kind":"check-incremental","handle":handle,"maxWork":0,"enabled":true,"buildDigest":"bad"}),
        json!({"kind":"run-script-incremental","source":"(check-sat)","maxWork":1,"cache":{"format":"other"}}),
    ] {
        let reply = call(request.clone());
        assert_eq!(reply["kind"], "error", "{request} {reply}");
    }
}

#[test]
fn scripts_and_contexts_with_the_same_normalized_source_share_entries() {
    let mut cache = CheckCache::default();
    let script = Script::parse(
        "(declare-const x Int)(declare-const b Bool)(push 1)(assert (=> b (> x 4)))(check-sat-assuming (b))(pop 1)",
    )
    .unwrap();
    let cold = script
        .solve_incremental(100_000, &mut cache, enabled(BUILD, 8))
        .unwrap();
    assert_eq!(cold.stats, stats(0, 1, 1, false));
    let assumptions = [Assumption {
        symbol: "b".into(),
        positive: true,
    }];
    let mut stacked = Solver::parse("(declare-const x Int)\n(declare-const b Bool)").unwrap();
    stacked.assert("(=> b (> x 4))").unwrap();
    let inline =
        Solver::parse("(declare-const x Int)(declare-const b Bool)(assert (=> b (> x 4)))")
            .unwrap();
    for solver in [&stacked, &inline] {
        let hit = solver
            .check_incremental(&assumptions, 0, &mut cache, enabled(BUILD, 8))
            .unwrap();
        assert_eq!(hit.stats, stats(1, 0, 0, false));
        assert_eq!(answer(&hit.result), answer(&cold.results[0]));
    }
}
