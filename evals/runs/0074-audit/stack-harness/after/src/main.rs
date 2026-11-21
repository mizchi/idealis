use rtla_compiler::{CompileOptions, IrVersion};
use rtlc::{SafetyBounds, SafetyProgram};

fn chain(family: &str, n: usize) -> String {
    match family {
        "paren" => {
            let mut e = "x /= 10".to_string();
            for k in 11..10 + n {
                e = format!("({e} /\\ x /= {k})");
            }
            e
        }
        "if" => {
            let mut e = String::new();
            for k in 0..n {
                e.push_str(&format!("IF x = {} THEN FALSE ELSE ", k + 20));
            }
            e.push_str("TRUE");
            e
        }
        "flat" => (10..10 + n).map(|k| format!("x /= {k}")).collect::<Vec<_>>().join(" /\\ "),
        "case" => format!(
            "CASE {} [] OTHER -> TRUE",
            (0..n).map(|k| format!("x = {} -> FALSE", k + 20)).collect::<Vec<_>>().join(" [] ")
        ),
        "calls" => {
            let arms = |next: &str| { let mut e = String::new(); for k in 0..38 { e.push_str(&format!("IF x = {} THEN FALSE ELSE ", k + 20)); } e.push_str(next); e };
            let _ = arms;
            return String::new();
        }
        _ => panic!("family"),
    }
}

fn source(family: &str, n: usize) -> String {
    if family == "calls" {
        let arms = |next: String| { let mut e = String::new(); for k in 0..38 { e.push_str(&format!("IF x = {} THEN FALSE ELSE ", k + 20)); } e.push_str(&next); e };
        let mut defs = String::new();
        for d in 0..n { let next = if d + 1 == n { "TRUE".to_string() } else { format!("D{}(k)", d + 1) }; defs.push_str(&format!("D{d}(k) == {}\n", arms(next))); }
        return format!("---- MODULE M ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 5 THEN x + 1 ELSE 0\n{defs}Inv == D0(1)\n====\n");
    }
    format!(
        "---- MODULE M ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 5 THEN x + 1 ELSE 0\nInv == {}\n====\n",
        chain(family, n)
    )
}

const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
const BOUNDS: SafetyBounds = SafetyBounds { max_depth: 64, max_states: 10000, max_work: 1000000, max_temporal_nodes: None };

fn options(version: u8) -> CompileOptions {
    CompileOptions { irVersion: if version == 1 { IrVersion::V1 } else { IrVersion::V2 } }
}

fn big<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(1 << 30).spawn(f).unwrap().join().unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let stage = args[1].clone();
    let family = args[2].clone();
    let n: usize = args[3].parse().unwrap();
    let stack_kib: usize = args[4].parse().unwrap();
    let version: u8 = args.get(5).map(|v| v.parse().unwrap()).unwrap_or(2);
    let src = source(&family, n);
    let fam = family.clone();
    if stage == "eval" {
        let src2 = src.clone();
        let ready = big(move || {
            let module = rtla::parse_module(&src2).unwrap();
            let config = rtla::parse_config(CONFIG).unwrap();
            rtla_compiler::compile_with_options(&module, &config, options(version)).unwrap().prepare().unwrap()
        });
        let outcome = std::thread::Builder::new().stack_size(stack_kib * 1024).spawn(move || {
            let result = ready.check(BOUNDS).map(|r| format!("{} {} {} {}", r.status, r.states, r.generated, r.work)).unwrap_or_else(|e| e.message);
            std::mem::forget(ready);
            format!("eval {result}")
        }).unwrap().join().unwrap();
        println!("{outcome}");
        return;
    }
    let prepared: Option<(String, Option<SafetyProgram>)> = match stage.as_str() {
        "parse" | "compile" => None,
        _ => Some(big(move || {
            let module = rtla::parse_module(&src).unwrap();
            let config = rtla::parse_config(CONFIG).unwrap();
            let program = rtla_compiler::compile_with_options(&module, &config, options(version)).unwrap();
            let json = serde_json::to_string(&program).unwrap();
            (json, Some(program))
        })),
    };
    let src = source(&fam, n);
    let outcome = std::thread::Builder::new()
        .stack_size(stack_kib * 1024)
        .spawn(move || -> String {
            match stage.as_str() {
                "parse" => {
                    let module = rtla::parse_module(&src).map_err(|e| e.to_string());
                    let ok = module.is_ok();
                    std::mem::forget(module);
                    format!("parse {ok}")
                }
                "compile" => {
                    let module = big({ let src = src.clone(); move || rtla::parse_module(&src).unwrap() });
                    let config = rtla::parse_config(CONFIG).unwrap();
                    let program = rtla_compiler::compile_with_options(&module, &config, options(version));
                    let ok = program.as_ref().map(|_| "ok".to_string()).unwrap_or_else(|e| e.to_string());
                    std::mem::forget(program);
                    std::mem::forget(module);
                    format!("compile {ok}")
                }
                "serialize" => {
                    let (_, program) = prepared.unwrap();
                    let program = program.unwrap();
                    let json = serde_json::to_string(&program).unwrap();
                    std::mem::forget(program);
                    format!("serialize {}", json.len())
                }
                "decode" => {
                    let (json, program) = prepared.unwrap();
                    std::mem::forget(program);
                    let mut de = serde_json::Deserializer::from_str(&json);
                    de.disable_recursion_limit();
                    let program: SafetyProgram = serde::Deserialize::deserialize(&mut de).unwrap();
                    std::mem::forget(program);
                    "decode ok".into()
                }
                "json" => {
                    let (json, program) = prepared.unwrap();
                    std::mem::forget(program);
                    format!("json {json}")
                }
                "check" => {
                    let (_, program) = prepared.unwrap();
                    let program = program.unwrap();
                    let result = program.check(BOUNDS).map(|r| format!("{} {} {} {}", r.status, r.states, r.generated, r.work)).unwrap_or_else(|e| e.message);
                    std::mem::forget(program);
                    format!("check {result}")
                }
                _ => panic!("stage"),
            }
        })
        .unwrap()
        .join()
        .unwrap();
    println!("{outcome}");
}
