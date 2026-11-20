use criterion::{Criterion, criterion_group, criterion_main};
use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;
use serde_json::Value;
use std::hint::black_box;
use std::time::Duration;

fn measurements(criterion: &mut Criterion) {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/recursive-actions.json")).unwrap();
    for name in [
        "inline-control",
        "tail-12",
        "branching",
        "captured",
        "target-capture",
        "multi-argument",
        "accumulator",
    ] {
        let case = cases.iter().find(|case| case["name"] == name).unwrap();
        let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let options = CompileOptions {
            irVersion: IrVersion::V2,
        };
        let program = compile_with_options(&module, &config, options).unwrap();
        let bounds = SafetyBounds {
            max_depth: 1000,
            max_states: 2000,
            max_work: 1_000_000,
            max_temporal_nodes: Some(10000),
        };
        let expected = program.check(bounds).unwrap();
        assert_eq!(expected.status, "complete", "{name}: {expected:?}");
        assert_eq!(expected.states as u64, case["states"].as_u64().unwrap());
        let prepared = program.clone().prepare().unwrap();
        assert_eq!(prepared.check(bounds).unwrap(), expected);
        criterion.bench_function(&format!("recursive-compile/{name}"), |bencher| {
            bencher.iter(|| {
                compile_with_options(black_box(&module), black_box(&config), options).unwrap()
            })
        });
        criterion.bench_function(&format!("recursive-check/{name}"), |bencher| {
            bencher.iter(|| program.check(black_box(bounds)).unwrap())
        });
        criterion.bench_function(&format!("recursive-prepared/{name}"), |bencher| {
            bencher.iter(|| prepared.check(black_box(bounds)).unwrap())
        });
    }
}
criterion_group! {name=benches; config=Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1));targets=measurements}
criterion_main!(benches);
