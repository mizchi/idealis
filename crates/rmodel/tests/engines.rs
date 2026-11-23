//! Every check of the corpus, through both lowerings: Alloy text solved by
//! ralloy's SAT (bounded by the model's steps) and TLA+ checked by rtlc
//! (complete). Both must give the verdict written before any engine ran
//! (plan 0091).
use serde_json::Value;
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Expected verdicts by model file stem and check, from the claims of the
/// corpus specification (plan 0092; they replaced expected.json).
fn claims() -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> {
    let spec: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/rmodel/spec.json")).unwrap(),
    )
    .unwrap();
    let mut claims =
        std::collections::BTreeMap::<String, std::collections::BTreeMap<String, String>>::new();
    for rule in spec["spec"]["rules"].as_array().unwrap() {
        for claim in rule["claims"].as_array().unwrap() {
            let file = Path::new(claim["model"].as_str().unwrap())
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            claims.entry(file).or_default().insert(
                claim["check"].as_str().unwrap().to_owned(),
                claim["expect"].as_str().unwrap().to_owned(),
            );
        }
    }
    claims
}

#[derive(Debug, PartialEq)]
struct Explicit {
    verdict: &'static str,
    depth: u32,
}

/// The verdict of a check from ralloy's SAT on the Alloy lowering.
fn sat(model: &rmodel::Model, alloy: &ralloy::Model, check: &rmodel::Check) -> &'static str {
    let label = rmodel::alloy_label(&check.name);
    let index = alloy
        .commands()
        .iter()
        .position(|command| command.label == label)
        .unwrap_or_else(|| panic!("{}: no command {label}", model.name));
    let analysis = alloy.analyze(index, 50_000_000).unwrap();
    if let Some(instance) = &analysis.instance {
        let checked = alloy.check_instance(index, instance, 50_000_000).unwrap();
        assert_ne!(
            checked.status,
            ralloy::CheckStatus::Invalid,
            "{}",
            check.name
        );
        assert_ne!(
            checked.status,
            ralloy::CheckStatus::Unknown,
            "{}",
            check.name
        );
    }
    match analysis.status {
        ralloy::Status::Counterexample => "violated",
        ralloy::Status::NoCounterexample => "holds",
        ralloy::Status::Instance => "reachable",
        ralloy::Status::NoInstance => "unreachable",
        ralloy::Status::Unknown => panic!("{}: SAT budget", check.name),
    }
}

/// The verdict and violation depth from rtlc on the TLA+ lowering.
fn explicit(model: &rmodel::Model, check: &rmodel::Check) -> Explicit {
    let (source, config) = rmodel::to_tla(model, &check.name).unwrap();
    let program = idealis_core::compile_tla_with_options(
        &source,
        &config,
        idealis_core::CompileOptions {
            irVersion: idealis_core::IrVersion::V2,
        },
    )
    .unwrap_or_else(|error| panic!("{}: {error:?}\n{source}", check.name));
    let result = program
        .check(idealis_core::SafetyBounds {
            max_depth: 1_000,
            max_states: 2_000_000,
            max_work: 500_000_000,
            max_temporal_nodes: None,
        })
        .unwrap_or_else(|error| panic!("{}: {error:?}", check.name));
    let found = match result.status.as_str() {
        "complete" => false,
        _ if result.violation.is_some() => true,
        other => panic!("{}: {other} {:?}", check.name, result.reason),
    };
    let verdict = match (check.kind, found) {
        (rmodel::CheckKind::Invariant, false) => "holds",
        (rmodel::CheckKind::Invariant, true) => "violated",
        (rmodel::CheckKind::Reachable, false) => "unreachable",
        (rmodel::CheckKind::Reachable, true) => "reachable",
    };
    Explicit {
        verdict,
        depth: result
            .violation
            .as_ref()
            .map_or(result.depth, |_| result.depth),
    }
}

