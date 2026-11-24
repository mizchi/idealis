use ralloy::{CheckReason, CheckStatus, ErrorKind, Instance, Model};
use std::collections::BTreeMap;

const GRAPH: &str = "sig N { edge: set N } fact { no iden & edge } assert Safe { no iden & ^edge } check Safe for 3 but 0 Int";

fn graph(edges: &[[&str; 2]]) -> Instance {
    Instance {
        signatures: BTreeMap::from([("N".into(), vec!["server a".into(), "server b".into()])]),
        fields: BTreeMap::from([(
            "edge".into(),
            edges
                .iter()
                .map(|edge| edge.map(String::from).to_vec())
                .collect(),
        )]),
        ..Instance::default()
    }
}

#[test]
fn checks_the_observed_graph_instead_of_searching_for_another() {
    let model = Model::parse(GRAPH).unwrap();
    let dag = graph(&[["server a", "server b"]]);
    let cycle = graph(&[["server a", "server b"], ["server b", "server a"]]);
    assert_eq!(
        model.check_instance(0, &dag, 10_000).unwrap().status,
        CheckStatus::Matched
    );
    let checked = model.check_instance(0, &cycle, 10_000).unwrap();
    assert_eq!(checked.status, CheckStatus::Mismatch);
    assert_eq!(checked.reason, Some(CheckReason::Property));
    let found = model.analyze(0, 100_000).unwrap().instance.unwrap();
    assert_eq!(
        model.check_instance(0, &found, 10_000).unwrap().status,
        CheckStatus::Mismatch
    );
}

#[test]
fn assumptions_scope_and_budget_do_not_become_a_match() {
    let model = Model::parse(GRAPH).unwrap();
    let checked = model
        .check_instance(0, &graph(&[["server a", "server a"]]), 10_000)
        .unwrap();
    assert_eq!(checked.status, CheckStatus::Invalid);
    assert_eq!(checked.reason, Some(CheckReason::Facts));
    let exact = Model::parse("sig N { edge: set N } run {} for 1 but exactly 1 N, 0 Int").unwrap();
    let checked = exact.check_instance(0, &graph(&[]), 10_000).unwrap();
    assert_eq!(checked.status, CheckStatus::Invalid);
    assert_eq!(checked.reason, Some(CheckReason::Scope));
    for budget in [0, 1] {
        let checked = model.check_instance(0, &graph(&[]), budget).unwrap();
        assert_eq!(checked.status, CheckStatus::Unknown);
        assert_eq!(checked.reason, Some(CheckReason::Work));
    }
}

#[test]
fn declaration_multiplicity_and_builtin_integers_are_checked() {
    let model = Model::parse("sig N { edge: N } run { some edge } for 2 but 0 Int").unwrap();
    let checked = model.check_instance(0, &graph(&[]), 10_000).unwrap();
    assert_eq!(checked.status, CheckStatus::Invalid);
    assert_eq!(checked.reason, Some(CheckReason::Declaration));
    let empty = Instance {
        signatures: BTreeMap::from([("N".into(), vec![])]),
        fields: BTreeMap::from([("edge".into(), vec![])]),

        ..Instance::default()
    };
    let empty_model = Model::parse("sig N { edge: some N } run { no N } for 0 but 0 Int").unwrap();
    assert_eq!(
        empty_model
            .check_instance(0, &empty, 10_000)
            .unwrap()
            .status,
        CheckStatus::Matched
    );
    let model = Model::parse("one sig N { value: Int } run { some value } for 0").unwrap();
    let instance = Instance {
        signatures: BTreeMap::from([("N".into(), vec!["opaque".into()])]),
        fields: BTreeMap::from([("value".into(), vec![vec!["opaque".into(), "-8".into()]])]),

        ..Instance::default()
    };
    assert_eq!(
        model.check_instance(0, &instance, 10_000).unwrap().status,
        CheckStatus::Matched
    );
}

#[test]
fn malformed_snapshots_are_errors() {
    let model = Model::parse(GRAPH).unwrap();
    let mut missing = graph(&[]);
    missing.fields.clear();
    let mut unknown = graph(&[]);
    unknown.fields.insert("unknown".into(), vec![]);
    let mut aliases = graph(&[]);
    aliases
        .signatures
        .get_mut("N")
        .unwrap()
        .push("server a".into());
    let duplicate = graph(&[["server a", "server b"], ["server a", "server b"]]);
    let absent_atom = graph(&[["server a", "absent"]]);
    for instance in [missing, unknown, aliases, duplicate, absent_atom] {
        assert_eq!(
            model.check_instance(0, &instance, 10_000).unwrap_err().kind,
            ErrorKind::Protocol
        );
    }
    assert_eq!(
        model
            .check_instance(99, &graph(&[]), 10_000)
            .unwrap_err()
            .kind,
        ErrorKind::Protocol
    );
}

#[test]
fn run_queries_use_their_positive_formula() {
    let model = Model::parse("sig N { edge: set N } run { no edge } for 2 but 0 Int").unwrap();
    assert_eq!(
        model.check_instance(0, &graph(&[]), 10_000).unwrap().status,
        CheckStatus::Matched
    );
    assert_eq!(
        model
            .check_instance(0, &graph(&[["server a", "server b"]]), 10_000)
            .unwrap()
            .status,
        CheckStatus::Mismatch
    );
}

#[test]
fn field_domains_and_all_multiplicities_are_enforced() {
    for source in [
        "sig N { edge: lone N } run {} for 2 but 0 Int",
        "sig N { edge: some N } run {} for 2 but 0 Int",
    ] {
        let model = Model::parse(source).unwrap();
        let instance = if source.contains("lone") {
            graph(&[["server a", "server a"], ["server a", "server b"]])
        } else {
            graph(&[])
        };
        let checked = model.check_instance(0, &instance, 10_000).unwrap();
        assert_eq!(checked.status, CheckStatus::Invalid);
        assert_eq!(checked.reason, Some(CheckReason::Declaration));
    }
    let model = Model::parse("sig A { r: set A } sig B {} run {} for 1 but 0 Int").unwrap();
    let instance = Instance {
        signatures: BTreeMap::from([
            ("A".into(), vec!["a".into()]),
            ("B".into(), vec!["b".into()]),
        ]),
        fields: BTreeMap::from([("r".into(), vec![vec!["a".into(), "b".into()]])]),

        ..Instance::default()
    };
    assert_eq!(
        model.check_instance(0, &instance, 10_000).unwrap().reason,
        Some(CheckReason::Declaration)
    );
}

#[test]
fn fixed_instance_wire_contract_is_strict() {
    let mut engine = ralloy::Engine;
    let request = serde_json::json!({"version":1,"request":{"kind":"check-instance","source":GRAPH,"command":0,"maxWork":10000,"instance":graph(&[])}});
    let result = engine.dispatch(&serde_json::to_vec(&request).unwrap());
    assert_eq!(result["kind"], "instance-check");
    assert_eq!(result["result"]["status"], "matched");
    let mut invalid = request;
    invalid["request"]["instance"]["extra"] = serde_json::json!(true);
    assert_eq!(
        engine.dispatch(&serde_json::to_vec(&invalid).unwrap())["error"]["kind"],
        "protocol"
    );
}
