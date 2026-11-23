//! Analysis time of every command of each Alloy oracle model (plan 0078).
#[path = "../tests/alloy/oracle.rs"]
mod oracle;

use criterion::{Criterion, criterion_group, criterion_main};
use ralloy::Model;
use std::{hint::black_box, time::Duration};

fn cases(criterion: &mut Criterion) {
    for (id, model) in oracle::models("core") {
        let parsed = Model::parse(&model.source).unwrap();
        let count = parsed.commands().len();
        criterion.bench_function(&format!("core-{id}"), |bench| {
            bench.iter(|| {
                for index in 0..count {
                    black_box(parsed.analyze(index, oracle::BUDGET).unwrap());
                }
            })
        });
    }
}
criterion_group! {name=benches;config=Criterion::default().warm_up_time(Duration::from_secs(1)).measurement_time(Duration::from_secs(2)).sample_size(20);targets=cases}
criterion_main!(benches);
