use ralloy::{Model, Status};

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
        "sig A {} sig B extends A {} run {}",
        "open util/ordering[A] sig A {} run {}",
        "sig A {} pred P[x: A] {} run {}",
        "sig A {} run { #A = 1 }",
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
