use ralloy::{CheckStatus, Instance, Model, Status};
use std::collections::BTreeMap;

fn analyze(source: &str) -> ralloy::Analysis {
    Model::parse(source).unwrap().analyze(0, 2_000_000).unwrap()
}

#[test]
fn directed_cycle_yields_a_concrete_counterexample() {
    let result = analyze(
        "sig Node { edge: set Node } fact { no iden & edge } assert Acyclic { no iden & ^edge } check Acyclic for 2 but exactly 2 Node, 0 Int",
    );
    assert_eq!(result.status, Status::Counterexample);
    let instance = result.instance.unwrap();
    assert_eq!(instance.signatures["Node"].len(), 2);
    assert_eq!(instance.fields["edge"].len(), 2);
}

#[test]
fn exhaustive_checks_distinguish_inconsistent_facts() {
    assert_eq!(analyze("sig N { r: set N } fact { no iden & ^r } assert Safe { all n: N | n not in n.^r } check Safe for 2 but 0 Int").status, Status::NoCounterexample);
    assert_eq!(
        analyze("sig A {} fact { some A no A } assert Safe { no A } check Safe for 2 but 0 Int")
            .status,
        Status::NoInstance
    );
}

#[test]
fn default_field_multiplicity_and_empty_quantifiers_match_alloy() {
    assert_eq!(
        analyze("one sig A { r: A } run { no r } for 0 but 0 Int").status,
        Status::NoInstance
    );
    assert_eq!(
        analyze("sig A {} run { all x: A | x in A } for 0 but 0 Int").status,
        Status::Instance
    );
    assert_eq!(
        analyze("sig A {} run { one x,y: A | x = y } for 1 but exactly 1 A, 0 Int").status,
        Status::Instance
    );
    assert_eq!(
        analyze("sig A {} run { one disj x,y: A | x in A } for 2 but exactly 2 A, 0 Int").status,
        Status::NoInstance
    );
}

#[test]
fn scopes_and_builtin_integers_are_explicit() {
    assert_eq!(
        analyze("sig A {} run { no Int } for 0").status,
        Status::NoInstance
    );
    assert_eq!(
        analyze("sig A {} run { no univ } for 0 but 0 Int").status,
        Status::Instance
    );
    assert_eq!(
        analyze("one sig A {} run { some A } for 0 but 0 Int").status,
        Status::Instance
    );
    assert_eq!(
        analyze("lone sig A {} run { some A } for 0 but 0 Int").status,
        Status::Instance
    );
}

#[test]
fn relation_algebra_and_predicate_calls_work() {
    assert_eq!(analyze("sig N { r: set N } pred P { some r r = ~r r.r in r + iden } run P for 2 but exactly 2 N, 0 Int").status, Status::Instance);
    assert_eq!(
        analyze("sig N { r: set N } assert Id { all n: N | n in n.*r } check Id for 2 but 0 Int")
            .status,
        Status::NoCounterexample
    );
    assert_eq!(
        analyze("sig N { r: set N } assert Empty { no r - (N -> N) } check Empty for 2 but 0 Int")
            .status,
        Status::NoCounterexample
    );
}

#[test]
fn empty_relations_preserve_arity() {
    assert_eq!(
        analyze("sig N { r: set N } run { r = (none -> none) no ^(none -> none) no (none + none) } for 1 but 0 Int")
            .status,
        Status::Instance
    );
}

#[test]
fn budget_and_unsupported_input_never_become_success() {
    let model = Model::parse("sig N { r: set N } run { some r } for 2 but 0 Int").unwrap();
    let result = model.analyze(0, 0).unwrap();
    assert_eq!(result.status, Status::Unknown);
    assert!(result.instance.is_none());
    for source in [
        "var sig A {} run {}",
        "open util/ordering[A] sig A {} run {}",
        "sig A {} pred P { P } run {}",
        "sig A {} fact { A } run {}",
        "sig A {} run { unknown in A }",
        "sig A {} run { all x: none | no ^x }",
        "sig A { r: set A } run { r = none }",
        "sig A { r: set A } run { r = none + none }",
        "sig A {} run { no ^none }",
        "one sig A {} run {} for 2 but 2 A",
        "sig A {} run {} for 1 but 1 A, 2 A",
    ] {
        assert!(Model::parse(source).is_err(), "{source}");
    }
}

