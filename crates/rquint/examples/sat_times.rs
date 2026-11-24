//! Times the ten lowerable 0044 cases on ralloy's SAT and rtlc (plan 0095),
//! median of five, as JSON lines. Reported, not an adoption gate.
use std::time::Instant;

fn median(mut run: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..5)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[2]
}

fn main() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    for name in ["Counter", "SetFold", "Choice", "Match", "NestedFold"] {
        let ir =
            std::fs::read_to_string(root.join(format!("fixtures/quint-ir/{name}.json"))).unwrap();
        for property in ["safety", "broken"] {
            let options = rquint::Options {
                invariants: vec![property.to_owned()],
                ..rquint::Options::default()
            };
            let alloy = ralloy::Model::parse(
                &rquint::to_alloy(&ir, &options, rquint::Bounds::default()).unwrap(),
            )
            .unwrap();
            let sat = median(|| {
                alloy.analyze(0, 50_000_000).unwrap();
            });
            let tla = rquint::translate(&ir, &options).unwrap();
            let program = idealis_core::compile_tla_with_options(
                &tla.source,
                &tla.config,
                idealis_core::CompileOptions {
                    irVersion: idealis_core::IrVersion::V2,
                },
            )
            .unwrap();
            let explicit = median(|| {
                let _ = program.check(idealis_core::SafetyBounds {
                    max_depth: 1_000,
                    max_states: 100_000,
                    max_work: 10_000_000,
                    max_temporal_nodes: None,
                });
            });
            println!(
                "{{\"name\":{name:?},\"property\":{property:?},\"satMs\":{sat:.3},\"explicitMs\":{explicit:.3}}}"
            );
        }
    }
}
