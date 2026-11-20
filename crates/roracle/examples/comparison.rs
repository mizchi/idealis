use roracle::{BenchConfig, Observation, OracleGroup};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut group = OracleGroup::new("sum");
    group
        .case("ten", 10_u64)
        .case("hundred", 100)
        .reference("formula", |input| {
            Observation::value(input * (input + 1) / 2)
        })
        .test_function("iterator", |input| {
            Observation::value((0..=*input).sum::<u64>())
        })
        .test_function("fold", |input| {
            let mut sum = 0;
            for number in 0..=*input {
                sum += number;
            }
            Observation::value(sum)
        });
    let report = group.check()?;
    report.assert_matched();
    let encoded = serde_json::to_string(&report)?;
    group
        .replay(&serde_json::from_str(&encoded)?)?
        .assert_matched();
    let mut tick = 0;
    group
        .bench_with_clock(
            BenchConfig {
                baseline: "iterator".into(),
                ..BenchConfig::default()
            },
            || {
                tick += 1;
                tick
            },
        )?
        .assert_matched();
    println!("standalone roracle consumer checked");
    Ok(())
}
