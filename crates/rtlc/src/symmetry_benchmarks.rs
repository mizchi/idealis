#![allow(unused_imports, dead_code)]
include!("lib.rs");

mod measurements {
    use crate::{SafetyBounds, SafetyOptions, SafetyProgram};
    use criterion::{Criterion, Throughput};
    use serde::Deserialize;
    use std::hint::black_box;
    use std::time::Duration;

    #[derive(Deserialize)]
    struct Input {
        name: String,
        states: usize,
        generated: u32,
        program: SafetyProgram,
        options: Option<SafetyOptions>,
    }

    pub fn run(criterion: &mut Criterion) {
        let mut inputs: Vec<Input> =
            serde_json::from_slice(include_bytes!("symmetry-fixtures.json")).unwrap();
        inputs.extend(
            serde_json::from_slice::<Vec<Input>>(include_bytes!("reduction-fixtures.json"))
                .unwrap(),
        );
        if std::env::var_os("IDEALIS_SYMMETRY_REVERSE").is_some() {
            inputs.reverse();
        }
        let bounds = SafetyBounds {
            max_depth: 100_000,
            max_states: 1_000_000,
            max_work: 1_000_000_000,
            max_temporal_nodes: None,
        };
        let mut group = criterion.benchmark_group("symmetry");
        for input in &inputs {
            let result = input
                .program
                .check_with_options(bounds, input.options.unwrap_or_default())
                .unwrap();
            assert_eq!(result.status, "complete");
            assert_eq!(result.states, input.states);
            assert_eq!(result.generated, input.generated);
            group.throughput(Throughput::Elements(u64::from(input.generated)));
            group.bench_function(&input.name, |bencher| {
                bencher.iter(|| {
                    black_box(
                        input
                            .program
                            .check_with_options(
                                black_box(bounds),
                                input.options.unwrap_or_default(),
                            )
                            .unwrap(),
                    )
                });
            });
        }
        group.finish();
    }
    pub fn configuration() -> Criterion {
        Criterion::default()
            .sample_size(20)
            .warm_up_time(Duration::from_millis(500))
            .measurement_time(Duration::from_secs(1))
            .noise_threshold(0.03)
    }
}
criterion::criterion_group! { name = benches; config = measurements::configuration(); targets = measurements::run }
criterion::criterion_main!(benches);
