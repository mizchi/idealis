//! idealis-check <model> [--checker auto|reference|explicit|sat|rprob|all] [--question NAME] [--max-states N] [--sat-steps N]
//! idealis-check capabilities <model>
//! idealis-check refinements <model> [--type NAME] [--max-work N]
//! idealis-check generate <model> rust|typescript|go|moonbit|ir|schema|states|glossary|glossary-localized|dictionary|pbt-cases|pbt-typescript|pbt-go|pbt-data-cases|pbt-data-typescript|branch-map
//! idealis-check generate <project> rust|typescript|go|moonbit|schema --public
//! idealis-check component <component.json>
//! idealis-check contracts <model.project.json> [--max-states N] [--max-arguments N]
//! idealis-check dead-code <model> [--instances ACTION] [--max-states N] [--max-arguments N]
//! idealis-check coverage <model> [--instances] [--branch-map PATH] [--max-states N] [--max-arguments N]
//! idealis-check deadlock <model> [--unless REACHABLE] [--ignore-stutter] [--max-states N] [--max-arguments N]
//! idealis-check branches <model> <action> [--branch-map PATH] [--max-states N] [--max-arguments N]
//! idealis-check witness <model> <question> [--checker rprob] [--max-states N] [--max-arguments N]
//! idealis-check witness-step <model> <action> <predicate> [--max-states N] [--max-arguments N]
//! idealis-check observation-pair <model> <request.json|-> [--max-states N]
//! idealis-check lean-oracle <model> <file.lean> [--max-states N]
//! idealis-check record-itf <source-label> <recording.json|->
//! idealis-check replay-itf <model> <trace.itf.json|->
//! idealis-check checker-request <model> <question> [--max-states N]
//! idealis-check checker-verify <request.json> <reply.json|->
//! idealis-check traceability <spec.json> (--rule ID | --check MODEL CHECK) [--root DIR] [--document PATH] [--max-states N]
//! idealis-check impact <model> <state> [--spec SPEC.json [--root DIR]]
//! Answers every question of an Idealis model, or one, and prints JSON. Exits
//! 1 when two definite answers disagree, 2 on a usage or model error.
use idealis_check::{Selection, Verdict};
mod capabilities;
mod impact;
mod lean_oracle;
mod traceability;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if matches!(arguments.as_slice(), [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "capabilities"
    {
        let [path] = args else { usage() };
        let (model, omitted) = load_model(path);
        for part in &omitted {
            eprintln!("omitted by the adapter: {} ({})", part.name, part.reason);
        }
        if !omitted.is_empty() {
            fail("capabilities requires a complete model; the IR has adapter omissions");
        }
        let report = capabilities::report(&model);
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        return;
    }
    if let [command, path, flags @ ..] = arguments.as_slice()
        && command == "refinements"
    {
        check_refinements(path, flags);
        return;
    }
    if let [command, spec, flags @ ..] = arguments.as_slice()
        && command == "traceability"
    {
        traceability::run(spec, flags);
        return;
    }
    if let [command, path, state, flags @ ..] = arguments.as_slice()
        && command == "impact"
    {
        let report = impact::run(path, state, flags).unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "component"
    {
        let [path] = args else { usage() };
        let report =
            idealis_check::project::check_component(path).unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        return;
    }
    if let [command, path, flags @ ..] = arguments.as_slice()
        && command == "contracts"
    {
        let result =
            idealis_check::project::check_contracts(path, search_limits(flags, None).budget())
                .unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&result).expect("JSON"));
        match result["status"].as_str() {
            Some("matched") => return,
            Some("mismatch") => std::process::exit(1),
            _ => std::process::exit(3),
        }
    }
    if let Some((command, args)) = arguments.split_first() {
        use idealis_check::conform::external;
        match (command.as_str(), args) {
            ("checker-request", [path, question, flags @ ..]) => {
                let limits = search_limits(flags, None);
                if limits.arguments.is_some() {
                    usage();
                }
                let (model, omitted) = load_model(path);
                if !omitted.is_empty() {
                    fail("checker-request requires a complete model; the IR has adapter omissions");
                }
                let budget = idealis_check::Budget {
                    states: limits.budget().states,
                    ..idealis_check::Budget::default()
                };
                let request = external::request(&model, question, budget)
                    .unwrap_or_else(|error| fail(&error));
                println!("{}", serde_json::to_string_pretty(&request).expect("JSON"));
                return;
            }
            ("checker-verify", [request, reply]) if request != "-" => {
                let report = external::verify(read_request(request), read_request(reply))
                    .unwrap_or_else(|error| fail(&error));
                println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
                if report["verdict"] == "error" {
                    std::process::exit(2);
                }
                if report["status"] != "verified" {
                    std::process::exit(3);
                }
                return;
            }
            ("checker-request" | "checker-verify", _) => usage(),
            _ => {}
        }
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "dead-code"
    {
        dead_code(args);
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "deadlock"
    {
        deadlock(args);
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "coverage"
    {
        let [path, flags @ ..] = args else { usage() };
        let limits = parse_search_limits(flags, Some("--branch-map"), Some("--instances"));
        let report = idealis_check::conform::coverage_report_with_instances(
            path,
            limits.budget(),
            limits.selected,
            limits.flag_set,
        )
        .unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        match report["status"].as_str() {
            Some("clear") => return,
            Some("unreachable" | "expectation_mismatch") => std::process::exit(1),
            _ => std::process::exit(3),
        }
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "branches"
    {
        let [path, action, flags @ ..] = args else {
            usage()
        };
        let limits = search_limits(flags, Some("--branch-map"));
        let report = idealis_check::conform::update_branch_report_with_map(
            path,
            action,
            limits.budget(),
            limits.selected,
        )
        .unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "lean-oracle"
    {
        lean_oracle::run(args);
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && matches!(command.as_str(), "witness" | "witness-step")
    {
        let (path, target, predicate, flags) = match (command.as_str(), args) {
            ("witness", [path, question, flags @ ..]) => (path, question, None, flags),
            ("witness-step", [path, action, predicate, flags @ ..]) => {
                (path, action, Some(predicate), flags)
            }
            _ => usage(),
        };
        let limits = search_limits(flags, predicate.is_none().then_some("--checker"));
        if limits.selected.is_some_and(|checker| checker != "rprob") {
            fail("witness --checker accepts rprob");
        }
        let (model, omitted) = load_model(path);
        if !omitted.is_empty() {
            fail(&format!(
                "{command} requires a complete model; the IR has adapter omissions"
            ));
        }
        if predicate.is_none()
            && limits.arguments.is_some()
            && model.temporal.iter().any(|check| &check.name == target)
        {
            fail("--max-arguments applies to finite witness search");
        }
        let budget = limits.budget();
        let report = match predicate {
            Some(predicate) => idealis_check::conform::transition_witness_with_budget(
                &model, target, predicate, budget,
            ),
            None if limits.selected.is_some() && limits.arguments.is_some() => {
                idealis_check::conform::rprob_witness_with_budget(&model, target, budget)
            }
            None if limits.selected.is_some() => {
                idealis_check::conform::rprob_witness(&model, target, budget.states)
            }
            None if limits.arguments.is_none() => {
                idealis_check::conform::witness(&model, target, budget.states)
            }
            None => idealis_check::conform::witness_with_budget(&model, target, budget),
        }
        .unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        return;
    }
    if let [command, model, request] = arguments.as_slice()
        && command == "validate"
    {
        validate_request(model, request);
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && matches!(command.as_str(), "record-itf" | "replay-itf")
    {
        use idealis_check::conform::itf;
        let [source, input] = args else { usage() };
        if command == "record-itf" {
            let trace =
                itf::record_itf(source, &read_request(input)).unwrap_or_else(|error| fail(&error));
            println!("{}", serde_json::to_string_pretty(&trace).expect("JSON"));
        } else {
            let (model, omitted) = load_model(source);
            if !omitted.is_empty() {
                fail("replay-itf requires a complete model; the IR has adapter omissions");
            }
            let report =
                itf::replay_itf(&model, &read_request(input)).unwrap_or_else(|error| fail(&error));
            println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
            if matches!(report, itf::ReplayResult::Mismatch { .. }) {
                std::process::exit(1);
            }
        }
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "observation-pair"
    {
        let (path, request, budget) = match args {
            [path, request] => (path, request, idealis_check::Budget::default().states),
            [path, request, flag, limit] if flag == "--max-states" => (
                path,
                request,
                limit
                    .parse()
                    .unwrap_or_else(|_| fail("invalid --max-states")),
            ),
            _ => usage(),
        };
        let (model, omitted) = load_model(path);
        if !omitted.is_empty() {
            fail("observation-pair requires a complete model; the IR has adapter omissions");
        }
        let query = read_request(request);
        let report = idealis_check::conform::observation_pair(&model, &query, budget)
            .unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        return;
    }
    if let [command, path, target, flags @ ..] = arguments.as_slice()
        && command == "generate"
    {
        let public = match flags {
            [] => false,
            [flag] if flag == "--public" => true,
            _ => usage(),
        };
        if target == "branch-map" && !public {
            let map = idealis_check::conform::generate_branch_map(path)
                .unwrap_or_else(|error| fail(&error));
            println!("{}", serde_json::to_string_pretty(&map).expect("JSON"));
            return;
        }
        if target == "dictionary" && !public {
            let report = idealis_check::project::dictionary_report(path)
                .unwrap_or_else(|error| fail(&error));
            println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
            return;
        }
        let (model, source) = if public {
            let (mut model, exports) = idealis_check::project::load_public_binding_source(path)
                .unwrap_or_else(|error| fail(&error));
            model
                .actions
                .retain(|action| exports.contains(&action.name));
            (model, None)
        } else if matches!(target.as_str(), "glossary" | "glossary-localized") {
            (load_model(path).0.into_model(), None)
        } else {
            let (model, source) = idealis_check::project::load_model_source(path)
                .unwrap_or_else(|error| fail(&error));
            (model, Some(source))
        };
        let text = match target.as_str() {
            "rust" => match source.as_deref() {
                Some(source) => idealis_model::bindings::rust(&model, source),
                None => idealis_model::bindings::rust_api(&model),
            },
            "typescript" => idealis_model::bindings::typescript(&model),
            "go" => idealis_model::bindings::go(&model, &model.name.to_lowercase()),
            "moonbit" => idealis_model::bindings::moonbit(&model),
            "schema" => {
                let schema: serde_json::Value =
                    serde_json::from_str(&idealis_model::bindings::json_schema(&model))
                        .expect("JSON");
                format!("{}\n", serde_json::to_string_pretty(&schema).expect("JSON"))
            }
            _ if public => fail("--public supports rust, typescript, go, moonbit, and schema"),
            // The kernel IR as JSON (plan 0108).
            "ir" => {
                let ir = idealis_check::ir::to_ir(&model).unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&ir).expect("JSON"))
            }
            "states" => {
                let states = idealis_check::conform::reachable_states(&model)
                    .unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&states).expect("JSON"))
            }
            "pbt-data-cases" => {
                let cases = idealis_check::conform::data_boundary_cases(&model)
                    .unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&cases).expect("JSON"))
            }
            "pbt-data-typescript" => idealis_check::conform::data_boundary_typescript(&model)
                .unwrap_or_else(|error| fail(&error)),
            "pbt-cases" => {
                let cases = idealis_check::conform::boundary_cases(
                    &model,
                    idealis_check::Budget::default().states,
                )
                .unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&cases).expect("JSON"))
            }
            "pbt-typescript" => idealis_check::conform::boundary_typescript(
                &model,
                idealis_check::Budget::default().states,
            )
            .unwrap_or_else(|error| fail(&error)),
            "pbt-go" => {
                idealis_check::conform::boundary_go(&model, idealis_check::Budget::default().states)
                    .unwrap_or_else(|error| fail(&error))
            }
            "glossary" => format!(
                "{}\n",
                serde_json::to_string_pretty(&model.entity_definitions).expect("JSON")
            ),
            "glossary-localized" => {
                use std::collections::BTreeMap;
                let mut glossary: BTreeMap<&str, BTreeMap<&str, &str>> = model
                    .entity_translations
                    .iter()
                    .map(|(name, translations)| {
                        (
                            name.as_str(),
                            translations
                                .iter()
                                .map(|(tag, text)| (tag.as_str(), text.as_str()))
                                .collect(),
                        )
                    })
                    .collect();
                for (name, description) in &model.entity_definitions {
                    glossary
                        .entry(name)
                        .or_default()
                        .insert("default", description);
                }
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&glossary).expect("JSON")
                )
            }
            _ => usage(),
        };
        print!("{text}");
        return;
    }
    let mut path = None;
    let mut selection = Selection::Auto;
    let mut only = None;
    let mut expectation = None;
    let mut budget = idealis_check::Budget::default();
    let mut sat_steps = None;
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
            "--expect" => {
                if expectation.is_some() {
                    fail("--expect can be specified once");
                }
                let name = rest.next().map(String::as_str).unwrap_or_else(|| usage());
                let verdict = match name {
                    "holds" => Verdict::Holds,
                    "violated" => Verdict::Violated,
                    "reachable" => Verdict::Reachable,
                    "unreachable" => Verdict::Unreachable,
                    _ => fail("--expect needs holds, violated, reachable, or unreachable"),
                };
                expectation = Some((name, verdict));
            }
            "--max-states" => {
                budget.states = rest
                    .next()
                    .and_then(|limit| limit.parse().ok())
                    .unwrap_or_else(|| fail("invalid --max-states"));
            }
            "--sat-steps" if sat_steps.is_none() => {
                sat_steps = Some(
                    rest.next()
                        .and_then(|limit| limit.parse::<usize>().ok())
                        .filter(|&steps| steps > 0)
                        .unwrap_or_else(|| fail("invalid --sat-steps")),
                );
            }
            "--sat-steps" => fail("--sat-steps can be specified once"),
            _ if path.is_none() => path = Some(argument.clone()),
            _ => usage(),
        }
    }
    let path = path.unwrap_or_else(|| usage());
    if expectation.is_some() && only.is_none() {
        fail("--expect requires --question");
    }
    let (model, omitted) = load_model(&path);
    if !omitted.is_empty() {
        for part in omitted {
            eprintln!("omitted by the adapter: {} ({})", part.name, part.reason);
        }
        fail("checking requires a complete model; the IR has adapter omissions");
    }
    let mut model = model.into_model();
    if let Some(steps) = sat_steps {
        model.scope.steps = Some(steps);
    }
    let names: Vec<String> = match only {
        Some(name) => vec![name],
        None => idealis_check::questions(&model)
            .into_iter()
            .map(|question| question.name)
            .collect(),
    };
    let mut reports = Vec::new();
    for name in names {
        match idealis_check::check(&model, &name, &selection, budget) {
            Ok(report) => reports.push(report),
            Err(error) => fail(&error.to_string()),
        }
    }
    println!("{}", serde_json::to_string_pretty(&reports).expect("JSON"));
    if reports.iter().any(|report| report.disagreement) {
        std::process::exit(1);
    }
    if let Some((name, expected)) = expectation {
        enforce_expectation(&reports[0], name, expected);
    }
}

