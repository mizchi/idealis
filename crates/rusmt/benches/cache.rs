//! Check cache costs. Each case has its uncached reference in this binary, so
//! ratios never mix builds. Not buildable against libraries before 0074.
use criterion::{Criterion, criterion_group, criterion_main};
use rusmt::{CacheOptions, CheckCache, Script, Solver};
use std::{hint::black_box, time::Duration};

const BUILD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn cases(criterion: &mut Criterion) {
    let options = CacheOptions {
        enabled: true,
        build_digest: BUILD,
        max_entries: 4096,
    };
    for count in [4, 32, 128] {
        let mut source = (0..count)
            .map(|index| format!("(declare-const x{index} Int)"))
            .collect::<String>();
        source.push_str("(assert (= x0 0))");
        for index in 1..count {
            source.push_str(&format!("(assert (= x{index} (+ x{} 1)))", index - 1));
        }
        let solver = Solver::parse(&source).unwrap();
        criterion.bench_function(&format!("prepared-{count}"), |bench| {
            bench.iter(|| solver.check(black_box(2_000_000)))
        });
        // Every call misses: a fresh cache pays for the key and the write.
        criterion.bench_function(&format!("cache-miss-{count}"), |bench| {
            bench.iter(|| {
                let mut cache = CheckCache::default();
                solver
                    .check_incremental(&[], black_box(2_000_000), &mut cache, options)
                    .unwrap()
            })
        });
    }
    for (name, source) in [
        ("repeat-64", include_str!("fixtures/repeat-64.smt2")),
        ("scopes-32", include_str!("fixtures/scopes-32.smt2")),
        ("ite-64", include_str!("fixtures/ite-64.smt2")),
    ] {
        let script = Script::parse(source).unwrap();
        criterion.bench_function(&format!("script-prepared-{name}"), |bench| {
            bench.iter(|| script.solve(black_box(2_000_000)))
        });
        // Fixtures repeat queries, so a cold run already hits within itself.
        criterion.bench_function(&format!("script-cache-cold-{name}"), |bench| {
            bench.iter(|| {
                let mut cache = CheckCache::default();
                script
                    .solve_incremental(black_box(2_000_000), &mut cache, options)
                    .unwrap()
            })
        });
        let mut warm = CheckCache::default();
        script
            .solve_incremental(2_000_000, &mut warm, options)
            .unwrap();
        criterion.bench_function(&format!("script-cache-warm-{name}"), |bench| {
            bench.iter(|| {
                script
                    .solve_incremental(black_box(2_000_000), &mut warm, options)
                    .unwrap()
            })
        });
    }
}
criterion_group! {name=benches;config=Criterion::default().warm_up_time(Duration::from_secs(1)).measurement_time(Duration::from_secs(2)).sample_size(20);targets=cases}
criterion_main!(benches);
