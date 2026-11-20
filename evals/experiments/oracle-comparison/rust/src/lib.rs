use roracle::{BenchConfig, Observation, OracleGroup, Status};

#[unsafe(no_mangle)]
pub extern "C" fn oracle_probe() -> u32 {
    let mut group = OracleGroup::new("portable");
    group
        .case("four", 4_u32)
        .reference("formula", |input| {
            Observation::value(input * (input + 1) / 2)
        })
        .test_function("sum", |input| Observation::value((0..=*input).sum::<u32>()));
    let report = group.check().unwrap();
    assert_eq!(report.status, Status::Matched);
    let saved = serde_json::to_vec(&report).unwrap();
    let mut replay = OracleGroup::new("portable");
    replay
        .reference("formula", |_| panic!("replay must not call reference"))
        .test_function("sum", |input: &u32| {
            Observation::value((0..=*input).sum::<u32>())
        });
    replay
        .replay(&serde_json::from_slice(&saved).unwrap())
        .unwrap()
        .assert_matched();
    let mut tick = 0;
    let bench = group
        .bench_with_clock(
            BenchConfig {
                baseline: "sum".into(),
                samples: 4,
                warmup: 1,
                iterations: 2,
                ..BenchConfig::default()
            },
            || {
                tick += 10;
                tick
            },
        )
        .unwrap();
    bench.assert_matched();
    assert_eq!(
        bench.cases[0].measurements[0]
            .statistics
            .as_ref()
            .unwrap()
            .median_ns,
        10.0
    );
    let mut bad = OracleGroup::new("bad");
    bad.case("four", 4_u32)
        .reference("formula", |_| Observation::value(10_u32))
        .test_function("broken", |_| Observation::value(9));
    assert_eq!(bad.check().unwrap().status, Status::Mismatch);
    51
}