fn enforce_expectation(report: &idealis_check::Report, name: &str, expected: Verdict) {
    if let Some(failed) = report.answers.iter().find(|a| a.verdict == Verdict::Error) {
        fail(&format!(
            "{}: checker {} failed",
            report.question, failed.checker
        ));
    }
    let mut matched = false;
    for answer in report.answers.iter().filter(|a| a.definite()) {
        if answer.verdict != expected {
            eprintln!(
                "{}: checker {} did not return {name}",
                report.question, answer.checker
            );
            std::process::exit(1);
        }
        matched = true;
    }
    if !matched {
        eprintln!("{}: no definitive {name} result", report.question);
        std::process::exit(3);
    }
}

fn check_refinements(path: &str, flags: &[String]) {
    let mut selected = None;
    let mut budget = None;
    let mut flags = flags.iter().map(String::as_str);
    while let Some(flag) = flags.next() {
        let value = flags.next().unwrap_or_else(|| usage());
        match flag {
            "--type" if selected.is_none() => selected = Some(value),
            "--max-work" if budget.is_none() => {
                budget = Some(value.parse().unwrap_or_else(|_| fail("invalid --max-work")));
            }
            _ => usage(),
        }
    }
    let budget = budget.unwrap_or(idealis_check::refinements::DEFAULT_WORK);
    let report = if path.ends_with(".json") && !idealis_check::project::is_manifest(path) {
        let (model, _) = load_model(path);
        // Adapter omissions apply to properties. Record declarations are validated on import.
        idealis_check::refinements::check(&model, selected, budget)
    } else {
        idealis_check::project::check_refinements(path, selected, budget)
    }
    .unwrap_or_else(|error| fail(&error));
    println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
    let code = report.exit_code();
    if code != 0 {
        std::process::exit(code);
    }
}

