use rmodel::reference::{Machine, Next, Outcome, evaluate};

const SOURCE: &str = include_str!("../../../spec/models/rmodel-behavior.model");
const SWAP: &str = "behavior swap() {
  guard { left = Off right = On }
  update { left = right right = left }
}";

fn source(declaration: &str) -> String {
    assert_eq!(SOURCE.matches(SWAP).count(), 1);
    SOURCE.replacen(SWAP, declaration, 1)
}

#[test]
fn behavior_has_the_same_guard_parallel_updates_and_frame_as_action() {
    let old = rmodel::parse(&source(
        "action swap() { left = Off right = On left' = right right' = left }",
    ))
    .unwrap();
    let new = rmodel::parse(SOURCE).unwrap();

    let verdicts = evaluate(&new, 100);
    assert_eq!(verdicts, evaluate(&old, 100));
    for (name, expected) in [
        ("PreservesUpdate", Outcome::Holds),
        ("Distinct", Outcome::Holds),
        ("Swapped", Outcome::Reachable { depth: 1 }),
        ("BothOn", Outcome::Unreachable),
    ] {
        assert_eq!(
            verdicts.iter().find(|v| v.check == name).unwrap().outcome,
            expected
        );
        assert_eq!(
            rmodel::to_tla(&new, name).unwrap(),
            rmodel::to_tla(&old, name).unwrap()
        );
    }
    assert_eq!(rmodel::to_alloy(&new), rmodel::to_alloy(&old));

    let machine = Machine::new(&new).unwrap();
    let config = machine.configurations().next().unwrap();
    let initial = machine.initial(&config);
    let steps = machine.steps(&config, &initial[0]);
    let Some(Next::State(next)) = &steps[0].next else {
        panic!("expected the swap to be enabled");
    };
    assert!(machine.steps(&config, next)[0].next.is_none());
}

#[test]
fn behavior_checks_update_targets_and_keeps_legacy_identifiers() {
    use rmodel::ErrorKind::{Syntax, Type};

    for (declaration, kind) in [
        ("behavior swap() { update { left = On left = Off } }", Type),
        ("behavior swap() { update { Value = On } }", Type),
        ("behavior swap() { update { left' = On } }", Syntax),
        ("behavior swap() { left = Off }", Syntax),
        (
            "behavior swap() { guard { left = Off } guard { right = On } }",
            Syntax,
        ),
    ] {
        assert_eq!(
            rmodel::parse(&source(declaration)).unwrap_err().kind,
            kind,
            "{declaration}"
        );
    }
    for declaration in [
        "action behavior(guard: Value) { guard != left update' = guard }",
        "behavior swap() { update { left = On } }",
        "behavior swap() { guard { left = Off } }",
        "behavior swap() {}",
    ] {
        assert!(rmodel::parse(&source(declaration)).is_ok(), "{declaration}");
    }
}

#[test]
fn unchanged_expands_to_equality_of_the_same_expression_in_both_states() {
    let source = include_str!("../../../fixtures/rcheck/structured/shipped-transition.model");
    let written = "payload(before(status), Shipped) = payload(after(status), Shipped)";
    assert_eq!(source.matches(written).count(), 2);
    for (suffix, expected) in [
        ("", Outcome::Holds),
        (
            "behavior reroute() {
               guard { is(status, Shipped) }
               update { status = Shipped(Courier) }
             }",
            Outcome::Violated { depth: 2 },
        ),
    ] {
        let source = format!("{source}\n{suffix}");
        let original = rmodel::parse(&source).unwrap();
        let candidate =
            rmodel::parse(&source.replace(written, "unchanged(payload(status, Shipped))")).unwrap();
        let explicit = rmodel::parse(&source.replace(
            written,
            "before(payload(status, Shipped)) = after(payload(status, Shipped))",
        ))
        .unwrap();

        let report = evaluate(&candidate, 1_000);
        assert_eq!(report, evaluate(&original, 1_000));
        assert_eq!(report, evaluate(&explicit, 1_000));
        assert_eq!(
            report
                .iter()
                .find(|verdict| verdict.check == "KeepsTracking")
                .unwrap()
                .outcome,
            expected
        );
        for check in ["ShippedStays", "KeepsTracking"] {
            assert_eq!(
                rmodel::to_tla(&candidate, check).unwrap(),
                rmodel::to_tla(&explicit, check).unwrap()
            );
        }
    }
}

