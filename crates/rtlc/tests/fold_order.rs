use rtlc::{CoreError, SafetyBounds, SafetyProgram, SafetyResult};
use serde_json::{Value, json};

#[path = "../examples/counter.rs"]
mod counter;

const STRINGS: &str =
    "Unsupported TLA input: set fold order for strings requires TLC intern IDs absent from the IR";
const RECORDS: &str = "Unsupported TLA input: set fold order for records requires TLC field-name intern IDs absent from the IR";
const MODEL_VALUES: &str =
    "Unsupported TLA input: set fold order for model values is not implemented";
const FUNCTIONS: &str = "Unsupported TLA input: set fold order for strings, records or functions";
const WIDE_KEYS: &str = "Unsupported TLA input: set fold order for integer function domains spanning more than i32::MAX";

fn bounds(max_work: u32) -> SafetyBounds {
    SafetyBounds {
        max_depth: 10,
        max_states: 100,
        max_work,
        max_temporal_nodes: None,
    }
}

fn int(value: i32) -> Value {
    json!({"op":"literal","value":{"type":"int","value":value.to_string()}})
}

fn text(value: &str) -> Value {
    json!({"op":"data","value":{"type":"string","value":value}})
}

fn model_value(name: &str) -> Value {
    json!({"op":"data","value":{"type":"model-value","value":name}})
}

fn record(fields: Vec<(&str, Value)>) -> Value {
    json!({"op":"record","fields":fields.into_iter().map(|(name, value)| json!([name, value])).collect::<Vec<_>>()})
}

fn set(values: Vec<Value>) -> Value {
    json!({"op":"set","values":values})
}

fn tuple(values: Vec<Value>) -> Value {
    json!({"op":"tuple","values":values})
}

fn cup(left: Value, right: Value) -> Value {
    json!({"op":"cup","left":left,"right":right})
}

fn at(target: Value, key: &str) -> Value {
    json!({"op":"at","target":target,"key":text(key)})
}

fn acc() -> Value {
    json!({"op":"@acc"})
}

fn elem() -> Value {
    json!({"op":"@elem"})
}