fn load_model(path: &str) -> (idealis_model::CheckedModel, Vec<idealis_check::ir::Omitted>) {
    if path.ends_with(".json") && !idealis_check::project::is_manifest(path) {
        let source = idealis_check::project::load_source(path).unwrap_or_else(|error| fail(&error));
        idealis_check::ir::parse_checked(&source).unwrap_or_else(|error| fail(&error))
    } else {
        let (model, _) = idealis_check::project::load_checked_model_source(path)
            .unwrap_or_else(|error| fail(&error));
        (model, Vec::new())
    }
}

fn validate_request(model: &str, path: &str) {
    use serde::Deserialize;
    use serde_json::Value;

    fn present_command<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Value>, D::Error> {
        Value::deserialize(deserializer).map(Some)
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        config: Value,
        initial: Value,
        #[serde(default)]
        prefix: Vec<Value>,
        #[serde(default, deserialize_with = "present_command")]
        command: Option<Value>,
        #[serde(default)]
        derive: Vec<String>,
    }

    let (model, omitted) = load_model(model);
    if !omitted.is_empty() {
        fail("validate requires a complete model; the IR has adapter omissions");
    }
    let request: Request = read_request(path);
    let validator =
        idealis_check::conform::Validator::new(&model).unwrap_or_else(|error| fail(&error));
    let mut session = validator
        .start(&request.config, &request.initial)
        .unwrap_or_else(|error| fail(&error.to_string()));
    for command in &request.prefix {
        session
            .replay(command)
            .unwrap_or_else(|error| fail(&error.to_string()));
    }
    let response = match request.command {
        Some(command) if request.derive.is_empty() => session
            .validate(&command)
            .map(|state| serde_json::json!({ "state": state })),
        Some(command) => session.validate_with_derivations(&command, &request.derive),
        None if request.derive.is_empty() => Ok(serde_json::json!({ "state": session.snapshot() })),
        None => session
            .snapshot_derivations(&request.derive)
            .map(|derived| serde_json::json!({ "state": session.snapshot(), "derived": derived })),
    }
    .unwrap_or_else(|error| fail(&error.to_string()));
    println!("{response}");
}

