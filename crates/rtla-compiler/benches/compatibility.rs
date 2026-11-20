use criterion::{Criterion, criterion_group, criterion_main};
use rtlc::SafetyBounds;
use serde_json::Value;
use std::hint::black_box;
use std::time::Duration;

fn measurements(criterion: &mut Criterion) {
    let operators: Vec<Value> =
        serde_json::from_str(include_str!("../tests/operators-support.json")).unwrap();
    for name in [
        "select-order",
        "sort-ascending",
        "assert-true",
        "fair-forall",
        "fair-intermittent-SF",
    ] {
        let case = operators.iter().find(|case| case["name"] == name).unwrap();
        let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let program = rtla_compiler::compile(&module, &config).unwrap();
        let bounds = SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1_000_000,
            max_temporal_nodes: Some(10000),
        };
        assert_eq!(program.check(bounds).unwrap().status, case["expected"]);
        criterion.bench_function(&format!("operators-compile/{name}"), |bencher| {
            bencher.iter(|| rtla_compiler::compile(black_box(&module), black_box(&config)).unwrap())
        });
        criterion.bench_function(&format!("operators-check/{name}"), |bencher| {
            bencher.iter(|| program.check(black_box(bounds)).unwrap())
        });
    }
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/finite-support.json")).unwrap();
    for name in [
        "model-untyped",
        "state-constraint",
        "action-constraint",
        "choose-order",
        "power-min",
        "temporal-fair",
    ] {
        let case = cases.iter().find(|case| case["name"] == name).unwrap();
        let source = case["source"].as_str().unwrap();
        let configuration = case["config"].as_str().unwrap();
        let module = rtla::parse_module(source).unwrap();
        let config = rtla::parse_config(configuration).unwrap();
        let program = rtla_compiler::compile(&module, &config).unwrap();
        let bounds = SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1_000_000,
            max_temporal_nodes: Some(10000),
        };
        assert_eq!(program.check(bounds).unwrap().status, case["expected"]);
        criterion.bench_function(&format!("compile/{name}"), |bencher| {
            bencher.iter(|| rtla_compiler::compile(black_box(&module), black_box(&config)).unwrap())
        });
        criterion.bench_function(&format!("check/{name}"), |bencher| {
            bencher.iter(|| program.check(black_box(bounds)).unwrap())
        });
    }
    let modules: Vec<Value> =
        serde_json::from_str(include_str!("../tests/modules-support.json")).unwrap();
    for name in [
        "extends-counter",
        "extends-diamond",
        "instance-twice",
        "module-definition-override",
    ] {
        let case = modules.iter().find(|case| case["name"] == name).unwrap();
        let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let dependencies: Vec<_> = case["modules"]
            .as_object()
            .unwrap()
            .values()
            .map(|source| rtla::parse_module(source.as_str().unwrap()).unwrap())
            .collect();
        let program = rtla_compiler::compile_modules(&module, &config, &dependencies).unwrap();
        assert_eq!(
            program
                .check(SafetyBounds {
                    max_depth: 100,
                    max_states: 1000,
                    max_work: 1_000_000,
                    max_temporal_nodes: Some(10000)
                })
                .unwrap()
                .status,
            "complete"
        );
        criterion.bench_function(&format!("modules/{name}"), |bencher| {
            bencher.iter(|| {
                rtla_compiler::compile_modules(
                    black_box(&module),
                    black_box(&config),
                    black_box(&dependencies),
                )
                .unwrap()
            })
        });
    }
    let flat = rtla::parse_module("---- MODULE Flat ----\nEXTENDS Naturals\nCONSTANT Limit\nVARIABLE value\nInit == value = 0\nNext == value < Limit /\\ value' = value + 1\nSafe == value <= Limit\n====").unwrap();
    let config = rtla::parse_config(
        "CONSTANT Limit = 2\nINIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
    )
    .unwrap();
    criterion.bench_function("modules/flat-counter", |bencher| {
        bencher.iter(|| rtla_compiler::compile(black_box(&flat), black_box(&config)).unwrap())
    });
}

criterion_group! {name = benches; config = Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1)); targets = measurements}
criterion_main!(benches);
