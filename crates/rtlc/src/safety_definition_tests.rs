use crate::ir::SafetyBounds;
use crate::{SafetyProgram, SafetyResult};
use serde_json::{Value, json};

fn literal(value: bool) -> Value {
    json!({"op":"literal","value":{"type":"bool","value":value}})
}

fn call(definition: usize, arguments: Vec<Value>, captures: Vec<Value>) -> Value {
    json!({"op":"call","definition":definition,"arguments":arguments,"captures":captures})
}

fn definition(id: usize, body: Value) -> Value {
    json!({"id":id,"parameterCount":1,"captureCount":0,"bodyKind":"expression","localScope":0,"body":body})
}

fn program() -> Value {
    json!({
        "format":"modelt.safety-program",
        "version":2,
        "name":"Shared",
        "variables":["x"],
        "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":{"op":"literal","value":{"type":"int","value":"0"}}}]}],
        "next":[{"name":"Next","steps":[{"kind":"assign","slot":0,"value":{"op":"read","slot":0,"next":false}}]}],
        "invariants":[{"name":"Safe","expression":call(0, vec![literal(true)], vec![])}],
        "assumptions":[],
        "checkDeadlock":false,
        "definitions":[definition(0,json!({"op":"parameter","slot":0}))]
    })
}

fn check(input: Value) -> Result<SafetyResult, crate::CoreError> {
    let program: SafetyProgram = serde_json::from_value(input).unwrap();
    program.check(SafetyBounds {
        max_depth: 64,
        max_states: 10000,
        max_work: 1000000,
        max_temporal_nodes: None,
    })
}

fn accepted(input: Value) {
    let result = check(input).unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 1);
    assert_eq!(result.generated, 2);
}

fn refused(input: Value) {
    let program: SafetyProgram = serde_json::from_value(input.clone()).unwrap();
    assert!(crate::safety_definitions::validate_program(&program).is_err());
    let error = check(input).unwrap_err();
    assert_eq!(error.code, "protocol");
    assert!(!error.message.is_empty());
}

#[test]
fn v2_definition_acceptance_is_paired_with_self_cycle_refusal() {
    let valid = program();
    accepted(valid.clone());
    let mut invalid = valid;
    invalid["definitions"][0]["body"] = call(0, vec![json!({"op":"parameter","slot":0})], vec![]);
    refused(invalid);
}

#[test]
fn explicitly_recursive_definitions_terminate_or_report_a_bound() {
    let mut input = program();
    input["definitions"][0]["recursive"] = json!(true);
    let parameter = json!({"op":"parameter","slot":0});
    let integer = |value: &str| json!({"op":"literal","value":{"type":"int","value":value}});
    input["invariants"][0]["expression"] = call(0, vec![integer("5")], vec![]);
    input["definitions"][0]["body"] = json!({"op":"if","condition":{"op":"le","left":parameter,"right":integer("0")},"consequent":literal(true),"alternative":call(0,vec![json!({"op":"sub","left":parameter,"right":integer("1")})],vec![])});
    accepted(input.clone());
    input["definitions"][0]["body"] = call(0, vec![parameter], vec![]);
    let result = check(input.clone()).unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("evaluation"));
    let program: SafetyProgram = serde_json::from_value(input).unwrap();
    let result = program
        .check(SafetyBounds {
            max_depth: 64,
            max_states: 1000,
            max_work: 7,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("work"));
    assert_eq!(result.work, 7);
}

#[test]
fn every_member_of_a_recursive_component_must_be_marked_and_phase_checked() {
    let mut input = program();
    input["definitions"][0]["recursive"] = json!(true);
    input["definitions"]
        .as_array_mut()
        .unwrap()
        .push(definition(1, call(0, vec![literal(true)], vec![])));
    input["definitions"][0]["body"] = json!({"op":"if","condition":literal(true),"consequent":literal(true),"alternative":call(1,vec![literal(true)],vec![])});
    refused(input.clone());
    input["definitions"][1]["recursive"] = json!(true);
    accepted(input.clone());
    input["definitions"][1]["body"] = json!({"op":"and","left":call(0,vec![literal(true)],vec![]),"right":{"op":"eq","left":{"op":"read","slot":0,"next":true},"right":{"op":"read","slot":0,"next":false}}});
    refused(input);
}

#[test]
fn v2_mutual_cycle_is_refused_even_in_an_unreached_branch() {
    let mut valid = program();
    valid["definitions"]
        .as_array_mut()
        .unwrap()
        .push(definition(1, literal(true)));
    accepted(valid.clone());
    let mut invalid = valid;
    invalid["definitions"][0]["body"] = json!({"op":"if","condition":literal(true),"consequent":literal(true),"alternative":call(1,vec![literal(true)],vec![])});
    invalid["definitions"][1]["body"] = call(0, vec![literal(true)], vec![]);
    refused(invalid);
}

#[test]
fn v2_unknown_definition_ids_are_refused_at_roots_and_in_definitions() {
    let valid = program();
    accepted(valid.clone());
    for path in ["root", "definition"] {
        let mut invalid = valid.clone();
        let expression = call(1, vec![literal(true)], vec![]);
        if path == "root" {
            invalid["invariants"][0]["expression"] = expression;
        } else {
            invalid["definitions"][0]["body"] = expression;
        }
        refused(invalid);
    }
}

#[test]
fn v2_call_arity_and_capture_count_are_checked_before_evaluation() {
    let valid = program();
    accepted(valid.clone());
    for arguments in [vec![], vec![literal(true), literal(true)]] {
        let mut invalid = valid.clone();
        invalid["invariants"][0]["expression"] = call(0, arguments, vec![]);
        refused(invalid);
    }
    let mut invalid = valid.clone();
    invalid["invariants"][0]["expression"] = call(0, vec![literal(true)], vec![literal(true)]);
    refused(invalid);
    let mut valid_capture = valid;
    valid_capture["definitions"][0]["captureCount"] = json!(1);
    valid_capture["definitions"][0]["body"] = json!({"op":"capture","slot":0});
    valid_capture["invariants"][0]["expression"] =
        call(0, vec![literal(true)], vec![literal(true)]);
    accepted(valid_capture.clone());
    valid_capture["invariants"][0]["expression"]["captures"] = json!([]);
    refused(valid_capture);
}

#[test]
fn v2_parameter_and_capture_slots_cannot_escape_the_lexical_frame() {
    let valid = program();
    accepted(valid.clone());
    for expression in [
        json!({"op":"parameter","slot":1}),
        json!({"op":"capture","slot":0}),
        json!({"op":"bound","slot":0}),
    ] {
        let mut invalid = valid.clone();
        invalid["definitions"][0]["body"] = expression;
        refused(invalid);
    }
    for op in ["parameter", "capture"] {
        let mut invalid = valid.clone();
        invalid["invariants"][0]["expression"] = json!({"op":op,"slot":0});
        refused(invalid);
    }
    let mut valid_capture = valid;
    valid_capture["definitions"][0]["captureCount"] = json!(1);
    valid_capture["definitions"][0]["body"] = json!({"op":"capture","slot":0});
    valid_capture["invariants"][0]["expression"]["captures"] = json!([literal(true)]);
    accepted(valid_capture.clone());
    valid_capture["definitions"][0]["body"]["slot"] = json!(1);
    refused(valid_capture);
}

#[test]
fn v2_unused_arguments_still_require_valid_references_and_local_slots() {
    let mut valid = program();
    valid["definitions"][0]["body"] = literal(true);
    accepted(valid.clone());
    for expression in [
        json!({"op":"bound","slot":0}),
        json!({"op":"parameter","slot":0}),
        json!({"op":"capture","slot":0}),
        call(1, vec![], vec![]),
    ] {
        let mut invalid = valid.clone();
        invalid["invariants"][0]["expression"]["arguments"][0] = expression;
        refused(invalid);
    }
}

#[test]
fn v2_callee_and_argument_binders_both_start_at_frame_local_slot_zero() {
    let mut valid = program();
    valid["definitions"][0]["localScope"] = json!(1);
    valid["definitions"][0]["body"] = json!({"op":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[{"op":"literal","value":{"type":"int","value":"9"}}]}}],"body":{"op":"parameter","slot":0}});
    valid["invariants"][0]["expression"]["arguments"] = json!([{"op":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[{"op":"literal","value":{"type":"int","value":"0"}}]}}],"body":{"op":"eq","left":{"op":"bound","slot":0},"right":{"op":"literal","value":{"type":"int","value":"0"}}}}]);
    accepted(valid.clone());
    let mut invalid = valid.clone();
    invalid["definitions"][0]["body"]["bounds"][0]["slot"] = json!(1);
    refused(invalid);
    let mut invalid = valid.clone();
    invalid["invariants"][0]["expression"]["arguments"][0]["bounds"][0]["slot"] = json!(1);
    refused(invalid);
    valid["definitions"][0]["localScope"] = json!(0);
    refused(valid);
}

#[test]
fn v2_local_scope_does_not_raise_the_existing_binder_limit() {
    let mut valid = program();
    valid["definitions"][0]["localScope"] = json!(40);
    accepted(valid.clone());
    valid["definitions"][0]["localScope"] = json!(41);
    refused(valid);
}

#[test]
fn v2_phase_validation_follows_calls_and_primed_parameters() {
    let mut valid = program();
    valid["definitions"][0]["body"] = json!({"op":"prime","argument":{"op":"parameter","slot":0}});
    valid["invariants"] = json!([]);
    valid["actionProperties"] = json!([{"name":"Same","expression":{"op":"eq","left":call(0,vec![json!({"op":"read","slot":0,"next":false})],vec![]),"right":{"op":"read","slot":0,"next":true}}}]);
    accepted(valid.clone());
    let mut invalid = valid.clone();
    invalid["invariants"] = invalid["actionProperties"].take();
    invalid["actionProperties"] = json!([]);
    refused(invalid);
    let mut invalid = valid;
    invalid["actionProperties"][0]["expression"]["left"]["arguments"][0]["next"] = json!(true);
    refused(invalid);
    let mut invalid = program();
    invalid["definitions"][0]["body"] = json!({"op":"read","slot":0,"next":true});
    refused(invalid);
    let mut invalid = program();
    invalid["assumptions"] = invalid["invariants"].take();
    invalid["invariants"] = json!([]);
    invalid["assumptions"][0]["expression"]["arguments"][0] =
        json!({"op":"read","slot":0,"next":false});
    refused(invalid);
}

#[test]
fn v2_definition_local_depth_keeps_the_existing_48_boundary() {
    let mut valid = program();
    let mut body = literal(true);
    for _ in 0..48 {
        body = json!({"op":"not","argument":body});
    }
    valid["definitions"][0]["body"] = body;
    accepted(valid.clone());
    let body = valid["definitions"][0]["body"].take();
    valid["definitions"][0]["body"] = json!({"op":"not","argument":body});
    refused(valid);
}

#[test]
fn v2_definition_ids_are_dense_and_follow_table_order() {
    let mut valid = program();
    valid["definitions"]
        .as_array_mut()
        .unwrap()
        .push(definition(1, literal(true)));
    accepted(valid.clone());
    for id in [0, 2] {
        let mut invalid = valid.clone();
        invalid["definitions"][1]["id"] = json!(id);
        refused(invalid);
    }
}