fn read_request<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let read = if path == "-" {
        serde_json::from_reader(std::io::stdin().lock())
    } else {
        let file = std::fs::File::open(path).unwrap_or_else(|error| fail(&error.to_string()));
        serde_json::from_reader(std::io::BufReader::new(file))
    };
    read.unwrap_or_else(|error| fail(&error.to_string()))
}

#[derive(Default)]
struct SearchLimits<'a> {
    states: Option<usize>,
    arguments: Option<u64>,
    selected: Option<&'a str>,
    flag_set: bool,
}

impl SearchLimits<'_> {
    fn budget(&self) -> idealis_model::reference::SearchBudget {
        let states = self
            .states
            .unwrap_or_else(|| idealis_check::Budget::default().states);
        let mut budget = idealis_model::reference::SearchBudget::for_states(states);
        if let Some(arguments) = self.arguments {
            budget.arguments = arguments;
        }
        budget
    }
}

fn search_limits<'a>(flags: &'a [String], selector: Option<&str>) -> SearchLimits<'a> {
    parse_search_limits(flags, selector, None)
}

fn parse_search_limits<'a>(
    flags: &'a [String],
    selector: Option<&str>,
    toggle: Option<&str>,
) -> SearchLimits<'a> {
    let mut limits = SearchLimits::default();
    let mut flags = flags.iter().map(String::as_str);
    while let Some(flag) = flags.next() {
        if Some(flag) == toggle && !limits.flag_set {
            limits.flag_set = true;
            continue;
        }
        let value = flags.next().unwrap_or_else(|| usage());
        match flag {
            "--max-states" if limits.states.is_none() => {
                limits.states = Some(
                    value
                        .parse()
                        .unwrap_or_else(|_| fail("invalid --max-states")),
                );
            }
            "--max-arguments" if limits.arguments.is_none() => {
                limits.arguments = Some(
                    value
                        .parse()
                        .unwrap_or_else(|_| fail("invalid --max-arguments")),
                );
            }
            flag if Some(flag) == selector && limits.selected.is_none() => {
                limits.selected = Some(value);
            }
            _ => usage(),
        }
    }
    limits
}

