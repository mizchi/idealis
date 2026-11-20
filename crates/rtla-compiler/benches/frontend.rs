use criterion::{Criterion, criterion_group, criterion_main};
use rtla_compiler::{CompileOptions, IrVersion, compile_modules_with_options};
use rtlc::SafetyBounds;
use serde_json::Value;
use std::hint::black_box;
use std::time::Duration;

fn measurements(criterion: &mut Criterion) {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/frontend-completion.json")).unwrap();
    for name in [
        "named",
        "label",
        "factorial",
        "instance-parameter",
        "select-lambda",
        "fold-recursive",
        "except-zero-arity",
    ] {
        let case = cases.iter().find(|case| case["name"] == name).unwrap();
        let source = case["source"].as_str().unwrap();
        let module = rtla::parse_module(source).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let dependencies: Vec<_> = case["modules"]
            .as_object()
            .unwrap()
            .values()
            .map(|value| rtla::parse_module(value.as_str().unwrap()).unwrap())
            .collect();
        let options = CompileOptions {
            irVersion: IrVersion::V2,
        };
        let program =
            compile_modules_with_options(&module, &config, &dependencies, options).unwrap();
        let bounds = SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1000000,
            max_temporal_nodes: Some(10000),
        };
        let checked = program.check(bounds).unwrap();
        assert_eq!(checked.status, "complete");
        assert_eq!(checked.states as u64, case["states"].as_u64().unwrap());
        criterion.bench_function(&format!("frontend-parse/{name}"), |bencher| {
            bencher.iter(|| rtla::parse_module(black_box(source)).unwrap())
        });
        criterion.bench_function(&format!("frontend-compile/{name}"), |bencher| {
            bencher.iter(|| {
                compile_modules_with_options(
                    black_box(&module),
                    black_box(&config),
                    black_box(&dependencies),
                    options,
                )
                .unwrap()
            })
        });
        criterion.bench_function(&format!("frontend-check/{name}"), |bencher| {
            bencher.iter(|| program.check(black_box(bounds)).unwrap())
        });
    }
}
criterion_group! {name=benches; config=Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1));targets=measurements}
criterion_main!(benches);
