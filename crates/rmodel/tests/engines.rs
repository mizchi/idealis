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

/// Enums name their atoms in both lowerings (plan 0093).
#[test]
fn enums_agree() {
    let model = rmodel::parse(
        "model Light
enum Color = Red | Green | Amber
sig Lamp
var shows: Lamp -> one Color
init { Lamp.shows in Red }
action next(l: Lamp) { Red in l.shows  shows' = shows - l -> Red + l -> Green }
action caution(l: Lamp) { Green in l.shows  shows' = shows - l -> Green + l -> Amber }
action stop(l: Lamp) { Amber in l.shows  shows' = shows - l -> Amber + l -> Red }
invariant OneColor { all l: Lamp | one l.shows }
reachable AllGreen { Lamp.shows = Green }
reachable Mixed { some shows.Red and some shows.Amber }
invariant NeverAmber { no shows.Amber }
reachable AmberAfterRed { all c: Color | c in Lamp.shows }
scope Lamp = 2, steps = 6",
    )
    .unwrap();
    let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model))
        .unwrap_or_else(|error| panic!("{error}\n{}", rmodel::to_alloy(&model)));
    for (check, want) in [
        ("OneColor", "holds"),
        ("AllGreen", "reachable"),
        ("Mixed", "reachable"),
        ("NeverAmber", "violated"),
        ("AmberAfterRed", "unreachable"),
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
fn malformed_enums_are_rejected() {
    for (source, needle) in [
        (
            "model M\nenum E = A | A\nscope steps = 1",
            "duplicate name A",
        ),
        (
            "model M\nenum E = A\nsig S\nscope S = 1, E = 1, steps = 1",
            "scope for unknown sig E",
        ),
        (
            "model M\nenum E = A\nrel r: set E\ninvariant I { r in B }\nscope steps = 1",
            "unknown name B",
        ),
        (
            "model M\nenum E = A\nsig A\nscope A = 1, steps = 1",
            "duplicate name A",
        ),
    ] {
        let error = rmodel::parse(source).unwrap_err();
        assert!(error.message.contains(needle), "{source}: {error}");
    }
}

/// Review of #102 and plan 0099: a count reaches its sort's full size
/// within the Alloy integer width; the Alloy
/// bit width covers every atom in scope.
#[test]
fn counts_across_signatures_do_not_wrap() {
    let model = rmodel::parse(
        "model Count
sig A
sig B
var marked: set A
init { no marked }
action mark(a: A) { marked' = marked + a }
reachable Big { #(marked + A) > 6 }
reachable AllMarked { #marked = 7 }
scope A = 7, B = 4, steps = 9",
    )
    .unwrap();
    let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model)).unwrap();
    for (check, want) in [("Big", "reachable"), ("AllMarked", "reachable")] {
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
    // Beyond 15 atoms no supported width holds every count.
    let error = rmodel::parse(
        "model Wide\nsig A\nsig B\nsig C\ninvariant I { #A > 0 }\nscope A = 6, B = 6, C = 6, steps = 1",
    )
    .unwrap_err();
    assert!(error.message.contains("15"), "{error}");
}

/// Review of #102: `none` takes the arity of the relation it stands for.
#[test]
fn none_for_a_binary_relation() {
    let model = rmodel::parse(
        "model Clear
sig A
var edges: A -> set A
init { some edges }
action clear() { edges' = none }
action link(a: A, b: A) { edges' = edges + a -> b }
reachable Cleared { edges = none }
invariant Within { edges in A -> A }
scope A = 2, steps = 4",
    )
    .unwrap();
    let source = rmodel::to_alloy(&model);
    let alloy = ralloy::Model::parse(&source).unwrap_or_else(|error| panic!("{error}\n{source}"));
    for (check, want) in [("Cleared", "reachable"), ("Within", "holds")] {
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

/// Review of #102: deep nesting is an error, never a stack overflow.
#[test]
fn deep_inputs_are_refused() {
    let base = "model M\nsig A\nvar v: set A\n";
    let scope = "\nscope A = 1, steps = 1";
    let nested = format!(
        "invariant I {{ {}some v{} }}",
        "(".repeat(5_000),
        ")".repeat(5_000)
    );
    assert!(rmodel::parse(&format!("{base}{nested}{scope}")).is_err());
    let chain = format!("invariant I {{ {} }}", vec!["some v"; 5_000].join(" or "));
    assert!(rmodel::parse(&format!("{base}{chain}{scope}")).is_err());
    let joins = format!("invariant I {{ some {} }}", vec!["v"; 5_000].join(" + "));
    assert!(rmodel::parse(&format!("{base}{joins}{scope}")).is_err());
}

/// Review of #103 and plan 0099: an enum member named like a generated sig
/// atom is another sort, so mixing them is a type error rather than an
/// equality between atoms.
#[test]
fn enum_atoms_are_another_sort() {
    let error = rmodel::parse(
        "model Names\nenum E = S0\nsig S\nvar on: set S\ninit { no on }\ninvariant Disjoint { no S & E }\nscope S = 1, steps = 3",
    )
    .unwrap_err();
    assert!(error.message.contains("different sorts"), "{error}");
}

/// Plan 0099: expressions whose sorts do not match are type errors.
#[test]
fn sort_mismatches_are_rejected() {
    let base = "model M\nsig A\nsig B\nenum E = Ea | Eb\nvar a: set A\nvar f: A -> set B\n";
    let scope = "\nscope A = 2, B = 2, steps = 3";
    for (body, needle) in [
        ("invariant I { a in B }", "comparison of different sorts"),
        (
            "invariant I { some a + B }",
            "set operation of different sorts",
        ),
        ("invariant I { some B.f }", "join of different sorts"),
        ("invariant I { some f.A }", "join of different sorts"),
        ("action x() { a' = Ea }", "another sort"),
        ("action x(b: B) { a' = a + b }", "different sorts"),
        (
            "invariant I { all e: E | e in a }",
            "comparison of different sorts",
        ),
    ] {
        let error = rmodel::parse(&format!("{base}{body}{scope}")).unwrap_err();
        assert!(error.message.contains(needle), "{body}: {error}");
    }
    // Well-sorted forms are accepted.
    for body in [
        "invariant I { a.f in B }",
        "action x(b: B, y: A) { f' = f + y -> b }",
    ] {
        assert!(
            rmodel::parse(&format!("{base}{body}{scope}")).is_ok(),
            "{body}"
        );
    }
}

/// Issue #106: a model that never counts keeps 4-bit integers, so its
/// universe stays within ralloy's 64 atoms.
#[test]
fn count_free_models_keep_the_small_width() {
    let model = rmodel::parse(
        "model Wide
sig A
sig B
sig C
sig D
sig E
var on: set A
init { no on }
action add(a: A) { on' = on + a }
reachable Some { some on }
scope A = 7, B = 7, C = 7, D = 7, E = 7, steps = 3",
    )
    .unwrap();
    let source = rmodel::to_alloy(&model);
    let alloy = ralloy::Model::parse(&source).unwrap_or_else(|error| panic!("{error}"));
    let check = &model.checks[0];
    assert_eq!(sat(&model, &alloy, check), "reachable");
}

/// Issue #106: `none` inside a compound expression takes the binary arity.
#[test]
fn compound_none_for_a_binary_relation() {
    let model = rmodel::parse(
        "model Clear
sig A
var edges: A -> set A
init { some edges }
action clear() { edges' = none + none }
reachable Cleared { edges = none - none }
scope A = 2, steps = 4",
    )
    .unwrap();
    let source = rmodel::to_alloy(&model);
    let alloy = ralloy::Model::parse(&source).unwrap_or_else(|error| panic!("{error}\n{source}"));
    assert_eq!(sat(&model, &alloy, &model.checks[0]), "reachable");
    assert_eq!(explicit(&model, &model.checks[0]).verdict, "reachable");
}

/// Issue #106: the total size of one formula is bounded, so nesting under
/// the depth limit cannot build a spine the checker overflows on.
#[test]
fn formula_size_is_bounded() {
    let level = |inner: String| format!("({} + {inner})", vec!["v"; 200].join(" + "));
    let mut expression = "v".to_owned();
    for _ in 0..40 {
        expression = level(expression);
    }
    let source = format!(
        "model M\nsig A\nvar v: set A\ninvariant I {{ some {expression} }}\nscope A = 1, steps = 1"
    );
    let error = rmodel::parse(&source).unwrap_err();
    assert!(error.message.contains("nodes"), "{error}");
}

/// Review of #110: a chain of `not` counts against the nesting bound.
#[test]
fn long_negation_chains_are_refused() {
    let chain = format!("invariant I {{ {}some v }}", "not ".repeat(100_000));
    assert!(
        rmodel::parse(&format!(
            "model M\nsig A\nvar v: set A\n{chain}\nscope A = 1, steps = 1"
        ))
        .is_err()
    );
}

/// Plan 0097: a parenthesized set expression may start a comparison.
#[test]
fn parenthesized_sets_start_comparisons() {
    let model = rmodel::parse(
        "model P\nsig A\nsig B\nvar v: set A\ninit { (A + A) in (A + A) and (no v) }\naction add(a: A) { v' = v + a }\ninvariant I { (v + v) in A }\nreachable R { ((some v)) }\nscope A = 1, B = 1, steps = 3",
    )
    .unwrap();
    assert_eq!(model.checks.len(), 2);
}

/// Plan 0099: a state that breaks a multiplicity has no successors. Here
/// only such a state could take `flag`, so `NeverFlagged` holds and the
/// multiplicity check is violated. Added after the generated models did not
/// detect a TLA+ lowering that let broken states step.
#[test]
fn broken_states_do_not_step() {
    let model = rmodel::parse(
        "model Broken
sig A
var x: lone A
var y: set A
init { no x and no y }
action add(a: A) { x' = x + a }
action flag() { #x = 2  y' = A  x' = none }
invariant NeverFlagged { no y }
scope A = 2, steps = 6",
    )
    .unwrap();
    let report = rmodel::reference::analyze(&model, 100_000);
    assert_eq!(
        report.declarations,
        rmodel::reference::Outcome::Violated { depth: 2 }
    );
    let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model)).unwrap();
    let check = &model.checks[0];
    assert_eq!(explicit(&model, check).verdict, "holds");
    assert_eq!(sat(&model, &alloy, check), "holds");
    let (source, config) = rmodel::to_tla(&model, rmodel::DECLARATIONS).unwrap();
    let result = idealis_core::compile_tla_with_options(
        &source,
        &config,
        idealis_core::CompileOptions {
            irVersion: idealis_core::IrVersion::V2,
        },
    )
    .unwrap()
    .check(idealis_core::SafetyBounds {
        max_depth: 100,
        max_states: 100_000,
        max_work: 10_000_000,
        max_temporal_nodes: None,
    })
    .unwrap();
    assert!(result.violation.is_some(), "{}", result.status);
}

/// Plan 0100: sum sorts make the idioms sort checking took away writable
/// again, and mean the same on all engines.
#[test]
fn sum_sorts_restore_polymorphic_idioms() {
    let model = rmodel::parse(
        "model Zoo
sig Cat
sig Dog
sig Fish
type Animal = Cat + Dog
sig Account
sig Customer
sig Staff
type Party = Customer + Staff
rel owner: Account -> one Party
var fed: set Animal
init { no fed }
action feed(a: Animal) { fed' = fed + a }
action feedCat(c: Cat) { fed' = fed + c }
invariant Plain { all a: Animal | a in Cat + Dog }
invariant Owned { Account.owner in Customer + Staff }
reachable AllFed { fed = Cat + Dog }
reachable OnlyCats { some fed and fed in Cat }
invariant NeverDog { no fed & Dog }
scope Cat = 2, Dog = 1, Fish = 1, Account = 2, Customer = 1, Staff = 1, steps = 5",
    )
    .unwrap();
    let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model))
        .unwrap_or_else(|error| panic!("{error}\n{}", rmodel::to_alloy(&model)));
    let reference = rmodel::reference::evaluate(&model, 1_000_000);
    for (check, want) in [
        ("Plain", "holds"),
        ("Owned", "holds"),
        ("AllFed", "reachable"),
        ("OnlyCats", "reachable"),
        ("NeverDog", "violated"),
    ] {
        let check = model
            .checks
            .iter()
            .find(|candidate| candidate.name == check)
            .unwrap();
        let verdict = reference.iter().find(|v| v.check == check.name).unwrap();
        let word = match verdict.outcome {
            rmodel::reference::Outcome::Holds => "holds",
            rmodel::reference::Outcome::Violated { .. } => "violated",
            rmodel::reference::Outcome::Reachable { .. } => "reachable",
            rmodel::reference::Outcome::Unreachable => "unreachable",
            other => panic!("{other:?}"),
        };
        assert_eq!(word, want, "{} reference", check.name);
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
fn malformed_sum_sorts_are_rejected() {
    let base = "model M\nsig A\nsig B\nsig C\n";
    let scope = "\nscope A = 1, B = 1, C = 1, steps = 1";
    for (body, needle) in [
        (
            "type S = A + B\nvar v: set A\ninvariant I { v + C in A }",
            "different sorts",
        ),
        (
            "type S = A + B\ntype T = B + C\nvar v: set A\ninvariant I { some v }",
            "two sum sorts",
        ),
        (
            "type S = A + B\ntype T = S + C\nvar v: set A\ninvariant I { some v }",
            "not a signature or enum",
        ),
        (
            "type S = A + B\nvar v: set A\naction x(s: S) { v' = v + s }",
            "another sort",
        ),
    ] {
        let error = rmodel::parse(&format!("{base}{body}{scope}")).unwrap_err();
        assert!(error.message.contains(needle), "{body}: {error}");
    }
}