fn deadlock(args: &[String]) {
    let Some((path, flags)) = args.split_first() else {
        usage();
    };
    use idealis_model::reference::DeadlockMode;
    let limits = parse_search_limits(flags, Some("--unless"), Some("--ignore-stutter"));
    let (model, omitted) = load_model(path);
    if !omitted.is_empty() {
        fail("deadlock requires a complete model; the IR has adapter omissions");
    }
    let mode = if limits.flag_set {
        DeadlockMode::IgnoreStutter
    } else {
        DeadlockMode::Guards
    };
    let report = idealis_check::conform::deadlock_witness_with_mode(
        &model,
        limits.selected,
        limits.budget(),
        mode,
    )
    .unwrap_or_else(|error| fail(&error));
    println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
    match report["verdict"].as_str() {
        Some("unreachable") => (),
        Some("reachable") => std::process::exit(1),
        _ => std::process::exit(3),
    }
}

fn dead_code(args: &[String]) {
    use idealis_model::reference::Outcome;
    let Some((path, flags)) = args.split_first() else {
        usage();
    };
    let limits = search_limits(flags, Some("--instances"));
    let budget = limits.budget();
    let (model, omitted) = load_model(path);
    if !omitted.is_empty() {
        fail("dead-code requires a complete model; the IR has adapter omissions");
    }
    let (report, instances) = match limits.selected {
        Some(action) => {
            const MAX_INSTANCES: usize = 4096;
            let (report, instances) = idealis_model::reference::analyze_instances_with_budget(
                &model,
                action,
                budget,
                MAX_INSTANCES,
            )
            .unwrap_or_else(|error| fail(&error));
            (report, Some(instances))
        }
        None => (
            idealis_model::reference::analyze_actions_with_budget(&model, budget),
            None,
        ),
    };
    let coverage = match report.scope {
        Outcome::Holds => "complete",
        Outcome::Empty => "empty",
        _ => "unknown",
    };
    let diagnostics = idealis_check::reference_diagnostics(report.scope, report.stop.as_ref());
    let status = |enabled| match enabled {
        Some(true) => "reachable",
        Some(false) => "unreachable",
        None if report.scope == Outcome::Empty => "empty",
        None => "unknown",
    };
    let mut found_dead = report
        .enabled
        .iter()
        .any(|(_, enabled)| *enabled == Some(false));
    let actions: Vec<_> = report
        .enabled
        .into_iter()
        .map(|(name, enabled)| serde_json::json!({ "name": name, "status": status(enabled) }))
        .collect();
    let mut output = serde_json::json!({ "coverage": coverage, "actions": actions });
    if let Some(instances) = instances {
        found_dead |= instances
            .iter()
            .any(|instance| instance.enabled == Some(false));
        output["instances"] = serde_json::json!(instances.into_iter().map(|instance| {
            serde_json::json!({
                "action": instance.call.action,
                "args": idealis_check::conform::arguments(&instance.call.args).collect::<serde_json::Map<_, _>>(),
                "status": status(instance.enabled),
            })
        }).collect::<Vec<_>>());
    }
    if !diagnostics.is_empty() {
        output["diagnostics"] = serde_json::json!(diagnostics);
    }
    println!("{}", serde_json::to_string_pretty(&output).expect("JSON"));
    if coverage != "complete" {
        std::process::exit(3);
    }
    if found_dead {
        std::process::exit(1);
    }
}

