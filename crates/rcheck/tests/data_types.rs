use serde_json::json;

const SOURCE: &str = include_str!("../../../examples/model-patterns/payment-data/payment.ideal");

#[test]
fn data_types_survive_ir_and_generate_distinct_types() {
    let model = rmodel::parse(SOURCE).unwrap();
    let ir = rcheck::ir::to_ir(&model).unwrap();
    let payload = serde_json::to_value(&ir).unwrap();
    assert_eq!(payload["records"][0]["data"], "newtype");
    assert_eq!(payload["records"][2]["data"], "record");
    let (decoded, omitted) = rcheck::ir::parse(&payload.to_string()).unwrap();
    assert!(omitted.is_empty());
    assert_eq!(rcheck::ir::to_ir(&decoded).unwrap(), ir);
    let mut invalid = payload;
    invalid["records"][0]["fields"][0]["name"] = json!("units");
    assert!(
        rcheck::ir::parse(&invalid.to_string())
            .unwrap_err()
            .contains("a newtype needs exactly one value field")
    );
    let states = rcheck::conform::reachable_states(&decoded).unwrap();
    assert!(
        serde_json::to_value(states)
            .unwrap()
            .to_string()
            .contains(&json!({"amount":{"value":1},"fee":{"value":0}}).to_string())
    );
    let typescript = rmodel::bindings::typescript(&decoded);
    assert!(typescript.contains("readonly [$data]: \"Amount\""));
    assert!(typescript.contains("readonly [$data]: \"Fee\""));
    assert!(typescript.contains("from_Amount as decodeAmount"));
    assert!(rmodel::bindings::go(&decoded, "payment").contains("type Amount struct"));
    assert!(rmodel::bindings::rust(&decoded, SOURCE).contains("pub struct Amount"));
}

#[test]
fn data_types_reject_swapped_values_and_ordinary_records_have_no_call_constructor() {
    for (changed, expected) in [
        (
            SOURCE.replace("amount: amount, fee: fee", "amount: fee, fee: amount"),
            "Request.amount needs one value of Amount",
        ),
        (
            SOURCE.replace("data Amount = Nat", "record Amount { value: Nat }"),
            "unknown variant tag Amount",
        ),
        (
            SOURCE.replace("Amount(1)", "Amount()"),
            "expects one argument",
        ),
    ] {
        let error = rmodel::parse(&changed).unwrap_err();
        assert!(error.message.contains(expected), "{error}");
    }
}

#[test]
fn refinements_filter_arguments_and_survive_json_ir() {
    let source = "model Payment
data Amount = Nat where { 0 <= value and value <= 1 }
data Fee = Nat
data Request = { amount: Amount, fee: Fee } where { fee.value <= amount.value }
var request: lone Request
init { no request }
behavior submit(input: Request) {
  guard { no request }
  update { request = input }
}
invariant Valid { some request implies request.fee.value <= request.amount.value }
scope Int = 0..2, steps = 2";
    let model = rmodel::parse(source).unwrap();
    let wire = serde_json::to_string(&rcheck::ir::to_ir(&model).unwrap()).unwrap();
    let (decoded, omitted) = rcheck::ir::parse(&wire).unwrap();
    assert!(omitted.is_empty());
    for model in [&model, &decoded] {
        let machine = rmodel::reference::Machine::new(model).unwrap();
        let config = machine.configurations().next().unwrap();
        let initial = machine.initial(&config);
        assert_eq!(machine.steps(&config, &initial[0]).len(), 3);
        let report = rmodel::reference::analyze(model, 100);
        assert_eq!(report.scope, rmodel::reference::Outcome::Holds);
        assert_eq!(report.declarations, rmodel::reference::Outcome::Holds);
    }
    assert_eq!(
        rcheck::ir::to_ir(&model).unwrap(),
        rcheck::ir::to_ir(&decoded).unwrap()
    );
}

const REFINED: &str =
    include_str!("../../../examples/model-patterns/payment-data/payment-refined.ideal");

#[test]
fn refinement_preflight_rejects_a_well_shaped_but_invalid_request() {
    use rcheck::conform::{ValidationError, Validator};
    let model = rmodel::parse(REFINED).unwrap();
    let validator = Validator::new(&model).unwrap();
    let session = validator
        .start(&json!({}), &json!({"request": null}))
        .unwrap();
    let command = |amount, fee| {
        json!({
            "kind": "submit",
            "input": {"amount": {"value": amount}, "fee": {"value": fee}}
        })
    };
    assert_eq!(
        session.validate(&command(1, 1)).unwrap()["request"]["fee"]["value"],
        1
    );
    assert!(matches!(
        session.validate(&command(1, 2)),
        Err(ValidationError::InvalidInput(_))
    ));
    assert_eq!(session.snapshot(), json!({"request": null}));
}

#[test]
fn refinement_violation_in_an_update_is_a_declaration_failure() {
    use rmodel::reference::Outcome;
    let source = REFINED
        .replace("submit(input: Request)", "submit()")
        .replace(
            "request = input",
            "request = Request { amount: Amount(0), fee: Fee(1) }",
        );
    let model = rmodel::parse(&source).unwrap();
    let report = rmodel::reference::analyze(&model, 100);
    assert_eq!(report.declarations, Outcome::Violated { depth: 1 });
    assert_eq!(report.scope, Outcome::Holds);
}

