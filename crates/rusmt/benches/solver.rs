use criterion::{Criterion, criterion_group, criterion_main};
use rusmt::{Problem, Script, Solver};
use std::{hint::black_box, time::Duration};

fn cases(criterion: &mut Criterion) {
    for count in [4, 32, 128] {
        let mut source = (0..count)
            .map(|index| format!("(declare-const x{index} Int)"))
            .collect::<String>();
        source.push_str("(assert (= x0 0))");
        for index in 1..count {
            source.push_str(&format!("(assert (= x{index} (+ x{} 1)))", index - 1));
        }
        let problem = Problem::parse(&source).unwrap();
        criterion.bench_function(&format!("parse-solve-{count}"), |bench| {
            bench.iter(|| Problem::parse(black_box(&source)).unwrap().solve(2_000_000))
        });
        criterion.bench_function(&format!("prepared-{count}"), |bench| {
            bench.iter(|| problem.solve(black_box(2_000_000)))
        });
    }
    let source = include_str!("fixtures/integer-ite.smt2");
    criterion.bench_function("parse-solve-ite", |bench| {
        bench.iter(|| Problem::parse(black_box(source)).unwrap().solve(2_000_000))
    });
    for (name, source) in [
        ("repeat-64", include_str!("fixtures/repeat-64.smt2")),
        ("scopes-32", include_str!("fixtures/scopes-32.smt2")),
        ("ite-64", include_str!("fixtures/ite-64.smt2")),
    ] {
        let script = Script::parse(source).unwrap();
        assert_eq!(script.solve(2_000_000).len(), 64);
        criterion.bench_function(&format!("script-parse-{name}"), |bench| {
            bench.iter(|| Script::parse(black_box(source)).unwrap())
        });
        criterion.bench_function(&format!("script-prepared-{name}"), |bench| {
            bench.iter(|| script.solve(black_box(2_000_000)))
        });
    }
    for count in [32, 256] {
        let declarations = (0..count)
            .map(|index| format!("(declare-const x{index} Int)"))
            .collect::<String>();
        let assertions = (1..count)
            .map(|index| format!("(<= (- x{index} x{}) {index})", index - 1))
            .collect::<Vec<_>>();
        criterion.bench_function(&format!("context-assert-{count}"), |bench| {
            bench.iter(|| {
                let mut solver = Solver::parse(black_box(&declarations)).unwrap();
                for assertion in &assertions {
                    solver.assert(black_box(assertion)).unwrap();
                }
                black_box(solver)
            })
        });
        let constant = format!("(= x{} 0)", count - 1);
        criterion.bench_function(&format!("context-constant-{count}"), |bench| {
            bench.iter(|| {
                let mut solver = Solver::parse(black_box(&declarations)).unwrap();
                for assertion in &assertions {
                    solver.assert(black_box(assertion)).unwrap();
                }
                solver.assert(black_box(&constant)).unwrap();
                black_box(solver)
            })
        });
    }
    let source = include_str!("fixtures/repeat-64.smt2")
        .split_once("(check-sat)")
        .unwrap()
        .0;
    criterion.bench_function("context-scopes-32", |bench| {
        bench.iter(|| {
            let mut solver = Solver::parse(black_box(source)).unwrap();
            for _ in 0..32 {
                solver.push(1).unwrap();
                solver.assert(black_box("(<= x31 31)")).unwrap();
                solver.pop(1).unwrap();
            }
            black_box(solver)
        })
    });
}
criterion_group! {name=benches;config=Criterion::default().warm_up_time(Duration::from_secs(1)).measurement_time(Duration::from_secs(2)).sample_size(20);targets=cases}
criterion_main!(benches);