/// Temporal operators outside plan 0090 are rejected, not read as names.
#[test]
fn unsupported_temporal_operators_are_errors() {
    for body in [
        "once { some A.v }",
        "historically { some A.v }",
        "before { some A.v }",
        "{ some A.v } until { no A.v }",
        "{ some A.v } releases { no A.v }",
    ] {
        let source = format!("sig B {{}} one sig A {{ var v: set B }} run {{ {body} }} for 2");
        assert!(Model::parse(&source).is_err(), "{body}");
    }
    assert!(Model::parse("one sig A { var v: set A } run {} for 2 but 0 steps").is_err());
    assert!(Model::parse("one sig A { var v: set A } run {} for 2 but 33 steps").is_err());
}

/// A temporal instance is a lasso of var-field states; static instances
/// carry no trace (plan 0090).
#[test]
fn temporal_instances_are_lassos() {
    let model = Model::parse(
        "sig B {} one sig A { var v: set B } fact { no A.v } run { eventually { some A.v } } for 2 but 3 steps",
    )
    .unwrap();
    assert_eq!(model.commands()[0].scope.steps, Some(3));
    let analysis = model.analyze(0, 1_000_000).unwrap();
    let instance = analysis.instance.unwrap();
    assert!(instance.fields.is_empty());
    assert_eq!(instance.trace.len(), 3);
    assert!(instance.back.is_some_and(|back| back < 3));
    let check = model.check_instance(0, &instance, 1_000_000).unwrap();
    assert_eq!(check.status, CheckStatus::Matched);
    let mut open = Instance {
        signatures: instance.signatures.clone(),
        trace: instance.trace.clone(),
        ..Instance::default()
    };
    assert!(model.check_instance(0, &open, 1_000_000).is_err());
    open.back = Some(3);
    assert!(model.check_instance(0, &open, 1_000_000).is_err());
    // The initial state violates the fact.
    open.back = Some(0);
    open.trace = vec![BTreeMap::from([(
        "v".into(),
        vec![vec!["A$0".into(), "B$0".into()]],
    )])];
    open.signatures = BTreeMap::from([
        ("A".into(), vec!["A$0".into()]),
        ("B".into(), vec!["B$0".into()]),
    ]);
    assert_eq!(
        model.check_instance(0, &open, 1_000_000).unwrap().reason,
        Some(ralloy::CheckReason::Facts)
    );
    let static_model = Model::parse("sig B {} run { some B } for 2").unwrap();
    assert_eq!(static_model.commands()[0].scope.steps, None);
    let static_instance = static_model
        .analyze(0, 1_000_000)
        .unwrap()
        .instance
        .unwrap();
    assert!(static_instance.trace.is_empty() && static_instance.back.is_none());
}

/// The parser admits 16 signatures; with Int and String the type masks need
/// 18 bits (found by rmodel's enums in plan 0094, which overflowed u16).
#[test]
fn sixteen_signatures_type_check() {
    let sigs: Vec<String> = (0..16).map(|index| format!("sig S{index} {{}}")).collect();
    let source = format!(
        "{}\nrun {{ some S15 and no S0 & S15 }} for 1",
        sigs.join("\n")
    );
    let model = Model::parse(&source).unwrap();
    assert_eq!(
        model.analyze(0, 1_000_000).unwrap().status,
        Status::Instance
    );
    // A join of two unrelated signatures is still recognized as one.
    let joined = format!("{}\nrun {{ some S14 & S15 }} for 1", sigs.join("\n"));
    assert_eq!(
        Model::parse(&joined)
            .unwrap()
            .analyze(0, 1_000_000)
            .unwrap()
            .status,
        Status::NoInstance
    );
}
