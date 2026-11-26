use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::{SafetyBounds, SafetyProgram};

const REQUIRED_STACK: usize = if cfg!(debug_assertions) {
    8 * 1024 * 1024
} else {
    2 * 1024 * 1024
};
const CONFIG: &str = "INIT q_init\nNEXT q_step\nINVARIANT q_inv\nCHECK_DEADLOCK FALSE\n";
const BOUNDS: SafetyBounds = SafetyBounds {
    max_depth: 64,
    max_states: 10000,
    max_work: 1_000_000,
    max_temporal_nodes: None,
};

fn hop_source(levels: usize) -> String {
    let mut lines = vec![
        "---- MODULE CallDepth ----".to_string(),
        "EXTENDS Integers".to_string(),
        "VARIABLE counter".to_string(),
        "hop0(x) == x + 1".to_string(),
    ];
    lines.extend((1..=levels).map(|level| format!("hop{level}(x) == hop{}(x) + 1", level - 1)));
    lines.extend(
        [
            "init == counter = 0",
            "inv == counter >= 0",
            &format!("step == counter' = hop{levels}(counter) % 5"),
            "q_init == init",
            "q_step == step",
            "q_inv == inv",
            "====",
        ]
        .map(String::from),
    );
    lines.join("\n") + "\n"
}

fn compiled(source: String) -> SafetyProgram {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let module = rtla::parse_module(&source).unwrap();
            let config = rtla::parse_config(CONFIG).unwrap();
            let options = CompileOptions {
                irVersion: IrVersion::V2,
            };
            compile_with_options(&module, &config, options).unwrap()
        })
        .unwrap()
        .join()
        .unwrap()
}

fn outcome(source: String) -> String {
    let program = compiled(source);
    std::thread::Builder::new()
        .stack_size(REQUIRED_STACK)
        .spawn(move || {
            let result = match program.check(BOUNDS) {
                Ok(result) => format!(
                    "{} {} {} {}",
                    result.status, result.states, result.generated, result.work
                ),
                Err(error) => format!("error {}: {}", error.code, error.message),
            };
            std::mem::forget(program);
            result
        })
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn layered_calls_complete_up_to_the_nesting_limit() {
    for (levels, expected) in [
        (40, "complete 5 6 432"),
        (41, "complete 5 6 442"),
        (76, "complete 5 6 792"),
        (83, "complete 5 6 862"),
    ] {
        assert_eq!(outcome(hop_source(levels)), expected, "hop {levels}");
    }
}

#[test]
fn layered_calls_past_the_nesting_limit_are_refused() {
    for levels in [84, 85, 168, 169] {
        assert_eq!(
            outcome(hop_source(levels)),
            "error protocol: Safety evaluation nesting exceeds 256",
            "hop {levels}"
        );
    }
}

#[test]
fn generated_layered_calls_complete() {
    let source = include_str!("../../../fixtures/call-depth/chain_N50.tla");
    assert_eq!(outcome(source.to_string()), "complete 5 6 532");
}
