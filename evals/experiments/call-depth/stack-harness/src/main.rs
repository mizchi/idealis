#[path = "../../../../../crates/rtlc/tests/stack_programs/mod.rs"]
mod stack_programs;

use rtla_compiler::{CompileOptions, IrVersion};
use rtlc::{SafetyBounds, SafetyProgram};

const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";
const WRAPPED_CONFIG: &str = "INIT q_init\nNEXT q_step\nINVARIANT q_inv\nCHECK_DEADLOCK FALSE\n";
const BOUNDS: SafetyBounds = SafetyBounds {
    max_depth: 64,
    max_states: 10000,
    max_work: 100_000_000,
    max_temporal_nodes: None,
};

fn hop_source(count: usize) -> String {
    let mut lines = vec![
        "---- MODULE chain ----".to_string(),
        "EXTENDS Integers".to_string(),
        "VARIABLE counter".to_string(),
        "hop0(x) == x + 1".to_string(),
    ];
    lines.extend((1..=count).map(|level| format!("hop{level}(x) == hop{}(x) + 1", level - 1)));
    lines.push("Init == counter = 0".to_string());
    lines.push(format!("Next == counter' = hop{count}(counter) % 5"));
    lines.push("Inv == counter >= 0".to_string());
    lines.push("====".to_string());
    lines.join("\n") + "\n"
}

fn large<T: Send + 'static>(body: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap()
}

fn compiled(source: String) -> SafetyProgram {
    compiled_with(source, CONFIG)
}

fn compiled_with(source: String, config: &'static str) -> SafetyProgram {
    large(move || {
        let module = rtla::parse_module(&source).unwrap();
        let config = rtla::parse_config(config).unwrap();
        let options = CompileOptions {
            irVersion: IrVersion::V2,
        };
        rtla_compiler::compile_with_options(&module, &config, options).unwrap()
    })
}

fn checked_on(kibibytes: usize, program: SafetyProgram) -> String {
    std::thread::Builder::new()
        .stack_size(kibibytes * 1024)
        .spawn(move || {
            let result = program
                .check(BOUNDS)
                .map(|result| {
                    format!(
                        "{} {} {} {}",
                        result.status, result.states, result.generated, result.work
                    )
                })
                .unwrap_or_else(|error| format!("error {}: {}", error.code, error.message));
            std::mem::forget(program);
            result
        })
        .unwrap()
        .join()
        .unwrap()
}

#[cfg(feature = "oracle-peak")]
fn peak() -> String {
    format!(" peak={}", rtlc::probe_peak())
}

#[cfg(not(feature = "oracle-peak"))]
fn peak() -> String {
    String::new()
}

fn kind_program(name: String, size: usize) -> SafetyProgram {
    large(move || stack_programs::program(&name, size))
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let mode = arguments[1].as_str();
    let number = |index: usize| -> usize { arguments[index].parse().unwrap() };
    match mode {
        "kind" => {
            let program = kind_program(arguments[2].clone(), number(3));
            let outcome = stack_programs::on_stack(number(4) * 1024, program);
            println!("{mode} {outcome}{}", peak());
        }
        "hop" => {
            let program = compiled(hop_source(number(2)));
            println!("{mode} {}{}", checked_on(number(3), program), peak());
        }
        "source" => {
            let source = std::fs::read_to_string(&arguments[2]).unwrap();
            let config = if source.contains("q_init") {
                WRAPPED_CONFIG
            } else {
                CONFIG
            };
            let program = compiled_with(source, config);
            println!("{mode} {}{}", checked_on(number(3), program), peak());
        }
        "emit-kind" => {
            let program = kind_program(arguments[2].clone(), number(3));
            println!("{}", serde_json::to_string(&program).unwrap());
        }
        "emit-hop-ir" => {
            let program = compiled(hop_source(number(2)));
            println!("{}", serde_json::to_string(&program).unwrap());
        }
        "emit-hop" => print!("{}", hop_source(number(2))),
        _ => panic!("mode {mode}"),
    }
}