#[test]
fn unchanged_preserves_names_and_requires_a_transition_with_an_unphased_expression() {
    let source = "model KeepSet
        enum Member = A | B
        var members: set Member
        var unchanged: one Bool
        init { members = A unchanged }
        behavior visit() { update { unchanged = false } }
        transition Stable { unchanged(members) }
        scope steps = 2";
    let model = rmodel::parse(source).unwrap();
    let stable = evaluate(&model, 100)
        .into_iter()
        .find(|verdict| verdict.check == "Stable")
        .unwrap();
    assert_eq!(stable.outcome, Outcome::Holds);
    for invalid in [
        source.replace("transition Stable", "invariant Stable"),
        source.replace("unchanged(members)", "unchanged(after(members))"),
        source.replace("unchanged(members)", "unchanged(unknown)"),
    ] {
        assert_eq!(
            rmodel::parse(&invalid).unwrap_err().kind,
            rmodel::ErrorKind::Type
        );
    }
    let constructor = "model UseConstructor
        enum Member = A
        variant Result = unchanged(Member)
        var value: one Result
        init { value = unchanged(A) }
        invariant Same { unchanged(A) = value }
        scope steps = 2";
    assert!(rmodel::parse(constructor).is_ok());
}

#[test]
fn nested_conditions_select_values_in_the_selected_state() {
    let source = "model Choice
      enum Item = A | B
      var selected: lone Item
      init { selected = none }
      behavior select() {
        guard { no selected }
        update { selected = if { no selected } then (if { A in Item } then A else B) else selected }
      }
      transition ChoosesA {
        after(if { some selected } then selected else B) = A
      }
      reachable Selected { selected = A }
      scope steps = 3";
    let model = rmodel::parse(source).unwrap();
    let verdicts = evaluate(&model, 100);
    assert_eq!(verdicts[0].outcome, Outcome::Reachable { depth: 1 });
    assert_eq!(verdicts[1].outcome, Outcome::Holds);
    let (source, cfg) = rmodel::to_tla(&model, "ChoosesA").unwrap();
    let result = idealis_core::compile_tla(&source, &cfg)
        .unwrap()
        .check(idealis_core::SafetyBounds {
            max_depth: 8,
            max_states: 100,
            max_work: 1_000_000,
            max_temporal_nodes: None,
        })
        .unwrap();
    assert_eq!(result.status, "complete");
    assert!(result.violation.is_none());
}

#[test]
fn a_conditional_checks_types_and_evaluates_the_chosen_branch() {
    let source = "model Choice
      var value: one Nat
      var if: one Bool
      init { value = 0 if { true } }
      behavior choose() {
        guard { value = 0 }
        update { value = if { if } then 1 else 2 }
      }
      reachable Chosen { value = 1 }
      scope Int = 0..1, steps = 2";
    let check = |text: &str| {
        let model = rmodel::parse(text).unwrap();
        rmodel::reference::analyze(&model, 1_000)
    };
    assert_eq!(check(source).scope, Outcome::Holds);
    assert_eq!(
        check(source).verdicts[0].outcome,
        Outcome::Reachable { depth: 1 }
    );
    assert_eq!(
        check(&source.replace("if { if }", "if { false }")).scope,
        Outcome::Violated { depth: 1 }
    );
    let ill_typed = source.replace("then 1 else 2", "then 1 else true");
    assert!(
        rmodel::parse(&ill_typed)
            .unwrap_err()
            .message
            .contains("conditional branches")
    );
}

