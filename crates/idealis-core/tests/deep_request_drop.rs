use idealis_core::{Command, Engine, Message, Request, SafetyBounds, SafetyProgram};
use rtlc::ir::{BodyKind, Branch, Definition, Expression, Scalar, Step};

const DEPTH: usize = 100_000;
const STACK_BYTES: usize = 8 * 1024 * 1024;

fn deep_action_frame_program() -> SafetyProgram {
    let leaf = vec![Branch {
        name: "Leaf".into(),
        steps: vec![Step::Guard {
            expression: Expression::Literal {
                value: Scalar::Bool(true),
            },
        }],
    }];
    let branches = (0..DEPTH).fold(leaf, |branches, _| {
        vec![Branch {
            name: "Nested".into(),
            steps: vec![Step::Frame {
                arguments: vec![],
                captures: vec![],
                parameter_count: 0,
                capture_count: 0,
                local_scope: 0,
                branches,
            }],
        }]
    });
    SafetyProgram {
        format: "modelt.safety-program".into(),
        version: 2,
        name: "DeepDrop".into(),
        variables: vec!["x".into()],
        initial: vec![],
        next: vec![],
        invariants: vec![],
        assumptions: vec![],
        check_deadlock: false,
        symmetry: None,
        initial_properties: vec![],
        action_properties: vec![],
        properties: vec![],
        fairness: vec![],
        constraints: vec![],
        action_constraints: vec![],
        definitions: Some(vec![Definition {
            recursive: false,
            id: 0,
            parameter_count: 0,
            capture_count: 0,
            body_kind: BodyKind::Expression,
            local_scope: 0,
            body: Expression::Enabled {
                branches,
                subscript: None,
            }
            .into(),
        }]),
    }
}

#[test]
fn deep_drop_engine_handle_check_safety_action_frames() {
    let message = std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| {
            Engine::default().handle(Command {
                version: 1,
                request: Request::CheckSafety {
                    options: None,
                    program: deep_action_frame_program(),
                    bounds: SafetyBounds {
                        max_depth: 8,
                        max_states: 100,
                        max_work: 10000,
                        max_temporal_nodes: Some(10000),
                    },
                },
            })
        })
        .unwrap()
        .join()
        .expect("the engine checks and drops the deep request without panicking");
    let Message::Error { code, message } = message else {
        panic!("expected the depth refusal: {message:?}")
    };
    assert_eq!(code, "protocol");
    assert_eq!(message, "Safety expression nesting exceeds 48");
}