#[test]
fn v2_decoding_is_versioned_and_rejects_unknown_fields() {
    accepted(program());
    for (field, value) in [
        ("unknown", json!(true)),
        ("version", json!(3)),
        ("definitions", json!(null)),
    ] {
        let mut invalid = program();
        invalid[field] = value;
        assert!(serde_json::from_value::<SafetyProgram>(invalid).is_err());
    }
    let mut invalid = program();
    invalid.as_object_mut().unwrap().remove("definitions");
    assert!(serde_json::from_value::<SafetyProgram>(invalid).is_err());
    for (field, value) in [("unknown", json!(true)), ("bodyKind", json!("action"))] {
        let mut invalid = program();
        invalid["definitions"][0][field] = value;
        assert!(serde_json::from_value::<SafetyProgram>(invalid).is_err());
    }
    let mut invalid = program();
    invalid["invariants"][0]["expression"]["extra"] = json!(true);
    assert!(serde_json::from_value::<SafetyProgram>(invalid).is_err());
    let mut v1 = program();
    v1["version"] = json!(1);
    v1["invariants"][0]["expression"] = literal(true);
    assert!(serde_json::from_value::<SafetyProgram>(v1.clone()).is_err());
    v1.as_object_mut().unwrap().remove("definitions");
    accepted(v1.clone());
    for expression in [
        call(0, vec![literal(true)], vec![]),
        json!({"op":"parameter","slot":0}),
        json!({"op":"capture","slot":0}),
    ] {
        v1["invariants"][0]["expression"] = expression;
        assert!(serde_json::from_value::<SafetyProgram>(v1.clone()).is_err());
    }
}

#[test]
fn v2_nested_identity_has_exactly_four_units_of_work() {
    use crate::ir::{Definition, Expression, Scalar};
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;

    let definitions: Vec<Definition> =
        serde_json::from_value(json!([definition(0, json!({"op":"parameter","slot":0}))])).unwrap();
    let expression: Expression =
        serde_json::from_value(call(0, vec![call(0, vec![literal(true)], vec![])], vec![]))
            .unwrap();
    for maximum in [3, 4, 5] {
        let mut arena = Arena::new(maximum);
        arena.definitions = &definitions;
        let result = expression.eval(EvalContext::new(&[], &[]), &mut arena);
        if maximum == 3 {
            assert_eq!(result.unwrap_err().code, "budget");
            assert_eq!(arena.work, 3);
        } else {
            assert_eq!(result.unwrap(), Scalar::Bool(true));
            assert_eq!(arena.work, 4);
        }
    }
}

#[test]
fn v2_repeated_parameter_reads_reexecute_the_argument_without_a_value_cache() {
    use crate::ir::{Definition, Expression, Scalar};
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;

    let definitions: Vec<Definition> = serde_json::from_value(json!([
        definition(0, json!({"op":"parameter","slot":0})),
        definition(
            1,
            json!({"op":"and","left":{"op":"parameter","slot":0},"right":{"op":"parameter","slot":0}})
        )
    ]))
    .unwrap();
    let expression: Expression =
        serde_json::from_value(call(1, vec![call(0, vec![literal(true)], vec![])], vec![]))
            .unwrap();
    for maximum in [6, 7, 8] {
        let mut arena = Arena::new(maximum);
        arena.definitions = &definitions;
        let result = expression.eval(EvalContext::new(&[], &[]), &mut arena);
        if maximum == 6 {
            assert_eq!(result.unwrap_err().code, "budget");
            assert_eq!(arena.work, 6);
        } else {
            assert_eq!(result.unwrap(), Scalar::Bool(true));
            assert_eq!(arena.work, 7);
        }
    }
}

#[test]
fn v2_phase_summary_validation_does_not_expand_nested_actuals_exponentially() {
    let mut input = program();
    input["definitions"][0]["body"] = json!({"op":"enabled","branches":[{"name":"Nested","steps":[{"kind":"assign","slot":0,"value":{"op":"read","slot":0,"next":false}},{"kind":"guard","expression":{"op":"and","left":{"op":"parameter","slot":0},"right":{"op":"prime","argument":{"op":"parameter","slot":0}}}}]}]});
    let mut expression = literal(true);
    for _ in 0..40 {
        expression = call(0, vec![expression], vec![]);
    }
    input["invariants"][0]["expression"] = expression;
    let program: SafetyProgram = serde_json::from_value(input).unwrap();
    assert!(crate::safety_definitions::validate_program(&program).is_ok());
}

#[test]
fn v2_except_old_requires_explicit_capture_and_keeps_its_lexical_value() {
    let mut valid = program();
    valid["definitions"][0]["parameterCount"] = json!(0);
    valid["definitions"][0]["captureCount"] = json!(1);
    valid["definitions"][0]["body"] = json!({"op":"at","target":{"op":"update","target":{"op":"tuple","values":[{"op":"literal","value":{"type":"int","value":"9"}}]},"updates":[{"path":[{"op":"literal","value":{"type":"int","value":"1"}}],"value":{"op":"capture","slot":0}}]},"key":{"op":"literal","value":{"type":"int","value":"1"}}});
    valid["invariants"][0]["expression"] = json!({"op":"eq","left":{"op":"update","target":{"op":"tuple","values":[{"op":"literal","value":{"type":"int","value":"7"}}]},"updates":[{"path":[{"op":"literal","value":{"type":"int","value":"1"}}],"value":call(0,vec![],vec![json!({"op":"old"})])}]},"right":{"op":"tuple","values":[{"op":"literal","value":{"type":"int","value":"7"}}]}});
    accepted(valid.clone());
    valid["definitions"][0]["captureCount"] = json!(0);
    valid["definitions"][0]["body"] = json!({"op":"old"});
    valid["invariants"][0]["expression"]["left"]["updates"][0]["value"]["captures"] = json!([]);
    refused(valid);
}

const REQUIRED_STACK: usize = 8 * 1024 * 1024;

fn call_chain(
    count: usize,
    recursive: bool,
) -> (Vec<crate::ir::Definition>, crate::ir::Expression) {
    let definitions = (0..count)
        .map(|id| {
            let body = if id + 1 == count {
                literal(true)
            } else {
                call(id + 1, vec![], vec![])
            };
            let mut input = definition(id, body);
            input["parameterCount"] = json!(0);
            if recursive {
                input["recursive"] = json!(true);
            }
            serde_json::from_value(input).unwrap()
        })
        .collect();
    (
        definitions,
        serde_json::from_value(call(0, vec![], vec![])).unwrap(),
    )
}

fn evaluate_chain(
    count: usize,
    recursive: bool,
) -> (Result<crate::ir::Scalar, crate::CoreError>, u32) {
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;

    std::thread::Builder::new()
        .stack_size(REQUIRED_STACK)
        .spawn(move || {
            let (definitions, expression) = call_chain(count, recursive);
            let mut arena = Arena::new(2000);
            arena.definitions = &definitions;
            let result = expression.eval(EvalContext::new(&[], &[]), &mut arena);
            (result, arena.work)
        })
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn v2_dynamic_call_depth_is_checked_before_exhausting_the_native_stack() {
    use crate::ir::Scalar;

    for count in [255, 256, 257] {
        let (result, work) = evaluate_chain(count, false);
        if count == 255 {
            assert_eq!(result.unwrap(), Scalar::Bool(true));
            assert_eq!(work, 255);
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, "protocol");
            assert_eq!(error.message, "Safety evaluation nesting exceeds 256");
            assert_eq!(work, 256);
        }
    }
}

#[test]
fn v2_recursive_call_depth_keeps_the_former_limit() {
    use crate::ir::Scalar;

    for count in [127, 128, 129] {
        let (result, work) = evaluate_chain(count, true);
        if count == 127 {
            assert_eq!(result.unwrap(), Scalar::Bool(true));
            assert_eq!(work, 127);
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, "evaluation-budget");
            assert_eq!(
                error.message,
                "Recursive safety evaluation nesting exceeds 128"
            );
            assert_eq!(work, 128);
        }
    }
}

#[test]
fn v2_enabled_subscripts_keep_the_depth_of_live_branch_steps() {
    let build = |step_count| {
        let mut input = program();
        let definitions: Vec<_> = (0..3)
            .map(|id| {
                let mut steps = vec![json!({"kind":"assign","slot":0,"value":{"op":"read","slot":0,"next":false}})];
                for _ in 1..step_count {
                    steps.push(json!({"kind":"guard","expression":literal(true)}));
                }
                let subscript = if id == 2 {
                    literal(false)
                } else {
                    call(id + 1, vec![], vec![])
                };
                let body = json!({"op":"enabled","branches":[{"name":"Nested","steps":steps}],"subscript":subscript});
                let mut entry = definition(id, body);
                entry["parameterCount"] = json!(0);
                entry
            })
            .collect();
        input["definitions"] = json!(definitions);
        input["invariants"][0]["expression"] = json!({"op":"not","argument":call(0,vec![],vec![])});
        serde_json::from_value::<SafetyProgram>(input).unwrap()
    };
    let bounds = SafetyBounds {
        max_depth: 0,
        max_states: 1,
        max_work: 50000,
        max_temporal_nodes: None,
    };
    let control = build(10).check(bounds).unwrap();
    assert_eq!(control.status, "complete");
    assert_eq!(control.states, 1);
    assert_eq!(control.generated, 2);
    let error = std::thread::Builder::new()
        .stack_size(REQUIRED_STACK)
        .spawn(move || build(100).check(bounds).unwrap_err())
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(error.code, "protocol");
    assert_eq!(error.message, "Safety evaluation nesting exceeds 256");
}

#[test]
fn v2_enabled_subscripts_keep_the_recursive_limit() {
    let build = |steps: usize, recursive: bool| {
        let mut list =
            vec![json!({"kind":"assign","slot":0,"value":{"op":"read","slot":0,"next":false}})];
        for _ in 1..steps {
            list.push(json!({"kind":"guard","expression":literal(true)}));
        }
        let body = json!({"op":"enabled","branches":[{"name":"Nested","steps":list}],"subscript":literal(false)});
        let mut entry = definition(0, body);
        entry["parameterCount"] = json!(0);
        entry["recursive"] = json!(recursive);
        let mut input = program();
        input["definitions"] = json!([entry]);
        input["invariants"][0]["expression"] = json!({"op":"not","argument":call(0,vec![],vec![])});
        serde_json::from_value::<SafetyProgram>(input).unwrap()
    };
    let bounds = SafetyBounds {
        max_depth: 0,
        max_states: 1,
        max_work: 50000,
        max_temporal_nodes: None,
    };
    let run = |steps, recursive| {
        let program = build(steps, recursive);
        std::thread::Builder::new()
            .stack_size(REQUIRED_STACK)
            .spawn(move || program.check(bounds).unwrap())
            .unwrap()
            .join()
            .unwrap()
    };
    for (steps, recursive, status, reason) in [
        (123, true, "complete", None),
        (124, true, "bounded", Some("evaluation")),
        (124, false, "complete", None),
    ] {
        let result = run(steps, recursive);
        assert_eq!(result.status, status, "{steps} {recursive}");
        assert_eq!(result.reason.as_deref(), reason, "{steps} {recursive}");
    }
}

fn integer(value: i32) -> Value {
    json!({"op":"literal","value":{"type":"int","value":value.to_string()}})
}

fn range_descriptor(lower: Value, upper: Value) -> Value {
    json!({"op":"map-set","bounds":[{"slot":0,"domain":{"kind":"range","lower":lower,"upper":upper}}],"body":{"op":"bound","slot":0}})
}

