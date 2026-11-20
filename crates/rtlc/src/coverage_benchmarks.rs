use criterion::{Criterion, criterion_group, criterion_main};
use rtlc::SafetyBounds;
use rtlc::ir::{Expression, Scalar, Step};
use std::hint::black_box;
use std::time::Duration;

#[path = "../examples/counter.rs"]
mod counter;

fn measurements(criterion: &mut Criterion) {
    let input: serde_json::Value =
        serde_json::from_str(include_str!("../tests/coverage-bench.json")).unwrap();
    let bounds: SafetyBounds = serde_json::from_value(input["bounds"].clone()).unwrap();
    for size in input["sizes"].as_array().unwrap() {
        let size = i32::try_from(size.as_i64().unwrap()).unwrap();
        let mut program = counter::ring();
        let Step::Assign {
            value: Expression::If { condition, .. },
            ..
        } = &mut program.next[0].steps[0]
        else {
            panic!("counter transition");
        };
        **condition = Expression::Lt {
            left: Box::new(Expression::Read {
                slot: 0,
                next: false,
            }),
            right: Box::new(Expression::Literal {
                value: Scalar::Int(size - 1),
            }),
        };
        program.invariants.clear();
        let expected = program.check(bounds).unwrap();
        assert_eq!(expected.status, "complete");
        assert_eq!(expected.states, usize::try_from(size).unwrap());
        assert_eq!(
            program.check_with_coverage(bounds).unwrap().result,
            expected
        );
        criterion.bench_function(&format!("coverage/ring-{size}/off"), |bencher| {
            bencher.iter(|| black_box(&program).check(black_box(bounds)).unwrap())
        });
        criterion.bench_function(&format!("coverage/ring-{size}/on"), |bencher| {
            bencher.iter(|| {
                black_box(&program)
                    .check_with_coverage(black_box(bounds))
                    .unwrap()
            })
        });
    }
}
criterion_group! {name = benches; config = Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1)); targets = measurements}
criterion_main!(benches);
