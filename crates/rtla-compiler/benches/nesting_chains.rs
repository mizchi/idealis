use criterion::{Criterion, criterion_group, criterion_main};
use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;
use std::hint::black_box;
use std::time::Duration;

fn module(body: &str) -> String {
    format!("---- MODULE NestingChains ----\nEXTENDS Integers\nVARIABLE x\n{body}====\n")
}

fn conjunction(length: usize) -> String {
    let chain = (1..length).fold("x /= 200".to_owned(), |left, index| {
        format!("({left} /\\ x /= {})", 200 + index)
    });
    module(&format!(
        "Init == x = 0\nNext == x' = IF x < 5 THEN x + 1 ELSE 0\nInv == {chain}\n"
    ))
}

fn alternatives(length: usize) -> String {
    let chain: String = (0..length)
        .map(|index| format!("IF x = {index} THEN {} ELSE ", 100 + index))
        .collect();
    module(&format!(
        "Init == x = 0\nNext == x' = IF x < {} THEN x + 1 ELSE 0\nlabel == {chain}0\nInv == label >= 0\n",
        length + 1
    ))
}

fn measurements(criterion: &mut Criterion) {
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE").unwrap();
    let options = CompileOptions {
        irVersion: IrVersion::V2,
    };
    let bounds = SafetyBounds {
        max_depth: 100,
        max_states: 1000,
        max_work: 1_000_000,
        max_temporal_nodes: None,
    };
    for (form, build) in [
        ("paren-and", conjunction as fn(usize) -> String),
        ("if-chain", alternatives),
    ] {
        for length in [3, 8, 38] {
            let module = rtla::parse_module(&build(length)).unwrap();
            let program = compile_with_options(&module, &config, options).unwrap();
            let expected = program.check(bounds).unwrap();
            assert_eq!(expected.status, "complete", "{form}-{length}");
            criterion.bench_function(
                &format!("nesting-chains-compile/{form}-{length}"),
                |bencher| {
                    bencher.iter(|| {
                        compile_with_options(black_box(&module), black_box(&config), options)
                            .unwrap()
                    })
                },
            );
        }
    }
}
criterion_group! {name=benches; config=Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1));targets=measurements}
criterion_main!(benches);