#[test]
fn v2_shared_range_membership_avoids_enumeration_and_preserves_call_charges() {
    use crate::ir::{Expression, Scalar};
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;
    use std::sync::Arc;

    let mut input = program();
    input["definitions"][0]["parameterCount"] = json!(2);
    input["definitions"][0]["localScope"] = json!(1);
    input["definitions"][0]["body"] = range_descriptor(
        json!({"op":"parameter","slot":0}),
        json!({"op":"parameter","slot":1}),
    );
    let membership = json!({"op":"function-in","left":integer(0),"right":call(0,vec![integer(0),integer(i32::MAX)],vec![])});
    input["invariants"][0]["expression"] = json!({"op":"and","left":membership,"right":membership});
    let program: SafetyProgram = serde_json::from_value(input).unwrap();
    let expression: Expression =
        serde_json::from_value(json!({"op":"and","left":membership,"right":membership})).unwrap();
    let metadata = Arc::new(crate::safety_definitions::validate_program(&program).unwrap());
    for maximum in [5, 6, 7] {
        let mut arena = Arena::new(maximum);
        arena.definitions = program.definitions.as_deref().unwrap();
        arena.metadata = Some(Arc::clone(&metadata));
        let result = expression.eval(EvalContext::new(&[], &[]), &mut arena);
        if maximum == 5 {
            assert_eq!(result.unwrap_err().code, "budget");
            assert_eq!(arena.work, 5);
        } else {
            assert_eq!(result.unwrap(), Scalar::Bool(true));
            assert_eq!(arena.work, 6);
        }
    }
}

#[test]
fn v2_range_descriptor_keeps_integer_errors_and_empty_range_order() {
    for (value, lower, upper, expected) in [
        (integer(0), 0, i32::MAX, Some(true)),
        (integer(-1), 0, i32::MAX, Some(false)),
        (literal(true), 1, 0, Some(false)),
        (
            json!({"op":"data","value":{"type":"model-value","value":"A_m"}}),
            1,
            0,
            Some(false),
        ),
        (literal(true), 0, 1, None),
        (
            json!({"op":"data","value":{"type":"model-value","value":"m"}}),
            0,
            1,
            Some(false),
        ),
    ] {
        let mut input = program();
        let membership = json!({"op":"function-in","left":value,"right":range_descriptor(integer(lower),integer(upper))});
        input["definitions"] = json!([]);
        input["invariants"][0]["expression"] = if expected == Some(false) {
            json!({"op":"not","argument":membership})
        } else {
            membership
        };
        match expected {
            Some(_) => accepted(input),
            None => {
                let error = check(input).unwrap_err();
                assert_eq!(error.code, "protocol");
                assert_eq!(error.message, "Expected an integer");
            }
        }
    }
}

#[test]
fn v2_range_descriptor_does_not_change_value_enumeration_or_v1_membership() {
    let mut input = program();
    input["definitions"] = json!([]);
    input["invariants"][0]["expression"] = json!({"op":"function-in","left":integer(0),"right":range_descriptor(integer(0),integer(i32::MAX))});
    let bounds = SafetyBounds {
        max_depth: 64,
        max_states: 10000,
        max_work: 64,
        max_temporal_nodes: None,
    };
    let result = serde_json::from_value::<SafetyProgram>(input.clone())
        .unwrap()
        .check(bounds)
        .unwrap();
    assert_eq!(result.status, "complete");
    input["version"] = json!(1);
    input.as_object_mut().unwrap().remove("definitions");
    let result = serde_json::from_value::<SafetyProgram>(input.clone())
        .unwrap()
        .check(bounds)
        .unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("work"));
    input["version"] = json!(2);
    input["definitions"] = json!([]);
    input["invariants"][0]["expression"] = json!({"op":"eq","left":range_descriptor(integer(0),integer(i32::MAX)),"right":{"op":"set","values":[integer(0)]}});
    let result = serde_json::from_value::<SafetyProgram>(input)
        .unwrap()
        .check(bounds)
        .unwrap();
    assert_eq!(result.status, "bounded");
    assert_eq!(result.reason.as_deref(), Some("work"));
}

#[test]
fn v2_membership_branch_keeps_descriptor_purpose_and_short_circuiting() {
    let range = range_descriptor(integer(0), integer(i32::MAX));
    let invalid = json!({"op":"head","argument":{"op":"tuple","values":[]}});
    for condition in [true, false] {
        let mut input = program();
        input["definitions"][0]["localScope"] = json!(1);
        input["definitions"][0]["body"] = json!({"op":"function-in","left":integer(0),"right":{"op":"if","condition":literal(condition),"consequent":if condition {range.clone()} else {invalid.clone()},"alternative":if condition {invalid.clone()} else {range.clone()}}});
        accepted(input);
    }
    let mut input = program();
    input["definitions"][0]["localScope"] = json!(1);
    input["definitions"][0]["body"] = json!({"op":"function-in","left":integer(0),"right":{"op":"if","condition":literal(true),"consequent":invalid,"alternative":range}});
    assert_eq!(
        check(input).unwrap_err().message,
        "Head of an empty sequence"
    );
}

fn frame(arguments: Vec<Value>, captures: Vec<Value>, body: Value) -> Value {
    json!({"op":"frame","parameterCount":arguments.len(),"captureCount":captures.len(),"localScope":0,"arguments":arguments,"captures":captures,"body":body})
}

fn action_frame(arguments: Vec<Value>, branches: Value) -> Value {
    json!({"kind":"frame","parameterCount":arguments.len(),"captureCount":0,"localScope":0,"arguments":arguments,"captures":[],"branches":branches})
}

#[test]
fn anonymous_nested_identity_has_four_work_without_eager_argument_evaluation() {
    use crate::ir::{Expression, Scalar};
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;
    let parameter = json!({"op":"parameter","slot":0});
    let expression: Expression = serde_json::from_value(frame(
        vec![frame(vec![literal(true)], vec![], parameter.clone())],
        vec![],
        parameter,
    ))
    .unwrap();
    for maximum in [3, 4, 5] {
        let mut arena = Arena::new(maximum);
        let result = expression.eval(EvalContext::new(&[], &[]), &mut arena);
        if maximum == 3 {
            assert_eq!(result.unwrap_err().code, "budget");
            assert_eq!(arena.work, 3);
        } else {
            assert_eq!(result.unwrap(), Scalar::Bool(true));
            assert_eq!(arena.work, 4);
        }
    }
    let expression: Expression = serde_json::from_value(frame(
        vec![json!({"op":"head","argument":{"op":"tuple","values":[]}})],
        vec![],
        literal(true),
    ))
    .unwrap();
    let mut arena = Arena::new(1);
    assert_eq!(
        expression
            .eval(EvalContext::new(&[], &[]), &mut arena)
            .unwrap(),
        Scalar::Bool(true)
    );
    assert_eq!(arena.work, 1);
}

#[test]
fn anonymous_frames_validate_counts_and_every_unused_actual() {
    let mut valid = program();
    valid["invariants"][0]["expression"] = frame(vec![literal(true)], vec![], literal(true));
    accepted(valid.clone());
    for (field, value) in [
        ("parameterCount", json!(0)),
        ("captureCount", json!(1)),
        ("localScope", json!(41)),
    ] {
        let mut invalid = valid.clone();
        invalid["invariants"][0]["expression"][field] = value;
        refused(invalid);
    }
    for actual in [
        json!({"op":"bound","slot":0}),
        json!({"op":"old"}),
        json!({"op":"read","slot":0,"next":true}),
        call(99, vec![], vec![]),
    ] {
        let mut invalid = valid.clone();
        invalid["invariants"][0]["expression"]["arguments"][0] = actual;
        refused(invalid);
    }
    for field in [
        "arguments",
        "captures",
        "parameterCount",
        "captureCount",
        "localScope",
        "body",
    ] {
        let mut invalid = valid.clone();
        invalid["invariants"][0]["expression"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(serde_json::from_value::<SafetyProgram>(invalid).is_err());
    }
    valid["invariants"][0]["expression"]["unknown"] = json!(0);
    assert!(serde_json::from_value::<SafetyProgram>(valid).is_err());
}

#[test]
fn all_anonymous_frame_kinds_are_rejected_by_v1() {
    let mut v1 = program();
    v1["version"] = json!(1);
    v1.as_object_mut().unwrap().remove("definitions");
    v1["invariants"][0]["expression"] = literal(true);
    accepted(v1.clone());
    let mut expression = v1.clone();
    expression["invariants"][0]["expression"] = frame(vec![], vec![], literal(true));
    assert!(serde_json::from_value::<SafetyProgram>(expression).is_err());
    let mut action = v1.clone();
    action["next"][0]["steps"] = json!([action_frame(vec![], json!([]))]);
    assert!(serde_json::from_value::<SafetyProgram>(action).is_err());
    v1["properties"] = json!([{"name":"P","formula":{"kind":"frame","parameterCount":0,"captureCount":0,"localScope":0,"arguments":[],"captures":[],"formula":{"kind":"atom","expression":literal(true)}}}]);
    assert!(serde_json::from_value::<SafetyProgram>(v1).is_err());
}

#[test]
fn anonymous_bodies_cannot_implicitly_read_caller_locals_or_old() {
    let mut valid = program();
    valid["invariants"][0]["expression"] = json!({"op":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[literal(true)]}}],"body":frame(vec![json!({"op":"bound","slot":0})],vec![],json!({"op":"parameter","slot":0}))});
    accepted(valid.clone());
    valid["invariants"][0]["expression"]["body"]["body"] = json!({"op":"bound","slot":0});
    refused(valid);
    let mut valid = program();
    valid["invariants"][0]["expression"] = json!({"op":"eq","left":{"op":"update","target":{"op":"tuple","values":[integer(7)]},"updates":[{"path":[integer(1)],"value":frame(vec![],vec![json!({"op":"old"})],json!({"op":"capture","slot":0}))}]},"right":{"op":"tuple","values":[integer(7)]}});
    accepted(valid.clone());
    valid["invariants"][0]["expression"]["left"]["updates"][0]["value"]["body"] =
        json!({"op":"old"});
    refused(valid);
}

#[test]
fn anonymous_action_continuation_restores_caller_binder_and_keeps_next_assignments() {
    let mut valid = program();
    valid["next"][0]["steps"] = json!([
        {"kind":"bind","slot":0,"domain":{"kind":"set","values":[integer(1),integer(2)]}},
        action_frame(vec![json!({"op":"bound","slot":0})], json!([{"name":"Inner","steps":[{"kind":"assign","slot":0,"value":{"op":"parameter","slot":0}}]}])),
        {"kind":"guard","expression":{"op":"eq","left":{"op":"read","slot":0,"next":true},"right":{"op":"bound","slot":0}}}
    ]);
    let result = check(valid.clone()).unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 3);
    assert_eq!(result.generated, 7);
    valid["next"][0]["steps"][1]["localScope"] = json!(1);
    valid["next"][0]["steps"][1]["branches"][0]["steps"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({"kind":"bind","slot":0,"domain":{"kind":"set","values":[integer(9)]}}),
        );
    let result = check(valid).unwrap();
    assert_eq!(result.status, "complete");
    assert_eq!(result.states, 3);
    assert_eq!(result.generated, 7);
}

