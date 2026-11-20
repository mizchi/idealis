use std::hint::black_box;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = std::env::args().nth(1).ok_or("case name required")?;
    let repetitions: u32 = std::env::args()
        .nth(2)
        .ok_or("repetitions required")?
        .parse()?;
    assert!((1..=1_000_000).contains(&repetitions));
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../tests/frontend-semantics-bench.json"))?;
    let case = cases
        .iter()
        .find(|case| case["name"] == name)
        .ok_or("unknown case")?;
    let module = rtla::parse_module(case["source"].as_str().unwrap())?;
    let config = rtla::parse_config(case["config"].as_str().unwrap())?;
    let program = rtla_compiler::compile_with_options(
        &module,
        &config,
        rtla_compiler::CompileOptions {
            irVersion: rtla_compiler::IrVersion::V2,
        },
    )?
    .prepare()?;
    let bounds = rtlc::SafetyBounds {
        max_depth: 1000,
        max_states: 2000,
        max_work: 1_000_000,
        max_temporal_nodes: Some(10000),
    };
    let expected = program.check(bounds)?;
    assert_eq!(expected.status, "complete");
    for _ in 0..repetitions {
        assert_eq!(black_box(&program).check(black_box(bounds))?, expected);
    }
    println!(
        "{}",
        serde_json::json!({"case":name,"repetitions":repetitions,"result":expected})
    );
    Ok(())
}
