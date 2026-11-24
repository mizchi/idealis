#[path = "../../../../../crates/rtlc/tests/stack_programs/mod.rs"]
mod stack_programs;

use rtla_compiler::{CompileOptions, IrVersion};
use rtlc::{SafetyBounds, SafetyProgram};

const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
const BOUNDS: SafetyBounds = SafetyBounds {
    max_depth: 64,
    max_states: 10000,
    max_work: 1000000,
    max_temporal_nodes: None,
};

fn chain(family: &str, length: usize) -> String {
    match family {
        "paren" => (11..10 + length).fold("x /= 10".to_string(), |left, k| {
            format!("({left} /\\ x /= {k})")
        }),
        "flat" => (10..10 + length)
            .map(|k| format!("x /= {k}"))
            .collect::<Vec<_>>()
            .join(" /\\ "),
        "if" => format!(
            "{}TRUE",
            (0..length)
                .map(|k| format!("IF x = {} THEN FALSE ELSE ", k + 20))
                .collect::<String>()
        ),
        "case" => format!(
            "CASE {} [] OTHER -> TRUE",
            (0..length)
                .map(|k| format!("x = {} -> FALSE", k + 20))
                .collect::<Vec<_>>()
                .join(" [] ")
        ),
        _ => panic!("family {family}"),
    }
}

fn source(family: &str, length: usize) -> String {
    format!(
        "---- MODULE M ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 5 THEN x + 1 ELSE 0\nInv == {}\n====\n",
        chain(family, length)
    )
}

fn large<T: Send + 'static>(body: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap()
}

fn compiled(family: String, length: usize, version: u8) -> SafetyProgram {
    large(move || {
        let module = rtla::parse_module(&source(&family, length)).unwrap();
        let config = rtla::parse_config(CONFIG).unwrap();
        let options = CompileOptions {
            irVersion: if version == 1 {
                IrVersion::V1
            } else {
                IrVersion::V2
            },
        };
        rtla_compiler::compile_with_options(&module, &config, options).unwrap()
    })
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let mode = arguments[1].as_str();
    let name = arguments[2].clone();
    let size: usize = arguments[3].parse().unwrap();
    let kib: usize = arguments[4].parse().unwrap();
    let outcome = match mode {
        "kind" => {
            let program = large(move || stack_programs::program(&name, size));
            stack_programs::on_stack(kib * 1024, program)
        }
        "chain" => {
            let version: u8 = arguments[5].parse().unwrap();
            let program = compiled(name, size, version);
            std::thread::Builder::new()
                .stack_size(kib * 1024)
                .spawn(move || {
                    let result = program
                        .check(BOUNDS)
                        .map(|r| format!("{} {} {} {}", r.status, r.states, r.generated, r.work))
                        .unwrap_or_else(|e| format!("error {}", e.message));
                    std::mem::forget(program);
                    result
                })
                .unwrap()
                .join()
                .unwrap()
        }
        _ => panic!("mode {mode}"),
    };
    println!("{mode} {outcome}");
}
