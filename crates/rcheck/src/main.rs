//! rcheck <model> [--checker auto|reference|explicit|sat|all] [--question NAME]
//! rcheck generate <model> rust|typescript|go|moonbit|ir|schema|states
//! Answers every question of an rmodel model, or one, and prints JSON. Exits
//! 1 when two definite answers disagree, 2 on a usage or model error.
use rcheck::Selection;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // rcheck generate <model> rust|typescript|schema (plan 0102).
    if let [command, path, target] = arguments.as_slice()
        && command == "generate"
    {
        let source = std::fs::read_to_string(path).unwrap_or_else(|error| fail(&error.to_string()));
        let model = rmodel::parse(&source).unwrap_or_else(|error| fail(&error.to_string()));
        let text = match target.as_str() {
            "rust" => rmodel::bindings::rust(&model, &source),
            "typescript" => rmodel::bindings::typescript(&model),
            "go" => rmodel::bindings::go(&model, &model.name.to_lowercase()),
            "moonbit" => rmodel::bindings::moonbit(&model),
            // The kernel IR as JSON (plan 0108).
            "ir" => {
                let ir = rcheck::ir::to_ir(&model).unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&ir).expect("JSON"))
            }
            "schema" => {
                let schema: serde_json::Value =
                    serde_json::from_str(&rmodel::bindings::json_schema(&model)).expect("JSON");
                format!("{}\n", serde_json::to_string_pretty(&schema).expect("JSON"))
            }
            "states" => {
                let states =
                    rcheck::conform::reachable_states(&model).unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&states).expect("JSON"))
            }
            _ => usage(),
        };
        print!("{text}");
        return;
    }
    let mut path = None;
    let mut selection = Selection::Auto;
    let mut only = None;
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--checker" => {
                selection = match rest.next().map(String::as_str) {
                    Some("auto") => Selection::Auto,
                    Some("all") => Selection::All,
                    Some(name) => Selection::Named(name.to_owned()),
                    None => usage(),
                }
            }
            "--question" => only = Some(rest.next().cloned().unwrap_or_else(|| usage())),
            _ if path.is_none() => path = Some(argument.clone()),
            _ => usage(),
        }
    }
    let path = path.unwrap_or_else(|| usage());
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| fail(&error.to_string()));
    // A `.json` input is the kernel IR an adapter wrote (plan 0108).
    let model = if path.ends_with(".json") {
        let (model, omitted) = rcheck::ir::parse(&source).unwrap_or_else(|error| fail(&error));
        for part in omitted {
            eprintln!("omitted by the adapter: {} ({})", part.name, part.reason);
        }
        model
    } else {
        rmodel::parse(&source).unwrap_or_else(|error| fail(&error.to_string()))
    };
    let names: Vec<String> = match only {
        Some(name) => vec![name],
        None => rcheck::questions(&model)
            .into_iter()
            .map(|question| question.name)
            .collect(),
    };
    let mut reports = Vec::new();
    for name in names {
        match rcheck::check(&model, &name, &selection, rcheck::Budget::default()) {
            Ok(report) => reports.push(report),
            Err(error) => fail(&error.to_string()),
        }
    }
    println!("{}", serde_json::to_string_pretty(&reports).expect("JSON"));
    if reports.iter().any(|report| report.disagreement) {
        std::process::exit(1);
    }
}

fn usage() -> ! {
    fail(
        "usage: rcheck <model> [--checker auto|reference|explicit|sat|all] [--question NAME] | rcheck generate <model> rust|typescript|go|moonbit|ir|schema|states",
    )
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}
