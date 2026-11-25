//! rcheck <model> [--checker auto|reference|explicit|sat|all] [--question NAME]
//! rcheck generate <model> rust|typescript|go|moonbit|ir|schema|states|glossary|pbt-cases|pbt-typescript|pbt-go
//! rcheck dead-code <model> [--max-states N]
//! rcheck lean-oracle <model> <file.lean> [--max-states N]
//! Answers every question of an rmodel model, or one, and prints JSON. Exits
//! 1 when two definite answers disagree, 2 on a usage or model error.
use rcheck::Selection;
mod lean_oracle;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Some((command, args)) = arguments.split_first()
        && command == "dead-code"
    {
        dead_code(args);
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "lean-oracle"
    {
        lean_oracle::run(args);
        return;
    }
    if let [command, model, request] = arguments.as_slice()
        && command == "validate"
    {
        validate_request(model, request);
        return;
    }
    // rcheck generate <model> rust|typescript|schema (plan 0102).
    if let [command, path, target] = arguments.as_slice()
        && command == "generate"
    {
        let source = rcheck::project::load_source(path).unwrap_or_else(|error| fail(&error));
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
            "pbt-cases" => {
                let cases =
                    rcheck::conform::boundary_cases(&model, rcheck::Budget::default().states)
                        .unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&cases).expect("JSON"))
            }
            "pbt-typescript" => {
                rcheck::conform::boundary_typescript(&model, rcheck::Budget::default().states)
                    .unwrap_or_else(|error| fail(&error))
            }
            "pbt-go" => rcheck::conform::boundary_go(&model, rcheck::Budget::default().states)
                .unwrap_or_else(|error| fail(&error)),
            "glossary" => format!(
                "{}\n",
                serde_json::to_string_pretty(&model.entity_definitions).expect("JSON")
            ),
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
    let (model, omitted) = load_model(&path);
    if !omitted.is_empty() {
        for part in omitted {
            eprintln!("omitted by the adapter: {} ({})", part.name, part.reason);
        }
        fail("checking requires a complete model; the IR has adapter omissions");
    }
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

fn load_model(path: &str) -> (rmodel::Model, Vec<rcheck::ir::Omitted>) {
    let source = rcheck::project::load_source(path).unwrap_or_else(|error| fail(&error));
    if path.ends_with(".json") && !rcheck::project::is_manifest(path) {
        rcheck::ir::parse(&source).unwrap_or_else(|error| fail(&error))
    } else {
        let model = rmodel::parse(&source).unwrap_or_else(|error| fail(&error.to_string()));
        (model, Vec::new())
    }
}

fn validate_request(model: &str, path: &str) {
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        config: Value,
        initial: Value,
        #[serde(default)]
        prefix: Vec<Value>,
        command: Value,
    }

    let (model, omitted) = load_model(model);
    if !omitted.is_empty() {
        fail("validate requires a complete model; the IR has adapter omissions");
    }
    let read = if path == "-" {
        serde_json::from_reader(std::io::stdin().lock())
    } else {
        let file = std::fs::File::open(path).unwrap_or_else(|error| fail(&error.to_string()));
        serde_json::from_reader(std::io::BufReader::new(file))
    };
    let request: Request = read.unwrap_or_else(|error| fail(&error.to_string()));
    let validator = rcheck::conform::Validator::new(&model).unwrap_or_else(|error| fail(&error));
    let mut session = validator
        .start(&request.config, &request.initial)
        .unwrap_or_else(|error| fail(&error.to_string()));
    for command in &request.prefix {
        session
            .replay(command)
            .unwrap_or_else(|error| fail(&error.to_string()));
    }
    let state = session
        .validate(&request.command)
        .unwrap_or_else(|error| fail(&error.to_string()));
    println!("{}", serde_json::json!({ "state": state }));
}

fn dead_code(args: &[String]) {
    use rmodel::reference::Outcome;
    let (path, max_states) = match args {
        [path] => (path, rcheck::Budget::default().states),
        [path, flag, limit] if flag == "--max-states" => (
            path,
            limit
                .parse()
                .unwrap_or_else(|_| fail("invalid --max-states")),
        ),
        _ => usage(),
    };
    let (model, omitted) = load_model(path);
    if !omitted.is_empty() {
        fail("dead-code requires a complete model; the IR has adapter omissions");
    }
    let report = rmodel::reference::analyze(&model, max_states);
    let coverage = match report.scope {
        Outcome::Holds => "complete",
        Outcome::Empty => "empty",
        _ => "unknown",
    };
    let found_dead = report
        .enabled
        .iter()
        .any(|(_, enabled)| *enabled == Some(false));
    let actions: Vec<_> = report
        .enabled
        .into_iter()
        .map(|(name, enabled)| {
            let status = match enabled {
                Some(true) => "reachable",
                Some(false) => "unreachable",
                None if report.scope == Outcome::Empty => "empty",
                None => "unknown",
            };
            serde_json::json!({ "name": name, "status": status })
        })
        .collect();
    let output = serde_json::json!({ "coverage": coverage, "actions": actions });
    println!("{}", serde_json::to_string_pretty(&output).expect("JSON"));
    if coverage != "complete" {
        std::process::exit(3);
    }
    if found_dead {
        std::process::exit(1);
    }
}

fn usage() -> ! {
    fail(
        "usage: rcheck <model> [--checker auto|reference|explicit|sat|all] [--question NAME] | rcheck generate <model> rust|typescript|go|moonbit|ir|schema|states|glossary|pbt-cases|pbt-typescript|pbt-go | rcheck dead-code <model> [--max-states N] | rcheck lean-oracle <model> <file.lean> [--max-states N] | rcheck validate <model> <request.json|->",
    )
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}
