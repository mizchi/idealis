use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use serde_json::{Value, json};
use std::hint::black_box;
use std::time::Duration;

fn payload(count: usize, width: usize, failure: bool) -> Vec<u8> {
    let value = json!({"name":"model-observation","values":(0..width).map(|index| json!({"index":index,"enabled":index%2==0,"tuple":[index,null,"value"]})).collect::<Vec<_>>()});
    serde_json::to_vec(&json!({"version":1,"group":"wire-bench","reference":"oracle","implementations":["target"],"cases":(0..count).map(|index| json!({"id":format!("case-{index}"),"input":{"index":index},"reference":{"kind":"value","value":value},"actual":[if failure {json!({"kind":"unknown","reason":"budget"})} else {json!({"kind":"value","value":value})}]})).collect::<Vec<_>>()})).unwrap()
}
fn measurements(criterion: &mut Criterion) {
    for (name, count, width, failure) in [
        ("single", 1, 1, false),
        ("batch-32", 32, 8, false),
        ("nested-256", 1, 256, false),
        ("unknown", 1, 8, true),
    ] {
        let input = payload(count, width, failure);
        let expected = roracle::dispatch(&input);
        assert_eq!(
            expected["report"]["status"],
            if failure { "unknown" } else { "matched" }
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&roracle::dispatch_json(&input)).unwrap(),
            expected
        );
        criterion.bench_function(&format!("wire-value/{name}"), |bencher| {
            bencher.iter_batched(
                || (),
                |_| roracle::dispatch(black_box(&input)),
                BatchSize::SmallInput,
            )
        });
        criterion.bench_function(&format!("wire-bytes/{name}"), |bencher| {
            bencher.iter_batched(
                || (),
                |_| roracle::dispatch_json(black_box(&input)),
                BatchSize::SmallInput,
            )
        });
    }
}
criterion_group! { name=benches; config=Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1)); targets=measurements }
criterion_main!(benches);
