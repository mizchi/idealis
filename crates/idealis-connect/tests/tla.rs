#![cfg(feature = "tla")]
use idealis_connect::tla::{
    ConnectSpec, Driver, FiniteModel, SafetyBounds, Status, Value, check_driver, finite_model,
};

const MODEL: &str = r#"---- MODULE Store ----
EXTENDS Naturals
VARIABLE x, act, obs
Init == x = 0 /\ act = "start" /\ obs \in {0, 1}
Next == \E v \in {0, 1, 2}: x' = v /\ act' = <<"put", v>> /\ obs' = v
Limit == x < 2
StepLimit == x' < 2
===="#;
const CFG: &str = "INIT Init\nNEXT Next\n";
fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 8,
        max_states: 32,
        max_work: 100000,
        max_temporal_nodes: None,
    }
}
struct Store(bool);
impl Driver for Store {
    fn reset(&mut self) -> Result<Value, String> {
        Ok(Value::Int("1".into()))
    }
    fn step(&mut self, action: &Value) -> Result<Value, String> {
        let Value::List(args) = action else {
            return Err("expected put".into());
        };
        Ok(if self.0 {
            Value::Int("9".into())
        } else {
            args[1].clone()
        })
    }
}
#[test]
fn constrained_graph_preserves_initial_observations_and_checks_live_stores() {
    for constraint in ["CONSTRAINT Limit", "ACTION_CONSTRAINT StepLimit"] {
        let model: FiniteModel = finite_model(
            MODEL,
            &format!("{CFG}{constraint}"),
            ConnectSpec::default(),
            bounds(),
        )
        .unwrap();
        assert_eq!(model.states.len(), 2);
        assert_eq!(model.initial.len(), 2);
        assert_eq!(model.initial[0].key, model.initial[1].key);
        let action = |v: &str| Value::List(vec![Value::String("put".into()), Value::Int(v.into())]);
        for (action, faulty, status) in [
            (action("1"), false, Status::Matched),
            (action("1"), true, Status::Mismatch),
            (action("2"), false, Status::Invalid),
        ] {
            assert_eq!(
                check_driver(&model, vec![action], &mut Store(faulty))
                    .unwrap()
                    .status,
                status
            );
        }
        let generator = idealis_connect::Generator::from_finite_model(&model, "1", 1, 3).unwrap();
        assert_eq!(generator.initial.len(), 1);
        let plan = idealis_connect::Plan {
            generator,
            seed: 7,
            cases: 4,
            max_attempts: 16,
            max_shrink_checks: 64,
            max_work: 100000,
        };
        let report = idealis_connect::tla::check(&model, plan, &mut Store(true)).unwrap();
        assert_eq!(report.result.status, idealis_connect::TestStatus::Failed);
        assert_eq!(report.counterexample.unwrap().status, Status::Mismatch);
        let actions: Vec<Value> = report.counterexample_actions.unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], Value::List(args) if args[0] == Value::String("put".into())));
        assert_eq!(
            serde_json::to_value(actions).unwrap(),
            serde_json::to_value(report.result.counterexample.unwrap().actions).unwrap()
        );
    }
}
#[test]
fn incomplete_or_unsound_projection_cannot_become_a_finite_model() {
    assert!(
        finite_model(
            MODEL,
            CFG,
            ConnectSpec::default(),
            SafetyBounds {
                max_states: 1,
                ..bounds()
            }
        )
        .is_err()
    );
    let source = MODEL.replace("x' = v", "obs = 0 /\\ x' = v");
    let error = finite_model(
        &source,
        &format!("{CFG}CHECK_DEADLOCK FALSE"),
        ConnectSpec::default(),
        bounds(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("projection"), "{error}");
}

#[test]
fn actions_unavailable_after_a_valid_nondeterministic_reset_are_invalid() {
    let source = r#"---- MODULE Fork ----
VARIABLE x, act, obs
Init == x \in {0, 1} /\ act = "start" /\ obs = x
Next == x = 0 /\ UNCHANGED x /\ act' = <<"put", 0>> /\ obs' = 0
===="#;
    let model = finite_model(
        source,
        &format!("{CFG}CHECK_DEADLOCK FALSE"),
        ConnectSpec::default(),
        bounds(),
    )
    .unwrap();
    let mut generator = idealis_connect::Generator::from_finite_model(&model, "1", 1, 1).unwrap();
    generator
        .initial
        .retain(|key| !generator.states[key].is_empty());
    let plan = idealis_connect::Plan {
        generator,
        seed: 1,
        cases: 1,
        max_attempts: 2,
        max_shrink_checks: 3,
        max_work: 10000,
    };
    let report = idealis_connect::tla::check(&model, plan, &mut Store(false)).unwrap();
    assert_eq!(report.result.status, idealis_connect::TestStatus::Exhausted);
    assert!(report.counterexample.is_none());
    assert!(report.counterexample_actions.is_none());
}

#[test]
fn reset_mismatch_keeps_the_whole_saved_case_even_when_no_steps_ran() {
    let source = MODEL.replace("obs \\in {0, 1}", "obs = 0");
    let model = finite_model(&source, CFG, ConnectSpec::default(), bounds()).unwrap();
    for length in [0, 2] {
        let plan = idealis_connect::Plan {
            generator: idealis_connect::Generator::from_finite_model(&model, "1", length, length)
                .unwrap(),
            seed: 7,
            cases: 1,
            max_attempts: 2,
            max_shrink_checks: 16,
            max_work: 10000,
        };
        let report = idealis_connect::tla::check(&model, plan, &mut Store(false)).unwrap();
        assert_eq!(report.result.status, idealis_connect::TestStatus::Failed);
        assert_eq!(report.counterexample.unwrap().checked_steps, 0);
        let actions = report.counterexample_actions.unwrap();
        assert_eq!(actions.len(), length);
        assert_eq!(
            serde_json::to_value(actions).unwrap(),
            serde_json::to_value(report.result.counterexample.unwrap().actions).unwrap()
        );
    }
}
