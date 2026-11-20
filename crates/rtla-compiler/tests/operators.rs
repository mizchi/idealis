use rtlc::SafetyBounds;

#[test]
fn standard_operators_and_quantified_fairness_match_the_fixed_corpus() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("operators-support.json")).unwrap();
    for case in cases {
        let module = rtla::parse_module(case["source"].as_str().unwrap()).unwrap();
        let config = rtla::parse_config(case["config"].as_str().unwrap()).unwrap();
        let modules: Vec<_> = case["modules"]
            .as_object()
            .unwrap()
            .values()
            .map(|source| rtla::parse_module(source.as_str().unwrap()).unwrap())
            .collect();
        let compiled = rtla_compiler::compile_modules(&module, &config, &modules);
        if case["expected"] == "unsupported" {
            assert!(compiled.is_err(), "{}", case["name"]);
            if modules.is_empty() {
                assert!(
                    rtla_compiler::compile(&module, &config).is_err(),
                    "{}: flat",
                    case["name"]
                );
            }
            continue;
        }
        let program = compiled.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
        let result = program.check(SafetyBounds {
            max_depth: 100,
            max_states: 1000,
            max_work: 1_000_000,
            max_temporal_nodes: Some(10000),
        });
        if case["expected"] == "model-error" {
            assert!(result.is_err(), "{}", case["name"]);
            continue;
        }
        assert_eq!(
            serde_json::to_value(result.unwrap()).unwrap()["status"],
            case["expected"],
            "{}",
            case["name"]
        );
    }
}

#[test]
fn quantified_fairness_expansion_does_not_hide_work_exhaustion() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("operators-support.json")).unwrap();
    let case = cases
        .iter()
        .find(|case| case["name"] == "fair-forall")
        .unwrap();
    let program = rtla_compiler::compile(
        &rtla::parse_module(case["source"].as_str().unwrap()).unwrap(),
        &rtla::parse_config(case["config"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let bounds = SafetyBounds {
        max_depth: 100,
        max_states: 1000,
        max_work: 1_000_000,
        max_temporal_nodes: Some(10000),
    };
    let full = program.check(bounds).unwrap();
    assert_eq!(full.status, "complete");
    let cut = program
        .check(SafetyBounds {
            max_work: full.work - 1,
            ..bounds
        })
        .unwrap();
    assert_eq!(cut.status, "bounded");
    assert_eq!(cut.reason.as_deref(), Some("work"));
    assert_eq!(cut.states, full.states);
}