#[test]
fn anonymous_prime_uses_requested_next_state_and_enabled_uses_fresh_candidates() {
    let mut input = program();
    input["next"][0]["steps"] = json!([
        {"kind":"bind","slot":0,"domain":{"kind":"set","values":[integer(0)]}},
        action_frame(vec![json!({"op":"add","left":{"op":"read","slot":0,"next":false},"right":{"op":"bound","slot":0}})], json!([{"name":"Step","steps":[{"kind":"assign","slot":0,"value":integer(1)},{"kind":"guard","expression":{"op":"eq","left":{"op":"prime","argument":{"op":"parameter","slot":0}},"right":integer(1)}}]}]))
    ]);
    input["invariants"][0]["expression"] =
        json!({"op":"eq","left":{"op":"read","slot":0,"next":false},"right":integer(0)});
    let result = check(input).unwrap();
    assert_eq!(result.status, "invariant-violation");
    assert_eq!(result.violation.unwrap().trace.len(), 2);
    let mut input = program();
    let enabled = json!({"op":"enabled","branches":[{"name":"Enabled","steps":[action_frame(vec![json!({"op":"bound","slot":0})],json!([{"name":"Step","steps":[{"kind":"assign","slot":0,"value":{"op":"parameter","slot":0}},{"kind":"guard","expression":{"op":"eq","left":{"op":"read","slot":0,"next":true},"right":integer(1)}}]}]))]}]});
    input["invariants"][0]["expression"] = json!({"op":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(1),integer(2)]}}],"body":{"op":"eq","left":enabled,"right":{"op":"eq","left":{"op":"bound","slot":0},"right":integer(1)}}});
    accepted(input);
}

#[test]
fn anonymous_frame_bodies_cannot_hide_cycles_or_bypass_aggregate_limits() {
    let mut input = program();
    input["definitions"][0]["body"] = frame(vec![], vec![], call(0, vec![literal(true)], vec![]));
    refused(input);
    let mut input = program();
    input["next"][0]["steps"] = json!([action_frame(
        vec![],
        json!([{"name":"Inner","steps":vec![json!({"kind":"guard","expression":literal(true)});256]}])
    )]);
    refused(input);
    let mut input = program();
    let branches = json!([{"name":"A","steps":[]},{"name":"B","steps":[]}]);
    input["next"][0]["steps"] = json!(vec![action_frame(vec![], branches); 13]);
    refused(input);
}

#[test]
fn anonymous_temporal_frames_own_outer_binders_across_state_evaluations() {
    let mut input = program();
    input["properties"] = json!([{"name":"P","formula":{"kind":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(0)]}}],"formula":{"kind":"frame","parameterCount":1,"captureCount":0,"localScope":1,"arguments":[{"op":"bound","slot":0}],"captures":[],"formula":{"kind":"always","formula":{"kind":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(9)]}}],"formula":{"kind":"atom","expression":{"op":"eq","left":{"op":"parameter","slot":0},"right":{"op":"read","slot":0,"next":false}}}}}}}}]);
    accepted(input.clone());
    input["properties"][0]["formula"]["bounds"][0]["domain"]["values"] =
        json!([integer(0), integer(1)]);
    let result = check(input).unwrap();
    assert_eq!(result.status, "liveness-violation");
    assert_eq!(result.violation.unwrap().name, "P");
}

#[test]
fn anonymous_temporal_frame_actuals_follow_prime_at_each_state_edge() {
    let mut input = program();
    input["next"][0]["steps"][0]["value"] = integer(1);
    input["properties"] = json!([{"name":"P","formula":{"kind":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(0)]}}],"formula":{"kind":"frame","parameterCount":1,"captureCount":0,"localScope":0,"arguments":[{"op":"add","left":{"op":"bound","slot":0},"right":{"op":"read","slot":0,"next":false}}],"captures":[],"formula":{"kind":"always","formula":{"kind":"atom","expression":{"op":"eq","left":{"op":"prime","argument":{"op":"parameter","slot":0}},"right":integer(1)}}}}}}]);
    let result = check(input).unwrap();
    assert_eq!(result.status, "liveness-violation");
    assert_eq!(result.violation.unwrap().name, "P");
}

#[test]
fn public_program_deep_action_frames_are_checked_before_recursive_analysis() {
    use crate::ir::{Branch, Expression, Scalar, Step};
    let mut input: SafetyProgram = serde_json::from_value(program()).unwrap();
    let mut branches = vec![Branch {
        name: "Leaf".into(),
        steps: vec![Step::Guard {
            expression: Expression::Literal {
                value: Scalar::Bool(true),
            },
        }],
    }];
    for _ in 0..5000 {
        branches = vec![Branch {
            name: "Nested".into(),
            steps: vec![Step::Frame {
                arguments: vec![],
                captures: vec![],
                parameter_count: 0,
                capture_count: 0,
                local_scope: 0,
                branches,
            }],
        }];
    }
    input.definitions.as_mut().unwrap()[0].body = Expression::Enabled {
        branches,
        subscript: None,
    }
    .into();
    let error = crate::safety_definitions::validate_program(&input)
        .err()
        .unwrap();
    assert_eq!(error.code, "protocol");
    assert_eq!(error.message, "Safety expression nesting exceeds 48");
}

macro_rules! analysis_parity_wire_range {
    ($name:ident, $target:ty, $fixture:expr, $field:literal) => {
        #[test]
        fn $name() {
            let mut value = $fixture;
            value[$field] = json!(u32::MAX);
            assert!(serde_json::from_value::<$target>(value.clone()).is_ok());
            value[$field] = json!(u64::from(u32::MAX) + 1);
            assert!(
                serde_json::from_value::<$target>(value).is_err(),
                "wire metadata above uint32 must be rejected before usize conversion"
            );
        }
    };
}

analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_frame_parametercount,
    crate::ir::Expression,
    json!({"op":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"body":literal(true)}),
    "parameterCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_frame_capturecount,
    crate::ir::Expression,
    json!({"op":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"body":literal(true)}),
    "captureCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_frame_localscope,
    crate::ir::Expression,
    json!({"op":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"body":literal(true)}),
    "localScope"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_call_definition,
    crate::ir::Expression,
    call(0, vec![], vec![]),
    "definition"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_parameter_slot,
    crate::ir::Expression,
    json!({"op":"parameter","slot":0}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_capture_slot,
    crate::ir::Expression,
    json!({"op":"capture","slot":0}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_bound_slot,
    crate::ir::Expression,
    json!({"op":"bound","slot":0}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_read_slot,
    crate::ir::Expression,
    json!({"op":"read","slot":0,"next":false}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_expression_fold_accumulator,
    crate::ir::Expression,
    json!({"op":"fold","accumulator":0,"initial":integer(0),"collection":{"op":"tuple","values":[]},"order":"sequence-left","body":integer(0)}),
    "accumulator"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_step_frame_parametercount,
    crate::ir::Step,
    json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"branches":[]}),
    "parameterCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_step_frame_capturecount,
    crate::ir::Step,
    json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"branches":[]}),
    "captureCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_step_frame_localscope,
    crate::ir::Step,
    json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"branches":[]}),
    "localScope"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_step_bind_slot,
    crate::ir::Step,
    json!({"kind":"bind","slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_step_bind_tuplesize,
    crate::ir::Step,
    json!({"kind":"bind","slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}}),
    "tupleSize"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_step_assign_slot,
    crate::ir::Step,
    json!({"kind":"assign","slot":0,"value":integer(0)}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_step_choose_slot,
    crate::ir::Step,
    json!({"kind":"choose","slot":0,"domain":{"kind":"set","values":[integer(0)]}}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_binding_slot,
    crate::ir::Binding,
    json!({"slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}}),
    "slot"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_binding_tuplesize,
    crate::ir::Binding,
    json!({"slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}}),
    "tupleSize"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_definition_id,
    crate::ir::Definition,
    definition(0, literal(true)),
    "id"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_definition_parametercount,
    crate::ir::Definition,
    definition(0, literal(true)),
    "parameterCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_definition_capturecount,
    crate::ir::Definition,
    definition(0, literal(true)),
    "captureCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_definition_localscope,
    crate::ir::Definition,
    definition(0, literal(true)),
    "localScope"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_temporal_frame_parametercount,
    crate::ir::TemporalFormula,
    json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"formula":{"kind":"atom","expression":literal(true)}}),
    "parameterCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_temporal_frame_capturecount,
    crate::ir::TemporalFormula,
    json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"formula":{"kind":"atom","expression":literal(true)}}),
    "captureCount"
);
analysis_parity_wire_range!(
    analysis_parity_wire_range_temporal_frame_localscope,
    crate::ir::TemporalFormula,
    json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"formula":{"kind":"atom","expression":literal(true)}}),
    "localScope"
);

#[cfg(target_pointer_width = "64")]
macro_rules! analysis_parity_wire_direct {
    ($name:ident, $mutate:expr, $max_accepted:expr) => {
        #[test]
        fn $name() {
            let mut value = program();
            value["invariants"][0]["expression"] = literal(true);
            value["definitions"][0]["parameterCount"] = json!(0);
            value["definitions"][0]["body"] = literal(true);
            let mut input: SafetyProgram = serde_json::from_value(value).unwrap();
            let set = $mutate;
            set(&mut input, u32::MAX as usize);
            let at_max = crate::safety_definitions::validate_program(&input);
            assert_eq!(at_max.is_ok(), $max_accepted);
            set(&mut input, u32::MAX as usize + 1);
            let error = crate::safety_definitions::validate_program(&input).err();
            assert!(error.is_some(), "direct IR accepted metadata above uint32");
            assert_eq!(error.unwrap().code, "protocol");
        }
    };
}

#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_frame_parametercount,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression = serde_json::from_value(json!({"op":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"body":literal(true)})).unwrap();
        if let crate::ir::Expression::Frame {
            parameter_count, ..
        } = &mut node
        {
            *parameter_count = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_frame_capturecount,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression = serde_json::from_value(json!({"op":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"body":literal(true)})).unwrap();
        if let crate::ir::Expression::Frame { capture_count, .. } = &mut node {
            *capture_count = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_frame_localscope,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression = serde_json::from_value(json!({"op":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"body":literal(true)})).unwrap();
        if let crate::ir::Expression::Frame { local_scope, .. } = &mut node {
            *local_scope = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_call_definition,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression =
            serde_json::from_value(call(0, vec![], vec![])).unwrap();
        if let crate::ir::Expression::Call { definition, .. } = &mut node {
            *definition = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_parameter_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression =
            serde_json::from_value(json!({"op":"parameter","slot":0})).unwrap();
        if let crate::ir::Expression::Parameter { slot, .. } = &mut node {
            *slot = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_capture_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression =
            serde_json::from_value(json!({"op":"capture","slot":0})).unwrap();
        if let crate::ir::Expression::Capture { slot, .. } = &mut node {
            *slot = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_bound_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression =
            serde_json::from_value(json!({"op":"bound","slot":0})).unwrap();
        if let crate::ir::Expression::Bound { slot, .. } = &mut node {
            *slot = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_read_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression =
            serde_json::from_value(json!({"op":"read","slot":0,"next":false})).unwrap();
        if let crate::ir::Expression::Read { slot, .. } = &mut node {
            *slot = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_expression_fold_accumulator,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Expression = serde_json::from_value(json!({"op":"fold","accumulator":0,"initial":integer(0),"collection":{"op":"tuple","values":[]},"order":"sequence-left","body":integer(0)})).unwrap();
        if let crate::ir::Expression::Fold { accumulator, .. } = &mut node {
            *accumulator = count;
        } else {
            unreachable!();
        }
        input.definitions.as_mut().unwrap()[0].body = node.into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_step_frame_parametercount,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Step = serde_json::from_value(json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"branches":[]})).unwrap();
        if let crate::ir::Step::Frame {
            parameter_count, ..
        } = &mut node
        {
            *parameter_count = count;
        } else {
            unreachable!();
        }
        input.next[0].steps = vec![node];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_step_frame_capturecount,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Step = serde_json::from_value(json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"branches":[]})).unwrap();
        if let crate::ir::Step::Frame { capture_count, .. } = &mut node {
            *capture_count = count;
        } else {
            unreachable!();
        }
        input.next[0].steps = vec![node];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_step_frame_localscope,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Step = serde_json::from_value(json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"branches":[]})).unwrap();
        if let crate::ir::Step::Frame { local_scope, .. } = &mut node {
            *local_scope = count;
        } else {
            unreachable!();
        }
        input.next[0].steps = vec![node];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_step_bind_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Step = serde_json::from_value(json!({"kind":"bind","slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}})).unwrap();
        if let crate::ir::Step::Bind { slot, .. } = &mut node {
            *slot = count;
        } else {
            unreachable!();
        }
        input.next[0].steps = vec![node];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_step_bind_tuplesize,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Step = serde_json::from_value(json!({"kind":"bind","slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}})).unwrap();
        if let crate::ir::Step::Bind { tuple_size, .. } = &mut node {
            *tuple_size = Some(count);
        } else {
            unreachable!();
        }
        input.next[0].steps = vec![node];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_step_assign_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Step =
            serde_json::from_value(json!({"kind":"assign","slot":0,"value":integer(0)})).unwrap();
        if let crate::ir::Step::Assign { slot, .. } = &mut node {
            *slot = count;
        } else {
            unreachable!();
        }
        input.next[0].steps = vec![node];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_step_choose_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::Step = serde_json::from_value(
            json!({"kind":"choose","slot":0,"domain":{"kind":"set","values":[integer(0)]}}),
        )
        .unwrap();
        if let crate::ir::Step::Choose { slot, .. } = &mut node {
            *slot = count;
        } else {
            unreachable!();
        }
        input.next[0].steps = vec![node];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_binding_slot,
    |input: &mut SafetyProgram, count: usize| {
        let mut binding: crate::ir::Binding = serde_json::from_value(
            json!({"slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}}),
        )
        .unwrap();
        binding.slot = count;
        input.definitions.as_mut().unwrap()[0].local_scope = 1;
        input.definitions.as_mut().unwrap()[0].body = crate::ir::Expression::Forall {
            bounds: vec![binding],
            body: Box::new(crate::ir::Expression::Literal {
                value: crate::ir::Scalar::Bool(true),
            }),
        }
        .into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_binding_tuplesize,
    |input: &mut SafetyProgram, count: usize| {
        let mut binding: crate::ir::Binding = serde_json::from_value(
            json!({"slot":0,"tupleSize":1,"domain":{"kind":"set","values":[integer(0)]}}),
        )
        .unwrap();
        binding.tuple_size = Some(count);
        input.definitions.as_mut().unwrap()[0].local_scope = 1;
        input.definitions.as_mut().unwrap()[0].body = crate::ir::Expression::Forall {
            bounds: vec![binding],
            body: Box::new(crate::ir::Expression::Literal {
                value: crate::ir::Scalar::Bool(true),
            }),
        }
        .into();
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_definition_id,
    |input: &mut SafetyProgram, count: usize| {
        input.definitions.as_mut().unwrap()[0].id = count;
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_definition_parametercount,
    |input: &mut SafetyProgram, count: usize| {
        input.definitions.as_mut().unwrap()[0].parameter_count = count;
    },
    true
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_definition_capturecount,
    |input: &mut SafetyProgram, count: usize| {
        input.definitions.as_mut().unwrap()[0].capture_count = count;
    },
    true
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_definition_localscope,
    |input: &mut SafetyProgram, count: usize| {
        input.definitions.as_mut().unwrap()[0].local_scope = count;
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_temporal_frame_parametercount,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::TemporalFormula = serde_json::from_value(json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"formula":{"kind":"atom","expression":literal(true)}})).unwrap();
        if let crate::ir::TemporalFormula::Frame {
            parameter_count, ..
        } = &mut node
        {
            *parameter_count = count;
        } else {
            unreachable!();
        }
        input.properties = vec![crate::ir::TemporalProperty {
            name: "P".into(),
            formula: node,
        }];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_temporal_frame_capturecount,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::TemporalFormula = serde_json::from_value(json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"formula":{"kind":"atom","expression":literal(true)}})).unwrap();
        if let crate::ir::TemporalFormula::Frame { capture_count, .. } = &mut node {
            *capture_count = count;
        } else {
            unreachable!();
        }
        input.properties = vec![crate::ir::TemporalProperty {
            name: "P".into(),
            formula: node,
        }];
    },
    false
);
#[cfg(target_pointer_width = "64")]
analysis_parity_wire_direct!(
    analysis_parity_wire_direct_temporal_frame_localscope,
    |input: &mut SafetyProgram, count: usize| {
        let mut node: crate::ir::TemporalFormula = serde_json::from_value(json!({"kind":"frame","arguments":[],"captures":[],"parameterCount":0,"captureCount":0,"localScope":0,"formula":{"kind":"atom","expression":literal(true)}})).unwrap();
        if let crate::ir::TemporalFormula::Frame { local_scope, .. } = &mut node {
            *local_scope = count;
        } else {
            unreachable!();
        }
        input.properties = vec![crate::ir::TemporalProperty {
            name: "P".into(),
            formula: node,
        }];
    },
    false
);

#[test]
fn analysis_parity_wire_arity_and_slot_controls_remain_distinct_from_range_checks() {
    for slot in [0, 1] {
        let mut value = program();
        value["definitions"][0]["body"] = json!({"op":"parameter","slot":slot});
        if slot == 0 {
            accepted(value);
        } else {
            refused(value);
        }
    }
    for count in [0, 1] {
        let mut value = program();
        value["definitions"][0]["parameterCount"] = json!(count);
        if count == 1 {
            accepted(value);
        } else {
            refused(value);
        }
    }
    for scope in [40, 41] {
        let mut value = program();
        value["definitions"][0]["localScope"] = json!(scope);
        if scope == 40 {
            accepted(value);
        } else {
            refused(value);
        }
    }
}

#[test]
fn bind_run_planning_does_not_recurse_before_the_nesting_limit_is_checked() {
    use crate::ir::{Branch, Domain, Expression, Scalar, Step};
    let outcome = std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let mut input: SafetyProgram = serde_json::from_value(json!({
                "format":"modelt.safety-program",
                "version":1,
                "name":"Deep",
                "variables":["x"],
                "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":{"op":"literal","value":{"type":"int","value":"0"}}}]}],
                "next":[],
                "invariants":[],
                "assumptions":[],
                "checkDeadlock":false
            }))
            .unwrap();
            let mut guard = Expression::Literal {
                value: Scalar::Bool(true),
            };
            for _ in 0..100_000 {
                guard = Expression::Not {
                    argument: Box::new(guard),
                };
            }
            input.next = vec![Branch {
                name: "Next".into(),
                steps: vec![
                    Step::Bind {
                        slot: 0,
                        domain: Domain::Set {
                            values: vec![Expression::Literal {
                                value: Scalar::Bool(true),
                            }],
                        },
                        tuple_size: None,
                    },
                    Step::Guard { expression: guard },
                ],
            }];
            let error = input
                .check(SafetyBounds {
                    max_depth: 64,
                    max_states: 10000,
                    max_work: 1000000,
                    max_temporal_nodes: None,
                })
                .unwrap_err();
            (error.code, error.message)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(outcome.0, "protocol");
    assert_eq!(outcome.1, "Safety expression nesting exceeds 48");
}

#[test]
fn bind_run_planning_stops_at_the_first_enabled_before_the_branch_limit_is_checked() {
    use crate::ir::{Branch, Domain, Expression, Step};
    let mut input: SafetyProgram = serde_json::from_value(json!({
        "format":"modelt.safety-program",
        "version":1,
        "name":"Pairs",
        "variables":["x"],
        "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":{"op":"literal","value":{"type":"int","value":"0"}}}]}],
        "next":[],
        "invariants":[],
        "assumptions":[],
        "checkDeadlock":false
    }))
    .unwrap();
    let steps = (0..20_000)
        .flat_map(|_| {
            [
                Step::Bind {
                    slot: 0,
                    domain: Domain::Set { values: vec![] },
                    tuple_size: None,
                },
                Step::Guard {
                    expression: Expression::Enabled {
                        branches: vec![],
                        subscript: None,
                    },
                },
            ]
        })
        .collect();
    input.next = vec![Branch {
        name: "Next".into(),
        steps,
    }];
    let started = std::time::Instant::now();
    let error = input
        .check(SafetyBounds {
            max_depth: 64,
            max_states: 10000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .unwrap_err();
    assert_eq!(error.message, "Safety branch exceeds 256 steps");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

fn pairs_before_the_branch_limit(
    tail: crate::ir::Step,
) -> (crate::SafetyProgram, std::time::Instant) {
    use crate::ir::{Branch, Domain, Expression, Scalar, Step};
    let mut input: SafetyProgram = serde_json::from_value(json!({
        "format":"modelt.safety-program",
        "version":1,
        "name":"Pairs",
        "variables":["x"],
        "initial":[{"name":"Init","steps":[{"kind":"assign","slot":0,"value":{"op":"literal","value":{"type":"int","value":"0"}}}]}],
        "next":[],
        "invariants":[],
        "assumptions":[],
        "checkDeadlock":false
    }))
    .unwrap();
    let mut steps: Vec<Step> = (0..512)
        .flat_map(|_| {
            [
                Step::Bind {
                    slot: 0,
                    domain: Domain::Set { values: vec![] },
                    tuple_size: None,
                },
                Step::Guard {
                    expression: Expression::Literal {
                        value: Scalar::Bool(true),
                    },
                },
            ]
        })
        .collect();
    steps.push(tail);
    input.next = vec![Branch {
        name: "Next".into(),
        steps,
    }];
    (input, std::time::Instant::now())
}

fn refused_within(
    input: &crate::SafetyProgram,
    started: std::time::Instant,
    limit: std::time::Duration,
) {
    let error = input
        .check(SafetyBounds {
            max_depth: 64,
            max_states: 10000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .unwrap_err();
    assert_eq!(error.message, "Safety branch exceeds 256 steps");
    assert!(started.elapsed() < limit, "{:?}", started.elapsed());
}

fn refused_quickly(input: &crate::SafetyProgram, started: std::time::Instant) {
    refused_within(input, started, std::time::Duration::from_millis(600));
}

#[test]
fn bind_run_planning_checks_an_expression_in_source_order_before_its_later_siblings() {
    use crate::ir::{Expression, Scalar, Step};
    let truth = || Expression::Literal {
        value: Scalar::Bool(true),
    };
    let (input, _) = pairs_before_the_branch_limit(Step::Guard {
        expression: Expression::And {
            left: Box::new(Expression::Enabled {
                branches: vec![],
                subscript: None,
            }),
            right: Box::new(Expression::Set {
                values: vec![truth(); 100_000],
            }),
        },
    });
    refused_quickly(&input, std::time::Instant::now());
}

#[test]
fn bind_run_planning_checks_frame_arguments_before_any_frame_branch() {
    use crate::ir::{Branch, Expression, Step};
    let enabled = || Expression::Enabled {
        branches: vec![],
        subscript: None,
    };
    let empty = || Branch {
        name: "Empty".into(),
        steps: vec![],
    };
    let frame = |arguments: Vec<Expression>, first: Vec<Step>| Step::Frame {
        arguments,
        captures: vec![],
        parameter_count: 0,
        capture_count: 0,
        local_scope: 0,
        branches: std::iter::once(Branch {
            name: "First".into(),
            steps: first,
        })
        .chain(std::iter::repeat_with(empty).take(500_000))
        .collect(),
    };
    for tail in [
        frame(vec![enabled()], vec![]),
        frame(
            vec![],
            vec![Step::Guard {
                expression: enabled(),
            }],
        ),
    ] {
        let (input, started) = pairs_before_the_branch_limit(tail);
        refused_quickly(&input, started);
    }
}

#[test]
fn bind_run_planning_checks_argument_and_capture_lists_one_element_at_a_time() {
    use crate::ir::{Expression, Scalar, Step};
    let enabled = || Expression::Enabled {
        branches: vec![],
        subscript: None,
    };
    let truth = || Expression::Literal {
        value: Scalar::Bool(true),
    };
    let long = || {
        std::iter::once(enabled())
            .chain(std::iter::repeat_with(truth).take(125_000))
            .collect::<Vec<_>>()
    };
    let frame = |arguments: Vec<Expression>, captures: Vec<Expression>| Step::Frame {
        arguments,
        captures,
        parameter_count: 0,
        capture_count: 0,
        local_scope: 0,
        branches: vec![],
    };
    let call = |arguments: Vec<Expression>, captures: Vec<Expression>| Step::Call {
        definition: 0,
        arguments,
        captures,
    };
    for tail in [
        call(long(), vec![]),
        call(vec![truth()], long()),
        frame(long(), vec![]),
        frame(vec![truth()], long()),
    ] {
        let (input, started) = pairs_before_the_branch_limit(tail);
        refused_within(&input, started, std::time::Duration::from_millis(300));
    }
}

#[test]
fn bind_run_planning_checks_a_long_child_list_one_element_at_a_time() {
    use crate::ir::{Domain, Expression, Scalar, Step};
    let values = || {
        std::iter::once(Expression::Enabled {
            branches: vec![],
            subscript: None,
        })
        .chain(
            std::iter::repeat_with(|| Expression::Literal {
                value: Scalar::Bool(true),
            })
            .take(200_000),
        )
        .collect::<Vec<_>>()
    };
    for tail in [
        Step::Guard {
            expression: Expression::Set { values: values() },
        },
        Step::Choose {
            slot: 0,
            domain: Domain::Set { values: values() },
        },
    ] {
        let (input, started) = pairs_before_the_branch_limit(tail);
        refused_within(&input, started, std::time::Duration::from_millis(850));
    }
}

fn applied_function_program(passes: usize, body_calls: usize) -> SafetyProgram {
    let function = json!({"op":"function","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(1)]}}],"body":call(passes + 1, vec![], vec![])});
    let mut definitions = vec![{
        let mut root = definition(0, call(1, vec![function], vec![]));
        root["parameterCount"] = json!(0);
        root["localScope"] = json!(1);
        root
    }];
    for id in 1..=passes {
        let body = if id == passes {
            json!({"op":"at","target":{"op":"parameter","slot":0},"key":integer(1)})
        } else {
            call(id + 1, vec![json!({"op":"parameter","slot":0})], vec![])
        };
        definitions.push(definition(id, body));
    }
    for index in 0..body_calls {
        let id = passes + 1 + index;
        let body = if index + 1 == body_calls {
            literal(true)
        } else {
            call(id + 1, vec![], vec![])
        };
        let mut entry = definition(id, body);
        entry["parameterCount"] = json!(0);
        definitions.push(entry);
    }
    let mut input = program();
    input["definitions"] = json!(definitions);
    input["invariants"][0]["expression"] = call(0, vec![], vec![]);
    serde_json::from_value(input).unwrap()
}

#[test]
fn v2_applied_lazy_function_body_counts_the_depth_of_its_application() {
    let bounds = SafetyBounds {
        max_depth: 0,
        max_states: 1,
        max_work: 50000,
        max_temporal_nodes: None,
    };
    let run = |passes, body_calls| {
        let program = applied_function_program(passes, body_calls);
        std::thread::Builder::new()
            .stack_size(REQUIRED_STACK)
            .spawn(move || program.check(bounds))
            .unwrap()
            .join()
            .unwrap()
    };
    let mut observed = vec![];
    for (passes, body_calls, completes) in [
        (1, 130, true),
        (70, 10, true),
        (70, 130, true),
        (130, 10, true),
        (122, 127, true),
        (122, 128, false),
        (130, 120, false),
    ] {
        let outcome = run(passes, body_calls);
        let matches = match (&outcome, completes) {
            (Ok(result), true) => result.status == "complete",
            (Err(error), false) => {
                error.code == "protocol" && error.message == "Safety evaluation nesting exceeds 256"
            }
            _ => false,
        };
        if !matches {
            observed.push(format!(
                "{passes} {body_calls}: expected {}, observed {}",
                if completes {
                    "complete"
                } else {
                    "a depth refusal"
                },
                match outcome {
                    Ok(result) => result.status,
                    Err(error) => format!("{}: {}", error.code, error.message),
                }
            ));
        }
    }
    assert!(observed.is_empty(), "\n{}", observed.join("\n"));
}

fn applied_recursive_function_program(passes: usize, body_calls: usize) -> SafetyProgram {
    let function = json!({"op":"function","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(1)]}}],"body":call(passes + 1, vec![], vec![])});
    let mut definitions = vec![{
        let mut root = definition(0, call(1, vec![function], vec![]));
        root["parameterCount"] = json!(0);
        root["localScope"] = json!(1);
        root
    }];
    for id in 1..=passes {
        let body = if id == passes {
            json!({"op":"at","target":{"op":"parameter","slot":0},"key":integer(1)})
        } else {
            call(id + 1, vec![json!({"op":"parameter","slot":0})], vec![])
        };
        definitions.push(definition(id, body));
    }
    for mut entry in zero_argument_chain(passes + 1, body_calls) {
        entry["recursive"] = json!(true);
        definitions.push(entry);
    }
    let mut input = program();
    input["definitions"] = json!(definitions);
    input["invariants"][0]["expression"] = call(0, vec![], vec![]);
    serde_json::from_value(input).unwrap()
}

#[test]
fn v2_applied_lazy_function_keeps_the_forwarded_depth_of_its_snapshot() {
    let bounds = SafetyBounds {
        max_depth: 0,
        max_states: 1,
        max_work: 50000,
        max_temporal_nodes: None,
    };
    let run = |passes, body_calls| {
        let program = applied_recursive_function_program(passes, body_calls);
        std::thread::Builder::new()
            .stack_size(REQUIRED_STACK)
            .spawn(move || program.check(bounds))
            .unwrap()
            .join()
            .unwrap()
            .unwrap()
    };
    let mut observed = vec![];
    for (passes, expected_work) in [
        (70, 143),
        (63, 129),
        (62, 127),
        (61, 125),
        (60, 125),
        (57, 125),
    ] {
        let result = run(passes, 10);
        if (
            result.status.as_str(),
            result.reason.as_deref(),
            result.work,
        ) != ("bounded", Some("evaluation"), expected_work)
        {
            observed.push(format!(
                "{passes}: expected bounded/evaluation work={expected_work}, observed {}/{:?} work={}",
                result.status, result.reason, result.work
            ));
        }
    }
    let result = run(56, 10);
    if (result.status.as_str(), result.reason.as_deref()) != ("complete", None) {
        observed.push(format!(
            "56: expected complete/None, observed {}/{:?} work={}",
            result.status, result.reason, result.work
        ));
    }
    assert!(observed.is_empty(), "\n{}", observed.join("\n"));
}

const DEPTH_REFUSAL: &str = "error protocol: Safety evaluation nesting exceeds 256";
const RECURSIVE_REFUSAL: &str =
    "error evaluation-budget: Recursive safety evaluation nesting exceeds 128";

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Parameters,
    Captures,
    Alternating,
    Frames,
}

fn receives_capture(shape: Shape, index: usize) -> bool {
    match shape {
        Shape::Captures => true,
        Shape::Alternating => index % 2 == 1,
        Shape::Parameters | Shape::Frames => false,
    }
}

fn lexical(capture: bool) -> Value {
    json!({"op":if capture { "capture" } else { "parameter" },"slot":0})
}

fn hand_over(shape: Shape, index: usize, definition: usize, actual: Value) -> Value {
    if receives_capture(shape, index) {
        call(definition, vec![], vec![actual])
    } else {
        call(definition, vec![actual], vec![])
    }
}

fn forwarding_chain(shape: Shape, first: usize, size: usize) -> Vec<Value> {
    (0..size)
        .map(|index| {
            let own = lexical(receives_capture(shape, index));
            let body = if index + 1 == size {
                own
            } else if shape == Shape::Frames {
                let next = hand_over(shape, index + 1, first + index + 1, lexical(false));
                frame(vec![own], vec![], next)
            } else {
                hand_over(shape, index + 1, first + index + 1, own)
            };
            let mut entry = definition(first + index, body);
            if receives_capture(shape, index) {
                entry["parameterCount"] = json!(0);
                entry["captureCount"] = json!(1);
            }
            entry
        })
        .collect()
}

fn forwarded_root(shape: Shape, first: usize, actual: Value) -> Value {
    hand_over(shape, 0, first, actual)
}

fn evaluated(
    definitions: Vec<Value>,
    expression: Value,
    maximum: u32,
) -> (Result<crate::ir::Scalar, crate::CoreError>, u32) {
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;

    std::thread::Builder::new()
        .stack_size(REQUIRED_STACK)
        .spawn(move || {
            let definitions: Vec<crate::ir::Definition> =
                serde_json::from_value(Value::Array(definitions)).unwrap();
            let expression: crate::ir::Expression = serde_json::from_value(expression).unwrap();
            let mut arena = Arena::new(maximum);
            arena.definitions = &definitions;
            let result = expression.eval(
                EvalContext::new(&[crate::ir::Scalar::Int(0)], &[]),
                &mut arena,
            );
            (result, arena.work)
        })
        .unwrap()
        .join()
        .unwrap()
}

fn summarized(outcome: (Result<crate::ir::Scalar, crate::CoreError>, u32)) -> String {
    let (result, work) = outcome;
    match result {
        Ok(crate::ir::Scalar::Bool(value)) => format!("{value} work={work}"),
        Ok(_) => format!("non-boolean work={work}"),
        Err(error) => format!("error {}: {} work={work}", error.code, error.message),
    }
}

fn disagreements(rows: Vec<(String, String, String)>) -> Vec<String> {
    rows.into_iter()
        .filter(|(_, expected, actual)| expected != actual)
        .map(|(label, expected, actual)| {
            format!("{label}: expected `{expected}`, observed `{actual}`")
        })
        .collect()
}

fn assert_agreement(rows: Vec<(String, String, String)>) {
    let found = disagreements(rows);
    assert!(found.is_empty(), "\n{}", found.join("\n"));
}

fn without_work(text: String) -> String {
    match text.rsplit_once(" work=") {
        Some((head, _)) => head.to_string(),
        None => text,
    }
}

fn zero_argument_chain(first: usize, count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| {
            let body = if index + 1 == count {
                literal(true)
            } else {
                call(first + index + 1, vec![], vec![])
            };
            let mut entry = definition(first + index, body);
            entry["parameterCount"] = json!(0);
            entry
        })
        .collect()
}

fn recursive_true(id: usize) -> Value {
    let mut entry = definition(id, literal(true));
    entry["parameterCount"] = json!(0);
    entry["recursive"] = json!(true);
    entry
}

fn forwarded_row(
    label: &str,
    shape: Shape,
    size: usize,
    expected: String,
) -> (String, String, String) {
    let actual = summarized(evaluated(
        forwarding_chain(shape, 0, size),
        forwarded_root(shape, 0, literal(true)),
        2000,
    ));
    (format!("{label} {size}"), expected, actual)
}

#[test]
fn v2_forwarded_arguments_do_not_add_evaluation_depth() {
    let refused = format!("{DEPTH_REFUSAL} work=256");
    let mut rows = vec![];
    for (label, shape) in [
        ("parameters", Shape::Parameters),
        ("captures", Shape::Captures),
        ("alternating", Shape::Alternating),
    ] {
        for (size, expected) in [
            (127, "true work=254".to_string()),
            (254, "true work=508".to_string()),
            (255, refused.clone()),
            (256, refused.clone()),
        ] {
            rows.push(forwarded_row(label, shape, size, expected));
        }
    }
    for (size, expected) in [
        (64, "true work=254".to_string()),
        (127, "true work=506".to_string()),
        (128, refused.clone()),
        (129, refused.clone()),
    ] {
        rows.push(forwarded_row("frames", Shape::Frames, size, expected));
    }
    assert_agreement(rows);
}

fn set_root(size: usize) -> (Vec<Value>, Value) {
    let set = json!({"op":"sequence-set","argument":{"op":"set","values":[literal(true)]}});
    (
        forwarding_chain(Shape::Parameters, 0, size),
        json!({"op":"sequence-in","left":{"op":"tuple","values":[]},"right":call(0, vec![set], vec![])}),
    )
}

fn function_root(size: usize) -> (Vec<Value>, Value) {
    let function = json!({"op":"function","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(1)]}}],"body":literal(true)});
    (
        forwarding_chain(Shape::Parameters, 0, size),
        json!({"op":"at","target":call(0, vec![function], vec![]),"key":integer(1)}),
    )
}

#[test]
fn v2_forwarded_sets_and_functions_do_not_add_evaluation_depth() {
    let mut rows = vec![];
    for (label, build, extra) in [
        ("set", set_root as fn(usize) -> (Vec<Value>, Value), 3),
        ("function", function_root, 2),
    ] {
        for size in [125, 250] {
            let (definitions, root) = build(size);
            rows.push((
                format!("{label} {size}"),
                format!("true work={}", 2 * size + extra),
                summarized(evaluated(definitions, root, 5000)),
            ));
        }
        let (definitions, root) = build(251);
        rows.push((
            format!("{label} 251"),
            DEPTH_REFUSAL.to_string(),
            without_work(summarized(evaluated(definitions, root, 5000))),
        ));
    }
    assert_agreement(rows);
}

#[test]
fn v2_refusal_moves_to_the_deepest_point_after_forwarding() {
    let both = |forwards: usize, calls: usize| {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, forwards);
        definitions.extend(zero_argument_chain(forwards, calls));
        let root = json!({"op":"and","left":forwarded_root(Shape::Parameters, 0, literal(true)),"right":call(forwards, vec![], vec![])});
        summarized(evaluated(definitions, root, 5000))
    };
    assert_agreement(vec![
        (
            "forward 100 and call 198".into(),
            "true work=398".into(),
            both(100, 198),
        ),
        (
            "forward 200 and call 300".into(),
            format!("{DEPTH_REFUSAL} work=655"),
            both(200, 300),
        ),
    ]);
}

#[test]
fn v2_refusal_continuation_enters_recursion_after_forwarding() {
    let enter = |forwards: usize| {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, forwards);
        definitions.push(recursive_true(forwards));
        let root = forwarded_root(Shape::Parameters, 0, call(forwards, vec![], vec![]));
        summarized(evaluated(definitions, root, 5000))
    };
    assert_agreement(vec![
        (
            "forward 254 into a recursive call".into(),
            format!("{RECURSIVE_REFUSAL} work=509"),
            enter(254),
        ),
        (
            "forward 128 into a recursive call".into(),
            format!("{RECURSIVE_REFUSAL} work=257"),
            enter(128),
        ),
    ]);
}

#[test]
fn v2_forwarding_hops_charge_one_unit_of_work_each() {
    let chain = |maximum: u32| {
        summarized(evaluated(
            forwarding_chain(Shape::Parameters, 0, 6),
            forwarded_root(Shape::Parameters, 0, literal(true)),
            maximum,
        ))
    };
    let rows = (0..=13)
        .map(|maximum| {
            let expected = if maximum < 12 {
                format!("error budget: Safety work budget exhausted work={maximum}")
            } else {
                "true work=12".to_string()
            };
            (format!("budget {maximum}"), expected, chain(maximum))
        })
        .collect();
    assert_agreement(rows);
}

#[test]
fn v2_forwarding_charges_each_hop_before_the_next_reference() {
    let broken = |maximum: u32| {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, 6);
        definitions[3]["body"]["arguments"][0]["slot"] = json!(5);
        summarized(evaluated(
            definitions,
            forwarded_root(Shape::Parameters, 0, literal(true)),
            maximum,
        ))
    };
    assert_agreement(vec![
        (
            "budget 7".into(),
            "error budget: Safety work budget exhausted work=7".into(),
            broken(7),
        ),
        (
            "budget 2000".into(),
            "error protocol: Invalid lexical reference work=8".into(),
            broken(2000),
        ),
    ]);
}

#[test]
fn v2_forwarding_refuses_before_the_next_reference() {
    let chain = |second: Option<usize>, maximum: u32| {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, 255);
        if let Some(slot) = second {
            definitions[253]["body"]["arguments"][0]["slot"] = json!(slot);
        }
        summarized(evaluated(
            definitions,
            forwarded_root(Shape::Parameters, 0, literal(true)),
            maximum,
        ))
    };
    let refusal = format!("{DEPTH_REFUSAL} work=256");
    assert_agreement(vec![
        ("budget 256".into(), refusal.clone(), chain(None, 256)),
        (
            "unreadable second reference".into(),
            refusal.clone(),
            chain(Some(5), 2000),
        ),
        ("budget 2000".into(), refusal, chain(None, 2000)),
    ]);
}

#[test]
fn v2_recursive_forwarding_keeps_the_former_limit() {
    let marked = |size: usize| {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, size);
        for entry in &mut definitions {
            entry["recursive"] = json!(true);
        }
        summarized(evaluated(
            definitions,
            forwarded_root(Shape::Parameters, 0, literal(true)),
            2000,
        ))
    };
    assert_agreement(vec![
        ("recursive 63".into(), "true work=126".into(), marked(63)),
        (
            "recursive 64".into(),
            format!("{RECURSIVE_REFUSAL} work=128"),
            marked(64),
        ),
        (
            "recursive 65".into(),
            format!("{RECURSIVE_REFUSAL} work=128"),
            marked(65),
        ),
    ]);
}

#[test]
fn v2_recursion_entered_after_forwarding_keeps_the_former_limit() {
    let mut definitions = forwarding_chain(Shape::Parameters, 0, 64);
    definitions.push(recursive_true(64));
    let root = forwarded_root(Shape::Parameters, 0, call(64, vec![], vec![]));
    assert_agreement(vec![(
        "forward 64 into a recursive call".into(),
        format!("{RECURSIVE_REFUSAL} work=129"),
        summarized(evaluated(definitions, root, 5000)),
    )]);
}

fn enabled_body(steps: usize, subscript: Option<Value>) -> Value {
    let mut list =
        vec![json!({"kind":"assign","slot":0,"value":{"op":"read","slot":0,"next":false}})];
    for _ in 1..steps {
        list.push(json!({"kind":"guard","expression":literal(true)}));
    }
    let mut body = json!({"op":"enabled","branches":[{"name":"Step","steps":list}]});
    if let Some(subscript) = subscript {
        body["subscript"] = subscript;
    }
    body
}

fn forwarded_subscript(forwards: usize, steps: usize, recursive: bool) -> (Vec<Value>, Value) {
    let mut definitions = forwarding_chain(Shape::Parameters, 0, forwards + 1);
    definitions[forwards]["body"] = enabled_body(steps, Some(lexical(false)));
    if recursive {
        for entry in &mut definitions {
            entry["recursive"] = json!(true);
        }
    }
    (
        definitions,
        forwarded_root(
            Shape::Parameters,
            0,
            json!({"op":"read","slot":0,"next":false}),
        ),
    )
}

fn recursive_definition(id: usize, body: Value) -> Value {
    let mut entry = definition(id, body);
    entry["parameterCount"] = json!(0);
    entry["recursive"] = json!(true);
    entry
}

#[test]
fn v2_forwarded_depth_reaches_enabled_and_snapshots() {
    let enters = |definition: Value, extra: Vec<Value>, actual: Value| {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, 60);
        definitions.push(definition);
        definitions.extend(extra);
        summarized(evaluated(
            definitions,
            forwarded_root(Shape::Parameters, 0, actual),
            50_000,
        ))
    };
    let enabled = |steps, subscript| recursive_definition(60, enabled_body(steps, subscript));
    let lazy_function = json!({"op":"function","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(1)]}}],"body":call(60, vec![], vec![])});
    let lazy_chain = (0..10)
        .map(|index| {
            let body = if index == 9 {
                literal(true)
            } else {
                call(61 + index + 1, vec![], vec![])
            };
            recursive_definition(61 + index, body)
        })
        .collect();
    assert_agreement(vec![
        (
            "subscript".into(),
            format!("{RECURSIVE_REFUSAL} work=127"),
            enters(
                enabled(5, Some(literal(false))),
                vec![],
                call(60, vec![], vec![]),
            ),
        ),
        (
            "step".into(),
            format!("{RECURSIVE_REFUSAL} work=127"),
            enters(enabled(10, None), vec![], call(60, vec![], vec![])),
        ),
        (
            "snapshot".into(),
            format!("{RECURSIVE_REFUSAL} work=125"),
            enters(
                recursive_definition(60, call(61, vec![], vec![])),
                lazy_chain,
                json!({"op":"at","target":lazy_function,"key":integer(1)}),
            ),
        ),
    ]);
}

#[test]
fn v2_enabled_subscripts_resolve_forwarded_parameters_without_depth() {
    let observed = |forwards: usize, steps: usize| {
        let (definitions, root) = forwarded_subscript(forwards, steps, false);
        summarized(evaluated(definitions, root, 50_000))
    };
    let stuttering =
        |forwards: usize, steps: usize| format!("false work={}", 3 * (forwards + 1) + steps + 1);
    assert_agreement(vec![
        (
            "10 forwards, 60 steps".into(),
            stuttering(10, 60),
            observed(10, 60),
        ),
        (
            "60 forwards, 131 steps".into(),
            stuttering(60, 131),
            observed(60, 131),
        ),
        (
            "60 forwards, 132 steps".into(),
            stuttering(60, 132),
            observed(60, 132),
        ),
        (
            "100 forwards, 60 steps".into(),
            stuttering(100, 60),
            observed(100, 60),
        ),
        (
            "100 forwards, 151 steps".into(),
            stuttering(100, 151),
            observed(100, 151),
        ),
        (
            "100 forwards, 152 steps".into(),
            DEPTH_REFUSAL.into(),
            without_work(observed(100, 152)),
        ),
    ]);
}

#[test]
fn v2_enabled_subscripts_keep_the_recursive_limit_for_forwarded_parameters() {
    let observed = |forwards: usize, steps: usize| {
        let (definitions, root) = forwarded_subscript(forwards, steps, true);
        without_work(summarized(evaluated(definitions, root, 50_000)))
    };
    assert_agreement(vec![
        (
            "40 forwards, 43 steps".into(),
            "false".into(),
            observed(40, 43),
        ),
        (
            "40 forwards, 44 steps".into(),
            RECURSIVE_REFUSAL.into(),
            observed(40, 44),
        ),
        (
            "45 forwards, 40 steps".into(),
            RECURSIVE_REFUSAL.into(),
            observed(45, 40),
        ),
    ]);
}

fn read_x() -> Value {
    json!({"op":"read","slot":0,"next":false})
}

fn checked(definitions: Vec<Value>, invariant: Value) -> Result<SafetyResult, crate::CoreError> {
    let mut input = program();
    input["definitions"] = Value::Array(definitions);
    input["invariants"][0]["expression"] = invariant;
    check(input)
}

fn walk_program(guard: Value, definitions: Vec<Value>) -> Value {
    let mut input = program();
    input["next"][0]["steps"] = json!([
        {"kind":"assign","slot":0,"value":{"op":"if",
            "condition":{"op":"lt","left":read_x(),"right":integer(4)},
            "consequent":{"op":"add","left":read_x(),"right":integer(1)},
            "alternative":integer(0)}},
        {"kind":"guard","expression":guard}
    ]);
    input["invariants"][0]["expression"] = literal(true);
    input["definitions"] = Value::Array(definitions);
    input
}

fn walked(result: Result<SafetyResult, crate::CoreError>) -> String {
    match result {
        Ok(result) => format!(
            "{} states={} generated={}",
            result.status, result.states, result.generated
        ),
        Err(error) => format!("error {}: {}", error.code, error.message),
    }
}

#[test]
fn v2_primed_forwarding_keeps_the_primed_values() {
    let observed = |forwards: usize, right: Value| {
        let guard = json!({"op":"eq",
            "left":{"op":"prime","argument":forwarded_root(Shape::Parameters, 0, read_x())},
            "right":right});
        walked(check(walk_program(
            guard,
            forwarding_chain(Shape::Parameters, 0, forwards),
        )))
    };
    let primed_terminal = || {
        forwarded_root(
            Shape::Parameters,
            0,
            json!({"op":"prime","argument":read_x()}),
        )
    };
    assert_agreement(vec![
        (
            "10 forwards, unprimed right side".into(),
            "complete states=1 generated=1".into(),
            observed(10, forwarded_root(Shape::Parameters, 0, read_x())),
        ),
        (
            "10 forwards".into(),
            "complete states=5 generated=6".into(),
            observed(10, primed_terminal()),
        ),
        (
            "150 forwards".into(),
            "complete states=5 generated=6".into(),
            observed(150, primed_terminal()),
        ),
    ]);
}

const STRING_FOLD_DIAGNOSTIC: &str = "error protocol: Unsupported TLA input: set fold order for strings requires TLC intern IDs absent from the IR";
const RECORD_FOLD_DIAGNOSTIC: &str = "error protocol: Unsupported TLA input: set fold order for records requires TLC field-name intern IDs absent from the IR";

fn text(value: &str) -> Value {
    json!({"op":"data","value":{"type":"string","value":value}})
}

fn element_record(value: i32) -> Value {
    json!({"op":"record","fields":[["a", integer(value)]]})
}

fn union_fold(collection: Value, element: Value) -> Value {
    json!({"op":"fold","accumulator":0,"order":"set-members",
        "initial":{"op":"set","values":[]},"collection":collection,
        "body":{"op":"cup","left":{"op":"bound","slot":0},"right":{"op":"set","values":[element]}}})
}

fn cardinality_at_least_zero(fold: Value) -> Value {
    json!({"op":"ge","left":{"op":"cardinality","argument":fold},"right":integer(0)})
}

fn forwarded_element(first: usize, slot: usize) -> Value {
    call(first, vec![json!({"op":"bound","slot":slot})], vec![])
}

#[test]
fn v2_fold_bodies_resolve_forwarded_elements_without_depth() {
    let observed = |forwards: usize| {
        walked(checked(
            forwarding_chain(Shape::Parameters, 0, forwards),
            cardinality_at_least_zero(union_fold(
                json!({"op":"set","values":[text("a"), text("b")]}),
                forwarded_element(0, 1),
            )),
        ))
    };
    assert_agreement(vec![
        (
            "5 forwards".into(),
            "complete states=1 generated=2".into(),
            observed(5),
        ),
        (
            "200 forwards".into(),
            "complete states=1 generated=2".into(),
            observed(200),
        ),
        (
            "300 forwards".into(),
            STRING_FOLD_DIAGNOSTIC.into(),
            observed(300),
        ),
    ]);
}

#[test]
fn v2_fold_rewrites_follow_the_first_refusal_after_forwarding() {
    let observed = |first: usize, second: usize| {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, first);
        definitions.extend(forwarding_chain(Shape::Parameters, first, second));
        let strings = json!({"op":"set","values":[text("a"), text("b")]});
        let records = json!({"op":"set","values":[element_record(1), element_record(2)]});
        let invariant = json!({"op":"if",
            "condition":{"op":"eq","left":read_x(),"right":integer(0)},
            "consequent":cardinality_at_least_zero(union_fold(strings.clone(), forwarded_element(0, 1))),
            "alternative":{"op":"if",
                "condition":{"op":"eq","left":read_x(),"right":integer(1)},
                "consequent":cardinality_at_least_zero(union_fold(records, forwarded_element(first, 1))),
                "alternative":cardinality_at_least_zero(union_fold(strings, json!({"op":"div","left":integer(1),"right":{"op":"mul","left":integer(0),"right":{"op":"cardinality","argument":{"op":"set","values":[{"op":"bound","slot":1}]}}}})))}});
        let mut input = program();
        input["next"][0]["steps"] = json!([
            {"kind":"assign","slot":0,"value":{"op":"if",
                "condition":{"op":"lt","left":read_x(),"right":integer(2)},
                "consequent":{"op":"add","left":read_x(),"right":integer(1)},
                "alternative":integer(2)}}
        ]);
        input["definitions"] = Value::Array(definitions);
        input["invariants"][0]["expression"] = invariant;
        walked(check(input))
    };
    assert_agreement(vec![
        (
            "5 and 6 forwards: state 2 divides by zero".into(),
            STRING_FOLD_DIAGNOSTIC.into(),
            observed(5, 6),
        ),
        (
            "5 and 300 forwards: state 1 reaches the limit".into(),
            RECORD_FOLD_DIAGNOSTIC.into(),
            observed(5, 300),
        ),
        (
            "200 and 300 forwards: state 0 passes, state 1 reaches the limit".into(),
            RECORD_FOLD_DIAGNOSTIC.into(),
            observed(200, 300),
        ),
    ]);
}

#[test]
fn v2_fold_rewrites_skip_folds_without_a_replacement() {
    let observed = |forwards: usize| {
        let inner = json!({"op":"fold","accumulator":2,"order":"sequence-left",
            "initial":literal(true),
            "collection":{"op":"tuple","values":[{"op":"bound","slot":1}]},
            "body":forwarded_element(0, 3)});
        let outer = json!({"op":"fold","accumulator":0,"order":"set-members",
            "initial":{"op":"set","values":[]},
            "collection":{"op":"set","values":[text("a"), text("b")]},
            "body":{"op":"cup","left":{"op":"bound","slot":0},"right":{"op":"set","values":[inner]}}});
        walked(checked(
            forwarding_chain(Shape::Parameters, 0, forwards),
            cardinality_at_least_zero(outer),
        ))
    };
    assert_agreement(vec![
        (
            "5 forwards".into(),
            "complete states=1 generated=2".into(),
            observed(5),
        ),
        (
            "200 forwards".into(),
            "complete states=1 generated=2".into(),
            observed(200),
        ),
        (
            "300 forwards".into(),
            STRING_FOLD_DIAGNOSTIC.into(),
            observed(300),
        ),
    ]);
}

#[test]
fn v2_forwarding_errors_keep_their_messages_and_work() {
    let unreadable = {
        let mut definitions = forwarding_chain(Shape::Parameters, 0, 40);
        definitions[10]["body"]["arguments"][0]["slot"] = json!(5);
        summarized(evaluated(
            definitions,
            forwarded_root(Shape::Parameters, 0, literal(true)),
            2000,
        ))
    };
    let outside_a_frame = summarized(evaluated(
        forwarding_chain(Shape::Parameters, 0, 40),
        forwarded_root(Shape::Parameters, 0, lexical(false)),
        2000,
    ));
    assert_agreement(vec![
        (
            "unreadable slot 29 hops away".into(),
            "error protocol: Invalid lexical reference work=69".into(),
            unreadable,
        ),
        (
            "forwarded argument outside any frame".into(),
            "error protocol: Missing lexical call frame work=80".into(),
            outside_a_frame,
        ),
    ]);
}

#[test]
fn v2_forwarded_bound_and_old_keep_their_lexical_values() {
    let quantified = checked(
        forwarding_chain(Shape::Parameters, 0, 5),
        json!({"op":"forall","bounds":[{"slot":0,"domain":{"kind":"set","values":[integer(1), integer(2)]}}],
            "body":{"op":"ge","left":call(0, vec![json!({"op":"bound","slot":0})], vec![]),"right":integer(1)}}),
    );
    let excepted = checked(
        forwarding_chain(Shape::Captures, 0, 5),
        json!({"op":"eq",
            "left":{"op":"update","target":{"op":"tuple","values":[integer(7)]},
                "updates":[{"path":[integer(1)],"value":forwarded_root(Shape::Captures, 0, json!({"op":"old"}))}]},
            "right":{"op":"tuple","values":[integer(7)]}}),
    );
    assert_agreement(vec![
        (
            "quantified variable".into(),
            "complete states=1 generated=2".into(),
            walked(quantified),
        ),
        (
            "EXCEPT old value".into(),
            "complete states=1 generated=2".into(),
            walked(excepted),
        ),
    ]);
}

#[test]
fn v1_enabled_subscripts_at_256_skip_the_depth_check() {
    use crate::ir::{Branch, Expression, Scalar, Step};
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;

    let mut steps = vec![Step::Assign {
        slot: 0,
        value: Expression::Read {
            slot: 0,
            next: false,
        },
    }];
    steps.extend((0..254).map(|_| Step::Guard {
        expression: Expression::Literal {
            value: Scalar::Bool(true),
        },
    }));
    let expression = Expression::Enabled {
        branches: vec![Branch {
            name: "Longest".into(),
            steps,
        }],
        subscript: Some(Box::new(Expression::Read {
            slot: 0,
            next: false,
        })),
    };
    assert_eq!(
        expression
            .eval(
                EvalContext::new(&[Scalar::Int(0)], &[]),
                &mut Arena::new(10_000)
            )
            .unwrap(),
        Scalar::Bool(false)
    );
}
