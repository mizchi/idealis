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

#[test]
fn v2_dynamic_call_depth_is_checked_before_exhausting_the_native_stack() {
    use crate::ir::{Definition, Expression, Scalar};
    use crate::safety_expressions::EvalContext;
    use crate::safety_values::Arena;

    for count in [127, 128, 129] {
        let definitions: Vec<Definition> = (0..count)
            .map(|id| {
                let body = if id + 1 == count {
                    literal(true)
                } else {
                    call(id + 1, vec![], vec![])
                };
                let mut input = definition(id, body);
                input["parameterCount"] = json!(0);
                serde_json::from_value(input).unwrap()
            })
            .collect();
        let expression: Expression = serde_json::from_value(call(0, vec![], vec![])).unwrap();
        let mut arena = Arena::new(1000);
        arena.definitions = &definitions;
        let result = expression.eval(EvalContext::new(&[], &[]), &mut arena);
        if count == 127 {
            assert_eq!(result.unwrap(), Scalar::Bool(true));
            assert_eq!(arena.work, 127);
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code, "protocol");
            assert_eq!(error.message, "Safety evaluation nesting exceeds 128");
            assert_eq!(arena.work, 128);
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
    let error = build(60).check(bounds).unwrap_err();
    assert_eq!(error.code, "protocol");
    assert_eq!(error.message, "Safety evaluation nesting exceeds 128");
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
