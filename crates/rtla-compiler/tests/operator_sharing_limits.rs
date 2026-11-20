use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;
use serde_json::Value;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CHILD_MARKER: &str = "IDEALIS_LIMITS_ISOLATED";
const MEMORY_CAP_KIB: u64 = 256 * 1024;
const NESTING_40: &str = "expression or operator nesting exceeds 40";

fn resident_kib(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn isolated(name: &str, hard_stop: Duration, body: fn()) {
    if std::env::var_os(CHILD_MARKER).is_some() {
        body();
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads", "1"])
        .env(CHILD_MARKER, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let mut peak_kib = 0;
    let mut stopped = None;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        peak_kib = peak_kib.max(resident_kib(child.id()).unwrap_or(0));
        if peak_kib > MEMORY_CAP_KIB {
            stopped = Some("memory");
        } else if started.elapsed() > hard_stop {
            stopped = Some("time");
        }
        if stopped.is_some() {
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let output = child.wait_with_output().unwrap();
    let tail = |bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        text.chars()
            .rev()
            .take(600)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>()
    };
    assert!(
        status.success(),
        "{name}: isolated run stopped_on={stopped:?} status={status} peak={} MiB after {} ms\nstdout: {}\nstderr: {}",
        peak_kib / 1024,
        started.elapsed().as_millis(),
        tail(&output.stdout),
        tail(&output.stderr)
    );
}

fn parse(source: &str, config: &str) -> (rtla::ast::Module, rtla::ast::Config) {
    (
        rtla::parse_module(source).unwrap(),
        rtla::parse_config(config).unwrap(),
    )
}

fn v2() -> CompileOptions {
    CompileOptions {
        irVersion: IrVersion::V2,
    }
}

fn flat_conjunction(operands: usize) {
    let safe = vec!["TRUE"; operands].join(" /\\ ");
    let source = format!(
        "---- MODULE Differential ----\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nSafe == {safe}\n===="
    );
    let (module, config) = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || parse(&source, "INIT Init\nNEXT Next\nINVARIANT Safe"))
        .unwrap()
        .join()
        .unwrap();
    let outcome = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            compile_with_options(&module, &config, v2())
                .map(|_| ())
                .map_err(|error| error.message)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(
        outcome,
        Err("compiled expression nesting exceeds 128".to_string())
    );
}

#[test]
fn v2_flat_conjunction_of_20000_operands_is_refused_on_a_1_mib_stack() {
    isolated(
        "v2_flat_conjunction_of_20000_operands_is_refused_on_a_1_mib_stack",
        Duration::from_secs(120),
        || flat_conjunction(20_000),
    );
}

fn chain(length: usize) -> String {
    let mut declarations = String::from("V0 == x\n");
    for index in 1..=length {
        declarations.push_str(&format!("V{index} == <<V{}, V{}>>\n", index - 1, index - 1));
    }
    declarations
}

fn unchanged(length: usize) -> (String, &'static str) {
    (
        format!(
            "---- MODULE Differential ----\nEXTENDS Integers\nVARIABLE x\n{}Init == x = 0\nNext == UNCHANGED V{length}\nSafe == x = 0\n====\n",
            chain(length)
        ),
        "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    )
}

fn specification(length: usize) -> (String, &'static str) {
    (
        format!(
            "---- MODULE Differential ----\nEXTENDS Integers\nVARIABLE x\n{}Init == x = 0\nNext == x' = x\nSpec == Init /\\ [][Next]_V{length}\nSafe == x = 0\n====\n",
            chain(length)
        ),
        "SPECIFICATION Spec\nINVARIANT Safe\nCHECK_DEADLOCK FALSE\n",
    )
}

fn check(source: &str, config: &str) -> Result<Value, String> {
    let (module, config) = parse(source, config);
    let compiled = compile_with_options(&module, &config, v2()).map_err(|error| error.message)?;
    let result = compiled
        .check(SafetyBounds {
            max_depth: 8,
            max_states: 100,
            max_work: 100000,
            max_temporal_nodes: None,
        })
        .map_err(|error| error.message)?;
    Ok(serde_json::to_value(result).unwrap())
}

fn assert_refused_quickly(build: fn(usize) -> (String, &'static str), length: usize) {
    let (source, config) = build(length);
    let started = Instant::now();
    let outcome = check(&source, config);
    assert_eq!(outcome, Err(NESTING_40.to_string()));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "refusal took {} ms",
        started.elapsed().as_millis()
    );
}

macro_rules! refused {
    ($test:ident, $build:ident, $length:literal) => {
        #[test]
        fn $test() {
            isolated(stringify!($test), Duration::from_secs(10), || {
                assert_refused_quickly($build, $length)
            });
        }
    };
}
refused!(
    v2_unchanged_tuple_chain_of_20_is_refused_within_1_s_and_256_mib,
    unchanged,
    20
);
refused!(
    v2_unchanged_tuple_chain_of_30_is_refused_within_1_s_and_256_mib,
    unchanged,
    30
);
refused!(
    v2_specification_tuple_chain_of_20_is_refused_within_1_s_and_256_mib,
    specification,
    20
);
refused!(
    v2_specification_tuple_chain_of_30_is_refused_within_1_s_and_256_mib,
    specification,
    30
);

fn recorded(name: &str) -> Value {
    let golden: Value = serde_json::from_str(include_str!(
        "../../../fixtures/tlc-operator-sharing/expected-step3f-b20.json"
    ))
    .unwrap();
    golden["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()
        .clone()
}

fn assert_completes(name: &str, build: fn(usize) -> (String, &'static str), length: usize) {
    let (source, config) = build(length);
    let expected = recorded(name);
    let result = check(&source, config).unwrap();
    assert_eq!(result["status"], "complete", "{name}: {result}");
    assert_eq!(result["states"], expected["states"], "{name}");
    assert_eq!(result["generated"], expected["generated"], "{name}");
    assert!(result["violation"].is_null(), "{name}: {result}");
}

#[test]
fn v2_unchanged_tuple_chain_of_6_completes() {
    assert_completes("sharing-tuple-unchanged-6-step3f-b20", unchanged, 6);
}

#[test]
fn v2_specification_tuple_chain_of_6_completes() {
    assert_completes("sharing-tuple-specification-6-step3f-b20", specification, 6);
}

#[test]
fn v2_specification_tuple_chain_of_16_completes() {
    assert_completes(
        "sharing-tuple-specification-16-step3f-b20",
        specification,
        16,
    );
}

fn wide_source(empty: bool, depth: usize) -> String {
    let mut definitions = format!("V0 == {}\n", if empty { "<<>>" } else { "x" });
    for index in 0..depth {
        definitions.push_str(&format!(
            "V{} == <<{}>>\n",
            index + 1,
            vec![format!("V{index}"); 16].join(",")
        ));
    }
    let next = if empty {
        format!("UNCHANGED V{depth} /\\ UNCHANGED x")
    } else {
        "x'=x".to_owned()
    };
    let subscript = if empty {
        "x".to_owned()
    } else {
        format!("V{depth}")
    };
    format!(
        "---- MODULE Wide ----\nEXTENDS Naturals\nVARIABLE x\n{definitions}Init == x=0\nNext == {next}\nSpec == Init /\\ [][Next]_{subscript}\n===="
    )
}

#[test]
fn wide_tuple_controls_complete() {
    for empty in [false, true] {
        let result = check(
            &wide_source(empty, 3),
            "SPECIFICATION Spec\nCHECK_DEADLOCK FALSE",
        )
        .unwrap();
        assert_eq!(result["status"], "complete");
        assert_eq!(result["states"], 1);
        assert_eq!(result["generated"], 2);
        assert!(result["violation"].is_null());
    }
}

#[test]
fn wide_tuple_expansion_is_refused_before_resource_exhaustion() {
    isolated(
        "wide_tuple_expansion_is_refused_before_resource_exhaustion",
        Duration::from_secs(10),
        || {
            for empty in [false, true] {
                assert_eq!(
                    check(
                        &wide_source(empty, 6),
                        "SPECIFICATION Spec\nCHECK_DEADLOCK FALSE"
                    ),
                    Err("target expansion exceeds 524288 nodes".to_owned())
                );
            }
        },
    );
}
