use rtlc::{SafetyBounds, SafetyOptions, SafetyProgram};

fn input(count: usize) -> String {
    let flags: Vec<_> = (0..count).map(|index| format!("flag{index}")).collect();
    let variables = [&["visible".to_string()][..], &flags].concat();
    let initial = flags
        .iter()
        .map(|name| format!(" /\\ {name} = FALSE"))
        .collect::<String>();
    let actions = flags
        .iter()
        .map(|name| {
            format!(
                "( /\\ ~{name} /\\ {name}' = TRUE /\\ UNCHANGED <<{}>>)",
                variables
                    .iter()
                    .filter(|variable| *variable != name)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(" \\/ ");
    format!(
        r#"---- MODULE PartialOrder ----
EXTENDS Naturals
VARIABLES {}
Init == visible = 0{initial}
Next == {actions} \/ (visible' = (IF visible < 3 THEN visible + 1 ELSE 0) /\ UNCHANGED <<{}>>)
Safe == visible <= 3
===="#,
        variables.join(","),
        flags.join(",")
    )
}

fn compile(source: &str) -> SafetyProgram {
    rtla_compiler::compile(
        &rtla::parse_module(source).unwrap(),
        &rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe").unwrap(),
    )
    .unwrap()
}
fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 100,
        max_states: 10000,
        max_work: 10000000,
        max_temporal_nodes: None,
    }
}
fn options() -> SafetyOptions {
    SafetyOptions {
        partial_order: true,
        ..Default::default()
    }
}

#[test]
fn invisible_monotone_actions_reduce_interleavings_and_keep_real_faults() {
    for count in [2, 3] {
        let source = input(count);
        let program = compile(&source);
        assert_eq!(
            program.check(bounds()).unwrap().states,
            4 * 2_usize.pow(count as u32)
        );
        let result = program.check_with_options(bounds(), options()).unwrap();
        assert_eq!(
            (result.status.as_str(), result.states, result.generated),
            ("complete", count + 4, count as u32 + 5)
        );
        let faulty = compile(&source.replace("Safe == visible <= 3", "Safe == visible < 3"));
        let result = faulty.check_with_options(bounds(), options()).unwrap();
        assert_eq!(result.status, "invariant-violation");
        assert_eq!(result.violation.unwrap().trace.len(), count + 4);
        for maximum in [1, 17] {
            let result = program
                .check_with_options(
                    SafetyBounds {
                        max_work: maximum,
                        ..bounds()
                    },
                    options(),
                )
                .unwrap();
            assert_eq!(result.status, "bounded");
            assert!(result.work <= maximum);
        }
    }
}

#[test]
fn visible_dependent_and_cyclic_actions_fall_back_without_changing_results() {
    let source = input(1);
    for source in [
        source.replace("Safe == visible <= 3", "Safe == ~flag0"),
        source.replace("flag0' = TRUE", "flag0' = ~flag0"),
        source.replace("(visible' =", "(~flag0 /\\ visible' ="),
        source.replace("ELSE 0", "ELSE IF flag0 THEN 0 ELSE 0"),
    ] {
        let program = compile(&source);
        assert_eq!(
            program.check(bounds()).unwrap(),
            program.check_with_options(bounds(), options()).unwrap()
        );
    }
    let program = compile(&input(3).replace("(visible' =", "(visible < 3 /\\ visible' ="));
    let result = program.check_with_options(bounds(), options()).unwrap();
    assert_eq!(result.status, "deadlock");
    assert_eq!(result.violation.unwrap().trace.len(), 7);
}

#[test]
fn partial_order_validates_ir_before_preparing_reductions() {
    let mut program = compile(&input(1));
    let rtlc::ir::Step::Assign { slot, .. } = program.next[0]
        .steps
        .iter_mut()
        .find(|step| matches!(step, rtlc::ir::Step::Assign { .. }))
        .unwrap()
    else {
        unreachable!()
    };
    *slot = 999;
    assert!(program.check_with_options(bounds(), options()).is_err());
}

#[test]
fn partial_order_preserves_type_errors_and_temporal_fallback() {
    let program = compile(&input(1).replace("flag0 = FALSE", "flag0 = 0"));
    let expected = program.check(bounds()).unwrap_err();
    let actual = program.check_with_options(bounds(), options()).unwrap_err();
    assert_eq!(
        (actual.code, actual.message),
        (expected.code, expected.message)
    );
    let mut program = compile(&input(1));
    program.properties.push(rtlc::ir::TemporalProperty {
        name: "P".into(),
        formula: rtlc::ir::TemporalFormula::Always {
            formula: Box::new(rtlc::ir::TemporalFormula::Atom {
                expression: rtlc::ir::Expression::Literal {
                    value: rtlc::ir::Scalar::Bool(true),
                },
            }),
        },
    });
    assert_eq!(
        program.check(bounds()).unwrap(),
        program.check_with_options(bounds(), options()).unwrap()
    );
}