#[test]
fn both_engines_give_the_expected_verdicts() {
    let mut compared = 0;
    for (file, checks) in claims() {
        let source =
            std::fs::read_to_string(root().join(format!("fixtures/rmodel/{file}.model"))).unwrap();
        let model = rmodel::parse(&source).unwrap_or_else(|error| panic!("{file}: {error}"));
        let alloy_source = rmodel::to_alloy(&model);
        let alloy = ralloy::Model::parse(&alloy_source)
            .unwrap_or_else(|error| panic!("{file}: {error}\n{alloy_source}"));
        assert_eq!(model.checks.len(), checks.len(), "{file}");
        for check in &model.checks {
            let want = checks[&check.name].as_str();
            let explicit = explicit(&model, check);
            assert_eq!(explicit.verdict, want, "{file} {} explicit", check.name);
            // The SAT bound covers the explicit search's depth.
            assert!(
                (explicit.depth as usize) < model.scope.steps,
                "{file} {}: depth {} needs more than {} steps",
                check.name,
                explicit.depth,
                model.scope.steps
            );
            assert_eq!(
                sat(&model, &alloy, check),
                want,
                "{file} {} SAT",
                check.name
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 20);
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Alloy 6.2.0 and TLC verdicts in oracle.json were recorded for exactly
/// the text rmodel lowers to now; rerun evals/experiments/rmodel/oracle.sh
/// after changing a lowering.
#[test]
fn external_tools_agree_on_the_current_lowerings() {
    let expected = claims();
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/rmodel/oracle.json")).unwrap(),
    )
    .unwrap();
    let rows = oracle["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 20);
    for row in rows {
        let file = row["model"].as_str().unwrap();
        let check = row["check"].as_str().unwrap();
        let model = rmodel::parse(
            &std::fs::read_to_string(root().join(format!("fixtures/rmodel/{file}.model"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            sha256_hex(rmodel::to_alloy(&model).as_bytes()),
            row["alsSha256"],
            "{file}"
        );
        let (source, _) = rmodel::to_tla(&model, check).unwrap();
        let source = source.replacen(
            &format!("MODULE {} ", model.name),
            &format!("MODULE {}_{check} ", model.name),
            1,
        );
        assert_eq!(
            sha256_hex(source.as_bytes()),
            row["tlaSha256"],
            "{file} {check}"
        );
        let found = match expected[file][check].as_str() {
            "violated" | "reachable" => "found",
            _ => "none",
        };
        assert_eq!(row["alloy"], found, "{file} {check} Alloy");
        assert_eq!(row["tlc"], found, "{file} {check} TLC");
    }
}

/// Relation-relation joins and products, which the corpus does not use,
/// mean the same in both lowerings.
#[test]
fn composed_relations_agree() {
    let model = rmodel::parse(
        "model Compose
sig A
sig B
sig C
rel f: A -> set B
rel g: B -> set C
var h: A -> set C
init { no h }
action compose() { h' = f.g }
action pairs(a: A, c: C) { h' = h + a -> c }
reachable Composed { some h and h = f.g }
invariant WithinComposition { h in f.g }
invariant Product { f.g in A -> C }
scope A = 2, B = 2, C = 2, steps = 4",
    )
    .unwrap();
    let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model)).unwrap();
    for (check, want) in [
        ("Composed", "reachable"),
        ("WithinComposition", "violated"),
        ("Product", "holds"),
    ] {
        let check = model
            .checks
            .iter()
            .find(|candidate| candidate.name == check)
            .unwrap();
        assert_eq!(
            explicit(&model, check).verdict,
            want,
            "{} explicit",
            check.name
        );
        assert_eq!(sat(&model, &alloy, check), want, "{} SAT", check.name);
    }
}

#[test]
fn malformed_models_are_rejected() {
    let base = "model M\nsig A\nvar v: set A\nrel r: set A\n";
    let scope = "\nscope A = 2, steps = 3";
    for (body, needle) in [
        ("invariant I { some w }", "unknown name w"),
        ("fact { some v }", "fact refers to the var v"),
        ("invariant I { some v' }", "expected"),
        ("action a() { v' = v' }", "expected"),
        ("action a() { v' = v  v' = none }", "assigned twice"),
        ("action a() { r' = r }", "is not a var"),
        ("action a(x: A) { v' = x -> x }", "another arity"),
        ("invariant I { #(A -> A) < 2 }", "# counts only sets"),
        ("invariant I { all v: A | some v }", "shadows"),
        ("invariant I { #v < 9 }", "literal 9"),
        ("sig A", "duplicate name A"),
    ] {
        let error = rmodel::parse(&format!("{base}{body}{scope}")).unwrap_err();
        assert!(error.message.contains(needle), "{body}: {error}");
    }
    assert!(rmodel::parse("model M\nsig A\nscope A = 8, steps = 3").is_err());
    assert!(rmodel::parse("model M\nsig A\nscope A = 2").is_err());
    assert!(rmodel::parse("model M\nsig A\nsig B\nscope A = 2, steps = 3").is_err());
}
