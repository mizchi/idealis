//! rcheck <model> [--checker auto|reference|explicit|sat|rprob|all] [--question NAME] [--max-states N]
//! rcheck capabilities <model>
//! rcheck generate <model> rust|typescript|go|moonbit|ir|schema|states|glossary|pbt-cases|pbt-typescript|pbt-go|branch-map
//! rcheck generate <project> rust|typescript|go|moonbit|schema --public
//! rcheck component <component.json>
//! rcheck contracts <project.rmodel.json> [--max-states N] [--max-arguments N]
//! rcheck dead-code <model> [--instances ACTION] [--max-states N] [--max-arguments N]
//! rcheck coverage <model> [--instances] [--branch-map PATH] [--max-states N] [--max-arguments N]
//! rcheck deadlock <model> [--unless REACHABLE] [--ignore-stutter] [--max-states N] [--max-arguments N]
//! rcheck branches <model> <action> [--branch-map PATH] [--max-states N] [--max-arguments N]
//! rcheck witness <model> <question> [--checker rprob] [--max-states N] [--max-arguments N]
//! rcheck witness-step <model> <action> <predicate> [--max-states N] [--max-arguments N]
//! rcheck observation-pair <model> <request.json|-> [--max-states N]
//! rcheck lean-oracle <model> <file.lean> [--max-states N]
//! rcheck record-itf <source-label> <recording.json|->
//! rcheck replay-itf <model> <trace.itf.json|->
//! rcheck checker-request <model> <question> [--max-states N]
//! rcheck checker-verify <request.json> <reply.json|->
//! rcheck traceability <spec.json> (--rule ID | --check MODEL CHECK) [--root DIR] [--document PATH] [--max-states N]
//! Answers every question of an rmodel model, or one, and prints JSON. Exits
//! 1 when two definite answers disagree, 2 on a usage or model error.
use rcheck::{Selection, Verdict};
mod capabilities;
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
    if let [command, spec, flags @ ..] = arguments.as_slice()
        && command == "traceability"
    {
        traceability::run(spec, flags);
        return;
    }
    if let Some((command, args)) = arguments.split_first()
        && command == "component"
    {
        let [path] = args else { usage() };
        let report = rcheck::project::check_component(path).unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
        return;
    }
    if let [command, path, flags @ ..] = arguments.as_slice()
        && command == "contracts"
    {
        let result = rcheck::project::check_contracts(path, search_limits(flags, None).budget())
            .unwrap_or_else(|error| fail(&error));
        println!("{}", serde_json::to_string_pretty(&result).expect("JSON"));
        match result["status"].as_str() {
            Some("matched") => return,
            Some("mismatch") => std::process::exit(1),
            _ => std::process::exit(3),
        }
    }
    if let Some((command, args)) = arguments.split_first() {
        use rcheck::conform::external;
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
                let budget = rcheck::Budget {
                    states: limits.budget().states,
                    ..rcheck::Budget::default()
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
        let report = rcheck::conform::coverage_report_with_instances(
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
        let report = rcheck::conform::update_branch_report_with_map(
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
            Some(predicate) => {
                rcheck::conform::transition_witness_with_budget(&model, target, predicate, budget)
            }
            None if limits.selected.is_some() && limits.arguments.is_some() => {
                rcheck::conform::rprob_witness_with_budget(&model, target, budget)
            }
            None if limits.selected.is_some() => {
                rcheck::conform::rprob_witness(&model, target, budget.states)
            }
            None if limits.arguments.is_none() => {
                rcheck::conform::witness(&model, target, budget.states)
            }
            None => rcheck::conform::witness_with_budget(&model, target, budget),
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
        use rcheck::conform::itf;
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
            [path, request] => (path, request, rcheck::Budget::default().states),
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
        let report = rcheck::conform::observation_pair(&model, &query, budget)
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
            let map =
                rcheck::conform::generate_branch_map(path).unwrap_or_else(|error| fail(&error));
            println!("{}", serde_json::to_string_pretty(&map).expect("JSON"));
            return;
        }
        let (model, source) = if public {
            let (mut model, exports) = rcheck::project::load_public_binding_source(path)
                .unwrap_or_else(|error| fail(&error));
            model
                .actions
                .retain(|action| exports.contains(&action.name));
            (model, None)
        } else {
            let (model, source) =
                rcheck::project::load_model_source(path).unwrap_or_else(|error| fail(&error));
            (model, Some(source))
        };
        let text = match target.as_str() {
            "rust" => match source.as_deref() {
                Some(source) => rmodel::bindings::rust(&model, source),
                None => rmodel::bindings::rust_api(&model),
            },
            "typescript" => rmodel::bindings::typescript(&model),
            "go" => rmodel::bindings::go(&model, &model.name.to_lowercase()),
            "moonbit" => rmodel::bindings::moonbit(&model),
            "schema" => {
                let schema: serde_json::Value =
                    serde_json::from_str(&rmodel::bindings::json_schema(&model)).expect("JSON");
                format!("{}\n", serde_json::to_string_pretty(&schema).expect("JSON"))
            }
            _ if public => fail("--public supports rust, typescript, go, moonbit, and schema"),
            // The kernel IR as JSON (plan 0108).
            "ir" => {
                let ir = rcheck::ir::to_ir(&model).unwrap_or_else(|error| fail(&error));
                format!("{}\n", serde_json::to_string_pretty(&ir).expect("JSON"))
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
    let mut expectation = None;
    let mut budget = rcheck::Budget::default();
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
    let names: Vec<String> = match only {
        Some(name) => vec![name],
        None => rcheck::questions(&model)
            .into_iter()
            .map(|question| question.name)
            .collect(),
    };
    let mut reports = Vec::new();
    for name in names {
        match rcheck::check(&model, &name, &selection, budget) {
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

fn enforce_expectation(report: &rcheck::Report, name: &str, expected: Verdict) {
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

fn load_model(path: &str) -> (rmodel::Model, Vec<rcheck::ir::Omitted>) {
    if path.ends_with(".json") && !rcheck::project::is_manifest(path) {
        let source = rcheck::project::load_source(path).unwrap_or_else(|error| fail(&error));
        rcheck::ir::parse(&source).unwrap_or_else(|error| fail(&error))
    } else {
        let (model, _) =
            rcheck::project::load_model_source(path).unwrap_or_else(|error| fail(&error));
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
    let validator = rcheck::conform::Validator::new(&model).unwrap_or_else(|error| fail(&error));
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
    fn budget(&self) -> rmodel::reference::SearchBudget {
        let states = self
            .states
            .unwrap_or_else(|| rcheck::Budget::default().states);
        let mut budget = rmodel::reference::SearchBudget::for_states(states);
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
    use rmodel::reference::DeadlockMode;
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
    let report =
        rcheck::conform::deadlock_witness_with_mode(&model, limits.selected, limits.budget(), mode)
            .unwrap_or_else(|error| fail(&error));
    println!("{}", serde_json::to_string_pretty(&report).expect("JSON"));
    match report["verdict"].as_str() {
        Some("unreachable") => (),
        Some("reachable") => std::process::exit(1),
        _ => std::process::exit(3),
    }
}

fn dead_code(args: &[String]) {
    use rmodel::reference::Outcome;
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
            let (report, instances) = rmodel::reference::analyze_instances_with_budget(
                &model,
                action,
                budget,
                MAX_INSTANCES,
            )
            .unwrap_or_else(|error| fail(&error));
            (report, Some(instances))
        }
        None => (
            rmodel::reference::analyze_actions_with_budget(&model, budget),
            None,
        ),
    };
    let coverage = match report.scope {
        Outcome::Holds => "complete",
        Outcome::Empty => "empty",
        _ => "unknown",
    };
    let diagnostics = rcheck::reference_diagnostics(report.scope, report.stop.as_ref());
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
                "args": rcheck::conform::arguments(&instance.call.args).collect::<serde_json::Map<_, _>>(),
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
  rcheck <model> [--checker auto|reference|explicit|sat|rprob|all] [--question NAME] [--max-states N]
         [--expect holds|violated|reachable|unreachable]
  rcheck capabilities <model>
  rcheck generate <model> <target>
  rcheck generate <project> rust|typescript|go|moonbit|schema --public
  rcheck component <component.json>
  rcheck contracts <project.rmodel.json> [--max-states N] [--max-arguments N]
  rcheck dead-code <model> [--instances ACTION] [--max-states N] [--max-arguments N]
  rcheck coverage <model> [--instances] [--branch-map PATH] [--max-states N] [--max-arguments N]
  rcheck deadlock <model> [--unless REACHABLE] [--ignore-stutter] [--max-states N] [--max-arguments N]
  rcheck branches <model> <action> [--branch-map PATH] [--max-states N] [--max-arguments N]
  rcheck witness <model> <question> [--checker rprob] [--max-states N] [--max-arguments N]
  rcheck witness-step <model> <action> <predicate> [--max-states N] [--max-arguments N]
  rcheck observation-pair <model> <request.json|-> [--max-states N]
  rcheck lean-oracle <model> <file.lean> [--max-states N]
  rcheck validate <model> <request.json|->
  rcheck record-itf <source-label> <recording.json|->
  rcheck replay-itf <model> <trace.itf.json|->
  rcheck checker-request <model> <question> [--max-states N]
  rcheck checker-verify <request.json> <reply.json|->
  rcheck traceability <spec.json> (--rule ID | --check MODEL CHECK)
         [--root DIR] [--document PATH] [--max-states N]

Model inputs: .model source, .rmodel.json project, or JSON IR (except for generate).
Generation targets: rust, typescript, go, moonbit, ir, schema, states, glossary,
                    pbt-cases, pbt-typescript, pbt-go, branch-map";

fn usage() -> ! {
    fail(USAGE)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}