#[test]
fn refinement_boundary_cases_use_the_type_bound_outside_search_scope() {
    let model = rmodel::parse(REFINED).unwrap();
    let cases = rcheck::conform::data_boundary_cases(&model).unwrap();
    let cases = cases["cases"].as_array().unwrap();
    for (value, valid) in [(999, true), (1000, true), (1001, false)] {
        assert!(
            cases.contains(&json!({"sort": "Amount", "input": {"value": value}, "valid": valid}))
        );
    }
    for (fee, valid) in [(0, true), (1, true), (2, false)] {
        assert!(cases.contains(&json!({
            "sort": "Request",
            "input": {"amount": {"value": 1}, "fee": {"value": fee}},
            "valid": valid
        })));
    }
}

#[test]
fn refinement_paths_are_numeric_and_value_local_in_source_and_ir() {
    let model = rmodel::parse(REFINED).unwrap();
    let invalid = REFINED.replace(
        "fee.value <= amount.value",
        "fee.value <= request.amount.value",
    );
    assert!(
        rmodel::parse(&invalid)
            .unwrap_err()
            .message
            .contains("unknown where field request")
    );
    let invalid = REFINED.replace("fee.value <= amount.value", "fee <= amount");
    assert!(
        rmodel::parse(&invalid)
            .unwrap_err()
            .message
            .contains("must be Int or Nat")
    );
    let mut ir = serde_json::to_value(rcheck::ir::to_ir(&model).unwrap()).unwrap();
    ir["records"][0]["where"][0]["left"] = json!({"field": ["typo"]});
    assert!(
        rcheck::ir::parse(&ir.to_string())
            .unwrap_err()
            .contains("unknown where field typo")
    );
}

#[test]
fn refinement_lowerings_agree_with_reference_on_domains_and_bad_writes() {
    use rcheck::{Budget, Selection, Verdict};
    let bad = REFINED
        .replace("submit(input: Request)", "submit()")
        .replace(
            "request = input",
            "request = Request { amount: Amount(0), fee: Fee(1) }",
        );
    let escaped = bad.replace("Amount(0)", "Amount(3)");
    let empty = REFINED.replace(
        "init { no request }",
        "init { request = Request { amount: Amount(0), fee: Fee(1) } }",
    );
    let indexed = REFINED.replace("0..2", "0..9");
    let filtered = REFINED.replace("0..2", "0..9")
        .replace("value <= 1000", "value <= 2")
        .replace("reachable Submitted", "invariant Limited { some request implies request.amount.value <= 2 }\nreachable Submitted");
    for (source, question, expected) in [
        (REFINED, "Submitted", Verdict::Reachable),
        (&indexed, "Submitted", Verdict::Reachable),
        (&filtered, "Limited", Verdict::Holds),
        (&bad, rmodel::DECLARATIONS, Verdict::Violated),
        (&indexed, rmodel::DECLARATIONS, Verdict::Holds),
        (&filtered, rmodel::DECLARATIONS, Verdict::Holds),
        (&escaped, rmodel::DECLARATIONS, Verdict::Unknown),
        (&empty, rmodel::DECLARATIONS, Verdict::Empty),
    ] {
        let model = rmodel::parse(source).unwrap();
        for checker in ["reference", "explicit", "rprob"] {
            let report = rcheck::check(
                &model,
                question,
                &Selection::Named(checker.into()),
                Budget::default(),
            )
            .unwrap_or_else(|error| panic!("{checker} {question}: {error}"));
            let answer = &report.answers[0];
            assert_eq!(answer.verdict, expected, "{checker} {question}: {answer:?}");
        }
    }
}

#[test]
fn refinement_witnesses_decode_the_filtered_argument_order() {
    for source in [REFINED.to_owned(), REFINED.replace("0..2", "0..9")] {
        let model = rmodel::parse(&source).unwrap();
        let reference = rmodel::reference::witness(&model, "Submitted", 1_000)
            .unwrap()
            .witness
            .unwrap();
        let compiled = rprob::witness(&model, "Submitted", 1_000)
            .unwrap()
            .witness
            .unwrap();
        assert_eq!(compiled.commands[0].action, "submit");
        assert_eq!(compiled.commands[0].args, reference.commands[0].args);
        assert_eq!(compiled.final_state, reference.final_state);
    }
}

#[test]
fn nested_refinements_are_checked_in_collections_and_packed_rows() {
    use rcheck::{Budget, Selection, Verdict};
    let wrapped = "model Wrapped
data Delta = Int where { value >= -1, value != 1 }
variant Item = NoItem | ItemValue(Delta)
sequence Items = Item, max = 1
var items: one Items
init { items = Items[] }
behavior write() { update { items = Items[ItemValue(Delta(1))] } }
scope Int = -1..1, steps = 2";
    let rows = REFINED
        .replace(
            "var request: lone Request",
            "enum Account = Ren | Kai
var requests: Account -> lone Request",
        )
        .replace("no request", "no requests")
        .replace("request = input", "requests = Ren -> input")
        .replace("request.", "Ren.requests.")
        .replace("some request implies", "some Ren.requests implies")
        .replace("0..2", "0..9");
    let bad_rows = rows.replace("submit(input: Request)", "submit()").replace(
        "Ren -> input",
        "Ren -> Request { amount: Amount(0), fee: Fee(1) }",
    );
    for (source, question, expected) in [
        (wrapped, rmodel::DECLARATIONS, Verdict::Violated),
        (&rows, "Submitted", Verdict::Reachable),
        (&rows, rmodel::DECLARATIONS, Verdict::Holds),
        (&bad_rows, rmodel::DECLARATIONS, Verdict::Violated),
    ] {
        let model = rmodel::parse(source).unwrap();
        for checker in ["reference", "explicit", "rprob"] {
            let report = rcheck::check(
                &model,
                question,
                &Selection::Named(checker.into()),
                Budget::default(),
            )
            .unwrap_or_else(|error| panic!("{checker} {question}: {error}"));
            assert_eq!(report.answers[0].verdict, expected, "{checker} {question}");
        }
    }
}
