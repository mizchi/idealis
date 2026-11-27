use rmodel::reference::Report;
use std::hint::black_box;

fn print(report: Report) {
    println!(
        "scope {:?}, declarations {:?}",
        report.scope, report.declarations
    );
    println!("verdicts {:?}", report.verdicts);
    println!("actions {:?}", report.enabled);
}

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [engine, mode, path, iterations, budget @ ..] = arguments.as_slice() else {
        panic!(
            "usage: rprob-benchmark reference|rprob cold|prepared <model> <iterations> [max-states]"
        );
    };
    let iterations = iterations.parse::<usize>().unwrap();
    assert!(iterations > 0);
    let max_states = match budget {
        [] => usize::MAX,
        [limit] => limit.parse().expect("max-states"),
        _ => panic!("too many arguments"),
    };
    let source = std::fs::read_to_string(path).unwrap();
    let evaluate = |model: &rmodel::Model| rmodel::reference::analyze(model, max_states);
    let mut last = None;
    match (engine.as_str(), mode.as_str()) {
        ("reference", "cold") => {
            for _ in 0..iterations {
                let model = rmodel::parse(black_box(&source)).unwrap();
                last = Some(black_box(evaluate(&model)));
            }
        }
        ("rprob", "cold") => {
            for _ in 0..iterations {
                let model = rmodel::parse(black_box(&source)).unwrap();
                let compiled = rprob::Compiled::new(&model).unwrap();
                last = Some(black_box(compiled.analyze(max_states)));
            }
        }
        ("reference", "prepared") => {
            let model = rmodel::parse(&source).unwrap();
            for _ in 0..iterations {
                last = Some(black_box(evaluate(black_box(&model))));
            }
        }
        ("rprob", "prepared") => {
            let model = rmodel::parse(&source).unwrap();
            let compiled = rprob::Compiled::new(&model).unwrap();
            for _ in 0..iterations {
                last = Some(black_box(black_box(&compiled).analyze(max_states)));
            }
        }
        _ => panic!("unknown engine or mode"),
    }
    print(last.unwrap());
}