#[test]
fn alloy_counts_in_conditional_updates_fit_the_model_scope() {
    let model = rmodel::parse(
        "model LargeConditionalCount
sig A
sig B
type Item = A + B
enum Flag = Off | On
var flag: one Flag
init { flag = Off }
behavior choose() { update { flag = if { #Item > 7 } then On else Off } }
reachable Chosen { flag = On }
scope A = 4, B = 4, steps = 2",
    )
    .unwrap();
    assert!(rmodel::to_alloy(&model).contains("5 Int"));
    assert_eq!(
        evaluate(&model, 100)[0].outcome,
        Outcome::Reachable { depth: 1 }
    );
}

#[test]
fn a_transition_can_check_only_one_action_and_bind_its_arguments() {
    use Outcome::{Holds, Reachable, Violated};
    let source = include_str!("../../../examples/model-patterns/retry/delivery.model");
    let answer = |source: &str, name: &str| {
        evaluate(&rmodel::parse(source).unwrap(), 10_000)
            .into_iter()
            .find(|answer| answer.check == name)
            .unwrap()
            .outcome
    };
    assert_eq!(answer(source, "RetryPreservesEffect"), Holds);
    assert_eq!(answer(source, "DeliveryReturnsReceipt"), Holds);
    assert_eq!(answer(source, "LostAfterCommit"), Reachable { depth: 2 });
    assert_eq!(
        answer(source, "IndependentRequests"),
        Reachable { depth: 2 }
    );
    let fault = source.replace(
        "then effects else",
        "then (effects - request -> Effect) + request -> Duplicate else",
    );
    assert_eq!(
        answer(&fault, "RetryPreservesEffect"),
        Violated { depth: 2 }
    );
    assert!(
        rmodel::parse(&source.replace("on deliver", "on missing"))
            .unwrap_err()
            .to_string()
            .contains("unknown action missing")
    );
}

#[test]
fn relationship_override_replaces_only_the_domain_of_the_patch() {
    let model = rmodel::parse(include_str!(
        "../../../fixtures/rmodel/relations/override.model"
    ))
    .unwrap();
    let answers = evaluate(&model, 20_000);
    assert_eq!(answers[0].outcome, Outcome::Holds);
    assert_eq!(answers[1].outcome, Outcome::Holds);
    assert_eq!(answers[2].outcome, Outcome::Reachable { depth: 1 });
    assert!(rmodel::parse("model Invalid enum Tab = Left invariant OnlyPairs { Left ++ Left = Left } scope steps = 2").is_err());
}

#[test]
fn a_parenthesized_value_can_start_a_post_state_comparison() {
    let model = rmodel::parse(&format!(
        "{SOURCE}\ntransition Parens {{ (left)' = after(left) }}"
    ))
    .unwrap();
    assert_eq!(
        evaluate(&model, 100)
            .into_iter()
            .find(|check| check.check == "Parens")
            .unwrap()
            .outcome,
        Outcome::Holds
    );
}

#[test]
fn a_comma_separates_a_parenthesized_condition_from_the_previous_name() {
    let suffix = r#"
        invariant Grouped {
            left in Value,
            (right = Off or left = On),
        }
    "#;
    let separated = rmodel::parse(&format!("{SOURCE}{suffix}")).unwrap();
    let explicit = rmodel::parse(&format!(
        "{SOURCE} invariant Grouped {{ left in Value and (right = Off or left = On) }}"
    ))
    .unwrap();
    let verdicts = evaluate(&separated, 100);
    assert_eq!(verdicts, evaluate(&explicit, 100));
    assert_eq!(
        verdicts
            .iter()
            .find(|v| v.check == "Grouped")
            .unwrap()
            .outcome,
        Outcome::Violated { depth: 0 }
    );
    for invalid in ["left in Value,, right = On", ", left in Value"] {
        assert!(rmodel::parse(&format!("{SOURCE} invariant Invalid {{ {invalid} }}")).is_err());
    }
}

#[test]
fn conditional_update_groups_read_the_prestate_and_preserve_skipped_fields() {
    let source = r#"
        model Grouped
        enum Lamp = Off | On
        var left: one Lamp
        var right: one Lamp
        var done: one Bool
        init { left = Off right = On done = false }
        behavior swap() {
          guard { not done }
          update {
            if { right = On } then { left = right right = left } else { left = Off }
            done = true
          }
        }
        behavior reset() {
          guard { done }
          update { if { done } then { if { left = On } then { left = Off } } }
        }
        reachable Swapped { left = On right = Off done }
        reachable Reset { left = Off right = Off done }
        scope steps = 3
    "#;
    let grouped = rmodel::parse(source).unwrap();
    let expanded = rmodel::parse(
        &source
            .replace(
                "if { right = On } then { left = right right = left } else { left = Off }",
                "left = if { right = On } then right else Off
         right = if { right = On } then left else right",
            )
            .replace(
                "if { left = On } then { left = Off }",
                "left = if { left = On } then Off else left",
            ),
    )
    .unwrap();

    let verdicts = evaluate(&grouped, 500);
    assert_eq!(verdicts, evaluate(&expanded, 500));
    for (name, depth) in [("Swapped", 1), ("Reset", 2)] {
        assert_eq!(
            verdicts
                .iter()
                .find(|verdict| verdict.check == name)
                .unwrap()
                .outcome,
            Outcome::Reachable { depth },
        );
        assert_eq!(
            rmodel::to_tla(&grouped, name).unwrap(),
            rmodel::to_tla(&expanded, name).unwrap()
        );
    }
    assert_eq!(rmodel::to_alloy(&grouped), rmodel::to_alloy(&expanded));
}

#[test]
fn conditional_updates_reject_duplicates_and_an_empty_group() {
    for update in [
        "if { true } then { left = On left = Off }",
        "if { true } then { left = On } left = Off",
        "if { missing } then {} else {}",
    ] {
        let declaration = format!("behavior swap() {{ update {{ {update} }} }}");
        assert!(rmodel::parse(&source(&declaration)).is_err(), "{update}");
    }
}

#[test]
fn expansion_of_conditional_updates_stays_within_the_node_budget() {
    let condition = vec!["true"; 80].join(" and ");
    let targets = (0..80)
        .map(|index| format!("v{index} = true"))
        .collect::<Vec<_>>()
        .join(" ");
    let declaration =
        format!("behavior swap() {{ update {{ if {{ {condition} }} then {{ {targets} }} }} }}");
    let error = rmodel::parse(&source(&declaration)).unwrap_err();
    assert!(error.message.contains("more than 4096 nodes"), "{error}");
}

#[test]
fn presence_guards_make_optional_integers_available_only_where_proven() {
    let source = "
        model OptionalPosition
        enum Request = Old | New
        var captured: Request -> lone Nat
        var cursor: one Nat
        init { no captured cursor = 0 }
        behavior capture(request: Request) {
            update { captured = captured ++ request -> 1 }
        }
        behavior apply(request: Request) {
            guard { some request.captured and request.captured >= cursor }
            update { cursor = request.captured + 1 }
        }
        behavior recover(request: Request) {
            update { cursor = (if { some request.captured } then request.captured else 0) + 1 }
        }
        reachable Applied { cursor = 2 }
        scope Int = 0..3, steps = 4
    ";
    let model = rmodel::parse(source).unwrap();
    assert_eq!(
        evaluate(&model, 1000)[0].outcome,
        Outcome::Reachable { depth: 2 }
    );
    for (from, to) in [
        (
            "some request.captured and request.captured >= cursor",
            "request.captured >= cursor and some request.captured",
        ),
        (
            "some request.captured and request.captured >= cursor",
            "some request.captured or cursor = 0",
        ),
        (
            "some request.captured and request.captured >= cursor",
            "some Old.captured",
        ),
        (
            "some request.captured and request.captured >= cursor",
            "not (some request.captured)",
        ),
        (
            "some request.captured and request.captured >= cursor",
            "some Request.captured and Request.captured >= cursor",
        ),
        (
            "then request.captured else 0",
            "then 0 else request.captured",
        ),
    ] {
        let error = rmodel::parse(&source.replace(from, to)).unwrap_err();
        assert!(error.to_string().contains("exactly one integer"), "{error}");
    }
}

#[test]
fn a_presence_guard_can_protect_a_sequence_lookup() {
    let source = "
        model OptionalHead
        enum Entry = A
        sequence Queue = Entry, max = 2
        var pending: one Queue
        var processed: one Queue
        init { pending = Queue[] processed = Queue[] }
        behavior enqueue() {
            guard { len(pending) = 0 }
            update { pending = Queue[A] }
        }
        behavior process() {
            guard { some at(pending, 0) len(processed) < 2 }
            update { processed = append(processed, at(pending, 0)) pending = tail(pending) }
        }
        reachable Processed { processed = Queue[A] }
        scope Int = 0..2, steps = 4
    ";
    let model = rmodel::parse(source).unwrap();
    assert_eq!(
        evaluate(&model, 1000)[0].outcome,
        Outcome::Reachable { depth: 2 }
    );
    let error = rmodel::parse(&source.replace("some at(pending, 0) ", "")).unwrap_err();
    assert!(error.to_string().contains("expected one Entry"), "{error}");
}

#[test]
fn reachable_from_includes_roots_and_only_nodes_on_their_paths() {
    use rmodel::reference::{self, Outcome};
    let source = "model Closure
enum Node = Root | Near | Far | Other
var links: Node -> lone Node
init { links = (Root -> Near) + (Near -> Far) + (Far -> Near) }
behavior connect() { update { links = links ++ (Far -> Other) } }
invariant ReachedCycle { Root + Near + Far in reachableFrom(Root, links) }
invariant EmptyRoots { no reachableFrom(none, links) }
invariant EmptyEdges { reachableFrom(Root + Other, none) = Root + Other }
reachable NewNode { Other in reachableFrom(Root, links) }
transition ChangeUsesBothGraphs on connect {
  (Other not in before(reachableFrom(Root, links))) implies Other in after(reachableFrom(Root, links))
}
scope steps = 4";
    let model = rmodel::parse(source).unwrap();
    let report = reference::analyze(&model, 10_000);
    assert_eq!(report.scope, Outcome::Holds);
    assert_eq!(
        report
            .verdicts
            .iter()
            .map(|v| v.outcome)
            .collect::<Vec<_>>(),
        [
            Outcome::Holds,
            Outcome::Holds,
            Outcome::Holds,
            Outcome::Reachable { depth: 1 },
            Outcome::Holds
        ],
    );
    assert!(
        rmodel::parse(&source.replace("reachableFrom(Root, links)", "reachableFrom(links, Root)"))
            .is_err()
    );
}

#[test]
fn closure_requires_comparable_atom_sorts_and_preserves_variant_tags() {
    for expression in [
        "closure()",
        "closure(A)",
        "closure(A -> N)",
        "closure(0 -> 0)",
    ] {
        let source = format!(
            "model Invalid enum Node = A enum Other = N
             invariant Invalid {{ no {expression} }} scope Int = 0..1, steps = 2"
        );
        let error = rmodel::parse(&source).unwrap_err();
        assert!(error.message.contains("closure needs"), "{error}");
    }
    let source = "model Existing enum Node = A variant Event = closure(Node)
                  var event: one Event init { event = closure(A) }
                  invariant StillAConstructor { is(event, closure) } scope steps = 2";
    assert!(rmodel::parse(source).is_ok());
    let source = "model Invalid record Node { value: Bool }
                  invariant Invalid { no closure(Node { value: true } -> Node { value: true }) } scope steps = 2";
    assert!(
        rmodel::parse(source)
            .unwrap_err()
            .message
            .contains("closure needs")
    );
}

#[test]
fn model_derivations_obey_name_and_phase_rules() {
    let prefix = "model Bad enum Item = A var seen: set Item init { no seen }";
    let cases = [
        ("derive derived = derived", "unknown name derived"),
        (
            "derive earlier = later derive later = Item",
            "unknown name later",
        ),
        ("derive seen = Item", "duplicate name seen"),
        (
            "derive derived = seen fact { no derived }",
            "fact refers to the var seen",
        ),
        (
            "derive derived = after(seen)",
            "expected a state expression",
        ),
        ("derive derived = unknown", "unknown name unknown"),
        (
            "derive derived = seen behavior add(derived: Item) {}",
            "derived shadows another name",
        ),
        (
            "derive derived = seen behavior add() { update { derived = A } }",
            "derived is not a var",
        ),
    ];
    for (definition, expected) in cases {
        let error = rmodel::parse(&format!("{prefix} {definition} scope steps = 2")).unwrap_err();
        assert!(error.message.contains(expected), "{definition}: {error}");
    }
}

#[test]
fn derived_expression_expansion_cannot_exceed_tree_limits() {
    let mut source = "model Deep enum Item = A derive d0 = Item".to_owned();
    for index in 1..=20 {
        let previous = index - 1;
        source.push_str(&format!(" derive d{index} = d{previous} + d{previous}"));
    }
    source.push_str(" scope steps = 2");
    let error = rmodel::parse(&source).unwrap_err();
    assert!(error.message.contains("size limit"), "{error}");
}

#[test]
fn value_arm_expectations_are_limited_to_direct_conditional_assignments() {
    let model = |value: &str| {
        format!(
            "model Assigned
             enum Choice = expect | Other
             var picked: set Choice
             init {{ no picked }}
             behavior choose() {{ update {{ picked = {value} }} }}
             scope steps = 2"
        )
    };
    let plain = "if { false } then expect else Other";
    let annotated =
        r#"if { false } then expect unreachable "This arm is disabled" expect else Other"#;
    assert!(rmodel::parse(&model(plain)).is_ok());
    assert!(rmodel::parse(&model(annotated)).is_ok());
    let located = rmodel::parse_located(&model(annotated)).unwrap();
    assert_eq!(
        located.update_branches["choose"][0].unreachable_reasons(),
        &[Some("This arm is disabled".into()), None]
    );
    let embedded = rmodel::parse(&model(&format!("({annotated}) + Other"))).unwrap_err();
    assert!(embedded.message.contains("direct conditional assignment"));
    let omitted =
        rmodel::parse(&model(&annotated.replace("This arm is disabled", ""))).unwrap_err();
    assert!(omitted.message.contains("nonempty quoted reason"));
}