const USAGE: &str = "Usage:
  idealis-check <model> [--checker auto|reference|explicit|sat|rprob|all] [--question NAME] [--max-states N] [--sat-steps N]
         [--expect holds|violated|reachable|unreachable]
  idealis-check capabilities <model>
  idealis-check refinements <model> [--type NAME] [--max-work N]
  idealis-check generate <model> <target>
  idealis-check generate <project> rust|typescript|go|moonbit|schema --public
  idealis-check component <component.json>
  idealis-check contracts <model.project.json> [--max-states N] [--max-arguments N]
  idealis-check dead-code <model> [--instances ACTION] [--max-states N] [--max-arguments N]
  idealis-check coverage <model> [--instances] [--branch-map PATH] [--max-states N] [--max-arguments N]
  idealis-check deadlock <model> [--unless REACHABLE] [--ignore-stutter] [--max-states N] [--max-arguments N]
  idealis-check branches <model> <action> [--branch-map PATH] [--max-states N] [--max-arguments N]
  idealis-check witness <model> <question> [--checker rprob] [--max-states N] [--max-arguments N]
  idealis-check witness-step <model> <action> <predicate> [--max-states N] [--max-arguments N]
  idealis-check observation-pair <model> <request.json|-> [--max-states N]
  idealis-check lean-oracle <model> <file.lean> [--max-states N]
  idealis-check validate <model> <request.json|->
  idealis-check record-itf <source-label> <recording.json|->
  idealis-check replay-itf <model> <trace.itf.json|->
  idealis-check checker-request <model> <question> [--max-states N]
  idealis-check checker-verify <request.json> <reply.json|->
  idealis-check traceability <spec.json> (--rule ID | --check MODEL CHECK)
         [--root DIR] [--document PATH] [--max-states N]
  idealis-check impact <model> <state> [--spec SPEC.json [--root DIR]]

Model inputs: .ideal source (.model also accepted), .project.json project (.rmodel.json also accepted),
              or JSON IR (except for generate).
Generation targets: rust, typescript, go, moonbit, ir, schema, states, glossary, glossary-localized, dictionary,
                    pbt-cases, pbt-typescript, pbt-go, pbt-data-cases, pbt-data-typescript, branch-map";

fn usage() -> ! {
    fail(USAGE)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}
