//! Times each corpus check on both engines (plan 0091), median of five, as
//! JSON lines. Reported, not an adoption gate.
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rmodel");
    let mut paths: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "model")
        })
        .collect();
    paths.sort();
    for path in paths {
        let model = idealis_model::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let alloy = ralloy::Model::parse(&idealis_model::to_alloy(&model)).unwrap();
        for check in &model.checks {
            let label = idealis_model::alloy_label(&check.name);
            let index = alloy
                .commands()
                .iter()
                .position(|command| command.label == label)
                .unwrap();
            let mut work = 0;
            let sat = median(|| work = alloy.analyze(index, 50_000_000).unwrap().stats.work);
            let (source, config) = idealis_model::to_tla(&model, &check.name).unwrap();
            let program = idealis_core::compile_tla_with_options(
                &source,
                &config,
                idealis_core::CompileOptions {
                    irVersion: idealis_core::IrVersion::V2,
                },
            )
            .unwrap();
            let mut states = 0;
            let explicit = median(|| {
                states = program
                    .check(idealis_core::SafetyBounds {
                        max_depth: 1_000,
                        max_states: 2_000_000,
                        max_work: 500_000_000,
                        max_temporal_nodes: None,
                    })
                    .unwrap()
                    .states
            });
            println!(
                "{{\"model\":{:?},\"check\":{:?},\"satMs\":{sat:.3},\"satWork\":{work},\"explicitMs\":{explicit:.3},\"states\":{states}}}",
                model.name, check.name
            );
        }
    }
}
