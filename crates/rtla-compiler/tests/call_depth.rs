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

const WALK: &str = "counter' = IF counter < 4 THEN counter + 1 ELSE 0";

fn family_source(family: &str, levels: usize) -> String {
    let chain = |name: &str, parameter: &str, innermost: &str, layer: &dyn Fn(usize) -> String| {
        let mut lines = vec![format!("{name}0({parameter}) == {innermost}")];
        lines.extend((1..=levels).map(layer));
        lines
    };
    let (variable, declarations, init, step, inv) = match family {
        "record-hop" => (
            "state",
            chain(
                "rec",
                "s",
                "[s EXCEPT !.count = IF s.count < 4 THEN s.count + 1 ELSE 0]",
                &|level| format!("rec{level}(s) == rec{}(s)", level - 1),
            ),
            "state = [count |-> 0]".to_string(),
            format!("state' = rec{levels}(state)"),
            "state.count >= 0".to_string(),
        ),
        "let-hop" => (
            "counter",
            chain("lhop", "x", "x + 1", &|level| {
                format!(
                    "lhop{level}(x) == LET inner == lhop{}(x) + 1 IN inner",
                    level - 1
                )
            }),
            "counter = 0".to_string(),
            WALK.to_string(),
            format!("lhop{levels}(counter) >= 0"),
        ),
        "set-hop" => (
            "counter",
            chain("shop", "S", "S", &|level| {
                format!("shop{level}(S) == shop{}(S)", level - 1)
            }),
            "counter = 0".to_string(),
            WALK.to_string(),
            format!("counter \\in shop{levels}(0..4)"),
        ),
        "function-hop" => (
            "counter",
            chain("fhop", "f", "f[1]", &|level| {
                format!("fhop{level}(f) == fhop{}(f)", level - 1)
            }),
            "counter = 0".to_string(),
            WALK.to_string(),
            format!("fhop{levels}([k \\in {{1}} |-> counter]) >= 0"),
        ),
        "primed-hop" => (
            "counter",
            chain("fwd", "x", "x", &|level| {
                format!("fwd{level}(x) == fwd{}(x)", level - 1)
            }),
            "counter = 0".to_string(),
            format!(
                "counter' = (IF counter < 4 THEN counter + 1 ELSE 0) /\\ fwd{levels}(counter)' = fwd{levels}((counter + 0)')"
            ),
            "counter >= 0".to_string(),
        ),
        _ => panic!("family {family}"),
    };
    format!(
        "---- MODULE CallDepth ----\nEXTENDS Integers\nVARIABLE {variable}\n{}\ninit == {init}\ninv == {inv}\nstep == {step}\nq_init == init\nq_step == step\nq_inv == inv\n====\n",
        declarations.join("\n")
    )
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
    let mut disagreements = vec![];
    for (levels, expected) in [
        (40, "complete 5 6 432"),
        (41, "complete 5 6 442"),
        (76, "complete 5 6 792"),
        (83, "complete 5 6 862"),
        (84, "complete 1 2 176"),
        (85, "complete 5 6 882"),
        (124, "complete 1 2 256"),
        (125, "complete 5 6 1282"),
    ] {
        let observed = outcome(hop_source(levels));
        if observed != expected {
            disagreements.push(format!(
                "hop {levels}: expected `{expected}`, observed `{observed}`"
            ));
        }
    }
    assert!(disagreements.is_empty(), "\n{}", disagreements.join("\n"));
}

#[test]
fn layered_calls_past_the_nesting_limit_are_refused() {
    for levels in [126, 168, 169] {
        assert_eq!(
            outcome(hop_source(levels)),
            "error protocol: Safety evaluation nesting exceeds 256",
            "hop {levels}"
        );
    }
}

#[test]
fn forwarding_families_complete_up_to_the_nesting_limit() {
    let mut disagreements = vec![];
    for (family, slope, intercept, accepted) in [
        ("record-hop", 19, 53, [10, 124, 248]),
        ("let-hop", 20, 32, [10, 49, 83]),
        ("set-hop", 10, 32, [10, 124, 249]),
        ("function-hop", 10, 37, [10, 123, 246]),
        ("primed-hop", 20, 47, [10, 124, 248]),
    ] {
        for levels in accepted {
            let expected = format!("complete 5 6 {}", slope * levels + intercept);
            let observed = outcome(family_source(family, levels));
            if observed != expected {
                disagreements.push(format!(
                    "{family} {levels}: expected `{expected}`, observed `{observed}`"
                ));
            }
        }
    }
    assert!(disagreements.is_empty(), "\n{}", disagreements.join("\n"));
}

#[test]
fn forwarding_families_past_the_nesting_limit_are_refused() {
    for (family, levels) in [
        ("record-hop", 249),
        ("let-hop", 84),
        ("set-hop", 250),
        ("function-hop", 247),
        ("primed-hop", 249),
    ] {
        assert_eq!(
            outcome(family_source(family, levels)),
            "error protocol: Safety evaluation nesting exceeds 256",
            "{family} {levels}"
        );
    }
}

#[test]
fn generated_layered_calls_complete() {
    let source = include_str!("../../../fixtures/call-depth/chain_N50.tla");
    assert_eq!(outcome(source.to_string()), "complete 5 6 532");
}