fn resolve(template: &Value, accumulator: Value, element: Value) -> Value {
    match template {
        Value::Object(fields) if fields.get("op") == Some(&json!("@acc")) => accumulator,
        Value::Object(fields) if fields.get("op") == Some(&json!("@elem")) => element,
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        resolve(value, accumulator.clone(), element.clone()),
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|value| resolve(value, accumulator.clone(), element.clone()))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn bound(slot: usize) -> Value {
    json!({"op":"bound","slot":slot})
}

fn parameter(slot: usize) -> Value {
    json!({"op":"parameter","slot":slot})
}

fn definition(id: usize, body: Value, captures: usize) -> Value {
    json!({"id":id,"parameterCount":2,"captureCount":captures,"bodyKind":"expression","localScope":2,"body":body})
}

fn wire(version: u8, expected: Value, fold: Value, definitions: Vec<Value>) -> SafetyProgram {
    let mut input = serde_json::to_value(counter::ring()).unwrap();
    input["invariants"][0]["expression"] = json!({"op":"or","left":{"op":"gt","left":{"op":"read","slot":0,"next":false},"right":int(3)},"right":{"op":"eq","left":fold,"right":expected}});
    if version == 2 {
        input["version"] = json!(2);
        input["definitions"] = json!(definitions);
    }
    serde_json::from_value(input).unwrap()
}

fn fold_program(
    version: u8,
    initial: &Value,
    collection: &Value,
    body: &Value,
    expected: &Value,
) -> SafetyProgram {
    let (callback, definitions) = if version == 1 {
        (resolve(body, bound(0), bound(1)), vec![])
    } else {
        (
            json!({"op":"call","definition":0,"arguments":[bound(0),bound(1)],"captures":[]}),
            vec![definition(0, resolve(body, parameter(0), parameter(1)), 0)],
        )
    };
    let fold = json!({"op":"fold","accumulator":0,"initial":initial,"collection":collection,"order":"set-members","body":callback});
    wire(version, expected.clone(), fold, definitions)
}

fn run(program: &SafetyProgram) -> Result<SafetyResult, CoreError> {
    program.check(bounds(100_000))
}

fn holds(program: &SafetyProgram, label: &str) -> bool {
    let result = run(program).unwrap_or_else(|error| panic!("{label}: {}", error.message));
    assert!(
        result.status == "complete" || result.status == "invariant-violation",
        "{}",
        result.status
    );
    result.violation.is_none()
}

fn refusal(program: &SafetyProgram) -> String {
    let error = run(program).unwrap_err();
    assert_eq!(error.code, "protocol");
    error.message
}

struct Accepted {
    name: &'static str,
    initial: Value,
    collection: Value,
    body: Value,
    expected: Value,
    wrong: Value,
}

fn accepted() -> Vec<Accepted> {
    let strings = || {
        vec![
            text("b"),
            text("a"),
            text("A"),
            text("aa"),
            text("B"),
            text(""),
        ]
    };
    let zeta_alpha = || {
        vec![
            record(vec![("zeta", int(0))]),
            record(vec![("alpha", int(0))]),
        ]
    };
    let numbers = || {
        vec![
            record(vec![("a", int(2))]),
            record(vec![("a", int(1)), ("b", int(0))]),
            record(vec![("a", int(1))]),
        ]
    };
    let plus_one = json!({"op":"add","left":acc(),"right":int(1)});
    vec![
        Accepted {
            name: "count records",
            initial: int(0),
            collection: set(zeta_alpha()),
            body: plus_one.clone(),
            expected: int(2),
            wrong: int(1),
        },
        Accepted {
            name: "count strings",
            initial: int(0),
            collection: set(strings()),
            body: plus_one,
            expected: int(6),
            wrong: int(5),
        },
        Accepted {
            name: "constant in the element",
            initial: int(0),
            collection: set(vec![text("b"), text("a")]),
            body: tuple(vec![acc()]),
            expected: tuple(vec![tuple(vec![int(0)])]),
            wrong: tuple(vec![tuple(vec![int(1)])]),
        },
        Accepted {
            name: "union of records",
            initial: set(vec![]),
            collection: set(zeta_alpha()),
            body: cup(acc(), set(vec![elem()])),
            expected: set(zeta_alpha()),
            wrong: set(vec![record(vec![("zeta", int(0))])]),
        },
        Accepted {
            name: "union of strings",
            initial: set(vec![]),
            collection: set(strings()),
            body: cup(acc(), set(vec![elem()])),
            expected: set(strings()),
            wrong: set(vec![text("a")]),
        },
        Accepted {
            name: "union with the accumulator on the right",
            initial: set(vec![]),
            collection: set(vec![tuple(vec![text("b")]), tuple(vec![text("a")])]),
            body: cup(set(vec![tuple(vec![elem()])]), acc()),
            expected: set(vec![
                tuple(vec![tuple(vec![text("a")])]),
                tuple(vec![tuple(vec![text("b")])]),
            ]),
            wrong: set(vec![tuple(vec![tuple(vec![text("a")])])]),
        },
        Accepted {
            name: "union of projections",
            initial: set(vec![]),
            collection: set(numbers()),
            body: cup(acc(), set(vec![at(elem(), "a")])),
            expected: set(vec![int(1), int(2)]),
            wrong: set(vec![int(1)]),
        },
        Accepted {
            name: "union under a condition on the element",
            initial: set(vec![]),
            collection: set(numbers()),
            body: json!({"op":"if","condition":{"op":"gt","left":at(elem(), "a"),"right":int(1)},"consequent":cup(acc(), set(vec![elem()])),"alternative":acc()}),
            expected: set(vec![record(vec![("a", int(2))])]),
            wrong: set(vec![]),
        },
        Accepted {
            name: "union of variant values",
            initial: set(vec![]),
            collection: set(vec![
                record(vec![("tag", text("B")), ("value", int(1))]),
                record(vec![("tag", text("A")), ("value", int(2))]),
            ]),
            body: cup(acc(), set(vec![at(elem(), "value")])),
            expected: set(vec![int(1), int(2)]),
            wrong: set(vec![int(1)]),
        },
        Accepted {
            name: "singleton string",
            initial: int(0),
            collection: set(vec![text("only")]),
            body: elem(),
            expected: text("only"),
            wrong: text("other"),
        },
        Accepted {
            name: "singleton record",
            initial: int(0),
            collection: set(vec![record(vec![("zeta", int(0))])]),
            body: elem(),
            expected: record(vec![("zeta", int(0))]),
            wrong: record(vec![("alpha", int(0))]),
        },
        Accepted {
            name: "singleton read of the element after an accumulator",
            initial: tuple(vec![]),
            collection: set(vec![text("only")]),
            body: json!({"op":"append","left":acc(),"right":elem()}),
            expected: tuple(vec![text("only")]),
            wrong: tuple(vec![]),
        },
    ]
}

#[test]
fn order_independent_folds_over_records_and_strings_are_accepted_in_both_ir_versions() {
    for version in [1, 2] {
        for case in accepted() {
            let program = fold_program(
                version,
                &case.initial,
                &case.collection,
                &case.body,
                &case.expected,
            );
            assert!(holds(&program, case.name), "v{version} {}", case.name);
            let wrong = fold_program(
                version,
                &case.initial,
                &case.collection,
                &case.body,
                &case.wrong,
            );
            assert!(
                !holds(&wrong, case.name),
                "v{version} {} control",
                case.name
            );
        }
    }
}

#[test]
fn a_set_of_one_element_is_folded_whatever_it_contains() {
    let wide = json!({"op":"data","value":{"type":"map","value":[[{"type":"int","value":"-2147483648"},{"type":"int","value":"0"}],[{"type":"int","value":"0"},{"type":"int","value":"0"}]]}});
    let boolean_key = json!({"op":"data","value":{"type":"map","value":[[{"type":"bool","value":false},{"type":"int","value":"0"}]]}});
    let size =
        |value: Value| json!({"op":"cardinality","argument":{"op":"domain","argument":value}});
    let truth = |value: bool| json!({"op":"literal","value":{"type":"bool","value":value}});
    let wrapped = |value: Value| tuple(vec![tuple(vec![value])]);
    let cases = [
        (wide, size(elem()), int(2), int(3)),
        (boolean_key, size(elem()), int(1), int(2)),
        (
            model_value("m"),
            json!({"op":"eq","left":elem(),"right":elem()}),
            truth(true),
            truth(false),
        ),
        (
            tuple(vec![set(vec![text("a"), text("b")])]),
            tuple(vec![elem()]),
            tuple(vec![tuple(vec![set(vec![text("a"), text("b")])])]),
            wrapped(set(vec![text("a")])),
        ),
        (
            tuple(vec![record(vec![("a", int(0))])]),
            tuple(vec![elem()]),
            wrapped(record(vec![("a", int(0))])),
            wrapped(record(vec![("a", int(1))])),
        ),
    ];
    for version in [1, 2] {
        for (index, (element, body, expected, wrong)) in cases.iter().enumerate() {
            let program = fold_program(
                version,
                &int(0),
                &set(vec![element.clone()]),
                body,
                expected,
            );
            assert!(holds(&program, "single"), "v{version} case {index}");
            let other = fold_program(version, &int(0), &set(vec![element.clone()]), body, wrong);
            assert!(
                !holds(&other, "single control"),
                "v{version} case {index} control"
            );
        }
    }
}

#[test]
fn a_singleton_body_error_is_reported_as_it_is() {
    for version in [1, 2] {
        let body = json!({"op":"head","argument":tuple(vec![])});
        let program = fold_program(version, &int(0), &set(vec![text("only")]), &body, &int(0));
        let error = run(&program).unwrap_err();
        assert!(!error.message.contains("fold order"), "{}", error.message);
    }
}

#[test]
fn the_overflow_of_an_independent_body_is_reported() {
    for version in [1, 2] {
        let body = json!({"op":"add","left":acc(),"right":int(1)});
        let collection = set(vec![
            record(vec![("zeta", int(0))]),
            record(vec![("alpha", int(0))]),
        ]);
        let program = fold_program(version, &int(2_147_483_646), &collection, &body, &int(0));
        let message = refusal(&program);
        assert!(message.contains("overflow"), "{message}");
    }
}

struct Refused {
    name: &'static str,
    initial: Value,
    collection: Value,
    body: Value,
    message: &'static str,
}

fn refused() -> Vec<Refused> {
    let strings = || vec![text("b"), text("a")];
    let records = || {
        vec![
            record(vec![("zeta", int(0))]),
            record(vec![("alpha", int(0))]),
        ]
    };
    let appended = json!({"op":"append","left":acc(),"right":elem()});
    vec![
        Refused {
            name: "append of strings",
            initial: tuple(vec![]),
            collection: set(strings()),
            body: appended.clone(),
            message: STRINGS,
        },
        Refused {
            name: "append of records",
            initial: tuple(vec![]),
            collection: set(records()),
            body: appended,
            message: RECORDS,
        },
        Refused {
            name: "last element",
            initial: int(0),
            collection: set(records()),
            body: elem(),
            message: RECORDS,
        },
        Refused {
            name: "sum of a field",
            initial: int(0),
            collection: set(vec![
                record(vec![("v", int(1))]),
                record(vec![("v", int(2))]),
            ]),
            body: json!({"op":"add","left":acc(),"right":at(elem(), "v")}),
            message: RECORDS,
        },
        Refused {
            name: "union that reads the accumulator in the added set",
            initial: set(vec![]),
            collection: set(strings()),
            body: cup(
                acc(),
                set(vec![tuple(vec![
                    elem(),
                    json!({"op":"cardinality","argument":acc()}),
                ])]),
            ),
            message: STRINGS,
        },
        Refused {
            name: "condition that reads the accumulator",
            initial: set(vec![]),
            collection: set(strings()),
            body: json!({"op":"if","condition":{"op":"eq","left":json!({"op":"cardinality","argument":acc()}),"right":int(0)},"consequent":cup(acc(), set(vec![elem()])),"alternative":acc()}),
            message: STRINGS,
        },
        Refused {
            name: "branch that is not the accumulator",
            initial: set(vec![]),
            collection: set(strings()),
            body: json!({"op":"if","condition":{"op":"eq","left":elem(),"right":text("a")},"consequent":set(vec![elem()]),"alternative":cup(acc(), set(vec![elem()]))}),
            message: STRINGS,
        },
        Refused {
            name: "difference from the accumulator",
            initial: set(vec![]),
            collection: set(strings()),
            body: json!({"op":"difference","left":acc(),"right":set(vec![elem()])}),
            message: STRINGS,
        },
        Refused {
            name: "union error of a missing field",
            initial: set(vec![]),
            collection: set(vec![
                record(vec![("a", int(1))]),
                record(vec![("b", int(2))]),
            ]),
            body: cup(acc(), set(vec![at(elem(), "a")])),
            message: RECORDS,
        },
        Refused {
            name: "union into a value that is not a set",
            initial: int(0),
            collection: set(strings()),
            body: cup(acc(), set(vec![elem()])),
            message: STRINGS,
        },
        Refused {
            name: "model value next to a string",
            initial: int(0),
            collection: set(vec![text("a"), model_value("m")]),
            body: json!({"op":"add","left":acc(),"right":int(1)}),
            message: "",
        },
    ]
}

#[test]
fn folds_that_depend_on_the_order_keep_their_refusal() {
    for version in [1, 2] {
        for case in refused() {
            let program = fold_program(
                version,
                &case.initial,
                &case.collection,
                &case.body,
                &int(0),
            );
            let message = refusal(&program);
            if case.message.is_empty() {
                assert!(
                    message == STRINGS || message == MODEL_VALUES || message == FUNCTIONS,
                    "v{version} {}: {message}",
                    case.name
                );
            } else {
                assert_eq!(message, case.message, "v{version} {}", case.name);
            }
        }
    }
}

#[test]
fn a_reason_that_cannot_be_ignored_wins_over_an_accepted_one_in_scan_order() {
    let wide = json!({"op":"data","value":{"type":"map","value":[[{"type":"int","value":"-2147483648"},{"type":"int","value":"0"}],[{"type":"int","value":"0"},{"type":"int","value":"0"}]]}});
    let body = json!({"op":"add","left":acc(),"right":int(1)});
    let collection = set(vec![
        tuple(vec![text("a"), wide.clone()]),
        tuple(vec![text("b"), wide]),
    ]);
    for version in [1, 2] {
        let program = fold_program(version, &int(0), &collection, &body, &int(2));
        let message = refusal(&program);
        assert!(message == STRINGS || message == WIDE_KEYS, "{message}");
    }
}

#[test]
fn a_union_body_error_that_is_not_an_order_refusal_passes_through() {
    let collection = set(vec![
        record(vec![("a", int(1)), ("b", int(0))]),
        record(vec![("a", int(2))]),
    ]);
    for version in [1, 2] {
        let body = cup(
            acc(),
            set(vec![json!({"op":"variant-tag","argument":elem()})]),
        );
        let program = fold_program(version, &set(vec![]), &collection, &body, &int(0));
        let message = refusal(&program);
        assert!(message.contains("VariantTag"), "{message}");
    }
}

#[test]
fn a_union_fold_exhausts_its_budget_instead_of_turning_into_a_refusal() {
    let strings = (0..20).map(|n| text(&format!("s{n}"))).collect();
    let collection = set(strings);
    for version in [1, 2] {
        let body = cup(acc(), set(vec![elem()]));
        let program = fold_program(version, &set(vec![]), &collection, &body, &collection);
        let complete = program.check(bounds(100_000)).unwrap();
        assert_eq!(complete.status, "complete");
        for budget in 1..complete.work {
            let result = program.check(bounds(budget)).unwrap();
            assert_eq!(result.status, "bounded", "v{version} budget {budget}");
        }
    }
}

#[test]
fn captures_of_the_fold_callback_must_not_read_the_accumulator_or_the_element() {
    let strings = set(vec![text("b"), text("a")]);
    let callee = cup(parameter(0), set(vec![json!({"op":"capture","slot":0})]));
    for (capture, accepted) in [(int(5), true), (bound(0), false), (bound(1), false)] {
        let fold = json!({"op":"fold","accumulator":0,"initial":set(vec![]),"collection":strings,"order":"set-members","body":{"op":"call","definition":0,"arguments":[bound(0),bound(1)],"captures":[capture]}});
        let expected = if accepted {
            set(vec![int(5)])
        } else {
            set(vec![])
        };
        let program = wire(2, expected, fold, vec![definition(0, callee.clone(), 1)]);
        if accepted {
            assert!(holds(&program, "capture"));
        } else {
            assert_eq!(refusal(&program), STRINGS);
        }
    }
}

#[test]
fn callback_calls_must_pass_the_accumulator_then_the_element() {
    let strings = set(vec![text("b"), text("a")]);
    let callee = cup(parameter(0), set(vec![parameter(1)]));
    for (arguments, accepted) in [
        (vec![bound(0), bound(1)], true),
        (vec![bound(1), bound(0)], false),
        (vec![bound(0)], false),
        (vec![bound(0), bound(1), bound(1)], false),
        (vec![bound(0), tuple(vec![bound(1)])], false),
    ] {
        let fold = json!({"op":"fold","accumulator":0,"initial":set(vec![]),"collection":strings,"order":"set-members","body":{"op":"call","definition":0,"arguments":arguments,"captures":[]}});
        let program = wire(
            2,
            strings.clone(),
            fold,
            vec![definition(0, callee.clone(), 0)],
        );
        match run(&program) {
            Ok(result) => assert!(
                accepted && result.violation.is_none(),
                "{arguments:?}",
                arguments = program.definitions.is_some()
            ),
            Err(error) => {
                assert!(!accepted, "{}", error.message);
            }
        }
    }
}

#[test]
fn a_nested_frame_that_reuses_a_parameter_number_is_not_accepted() {
    let strings = set(vec![text("b"), text("a")]);
    let frame = json!({"op":"frame","arguments":[parameter(1)],"captures":[],"parameterCount":1,"captureCount":0,"localScope":0,"body":set(vec![parameter(0)])});
    let callee = cup(parameter(0), frame);
    let fold = json!({"op":"fold","accumulator":0,"initial":set(vec![]),"collection":strings.clone(),"order":"set-members","body":{"op":"call","definition":0,"arguments":[bound(0),bound(1)],"captures":[]}});
    let program = wire(2, strings, fold, vec![definition(0, callee, 0)]);
    assert_eq!(refusal(&program), STRINGS);
}

#[test]
fn a_fold_over_integers_keeps_its_result_and_work() {
    for version in [1, 2] {
        let body = json!({"op":"add","left":acc(),"right":elem()});
        let program = fold_program(
            version,
            &int(0),
            &set(vec![int(3), int(1), int(2)]),
            &body,
            &int(6),
        );
        let result = run(&program).unwrap();
        assert_eq!(result.status, "complete");
        assert!(result.violation.is_none());
        assert_eq!(
            result.work,
            if version == 1 { 35 } else { 71 },
            "v{version}"
        );
    }
}

fn nested(version: u8) -> SafetyProgram {
    let outer = set(vec![
        record(vec![("w", int(2))]),
        record(vec![("w", int(3))]),
    ]);
    let expected = set(vec![int(1), int(2), int(3)]);
    let inner_set = |element: Value| {
        set(vec![
            record(vec![("v", int(1))]),
            record(vec![("v", at(element, "w"))]),
        ])
    };
    if version == 1 {
        let inner = json!({"op":"fold","accumulator":2,"initial":set(vec![]),"collection":inner_set(bound(1)),"order":"set-members","body":cup(bound(2), set(vec![at(bound(3), "v")]))});
        let fold = json!({"op":"fold","accumulator":0,"initial":set(vec![]),"collection":outer,"order":"set-members","body":cup(bound(0), inner)});
        wire(1, expected, fold, vec![])
    } else {
        let inner_callback = definition(0, cup(parameter(0), set(vec![at(parameter(1), "v")])), 0);
        let inner = json!({"op":"fold","accumulator":0,"initial":set(vec![]),"collection":inner_set(parameter(1)),"order":"set-members","body":{"op":"call","definition":0,"arguments":[bound(0),bound(1)],"captures":[]}});
        let outer_callback = definition(1, cup(parameter(0), inner), 0);
        let fold = json!({"op":"fold","accumulator":0,"initial":set(vec![]),"collection":outer,"order":"set-members","body":{"op":"call","definition":1,"arguments":[bound(0),bound(1)],"captures":[]}});
        wire(2, expected, fold, vec![inner_callback, outer_callback])
    }
}

#[test]
fn a_union_fold_nested_in_a_union_fold_is_accepted() {
    for version in [1, 2] {
        let program = nested(version);
        assert!(holds(&program, "nested"), "v{version}");
        let mut wrong = serde_json::to_value(&program).unwrap();
        wrong["invariants"][0]["expression"]["right"]["right"] = set(vec![int(1), int(2)]);
        let wrong: SafetyProgram = serde_json::from_value(wrong).unwrap();
        assert!(!holds(&wrong, "nested"), "v{version} control");
    }
}
