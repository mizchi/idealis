use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let golden: Value = serde_json::from_str(include_str!(
        "../../../fixtures/tlc-operator-sharing/expected-step3f-b20.json"
    ))
    .unwrap();
    golden["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("{name}: no recorded TLC result"))
        .clone()
}

fn run(source: &str, config: &str, ir_version: IrVersion) -> Result<Value, String> {
    let module = rtla::parse_module(source).unwrap();
    let config = rtla::parse_config(config).unwrap();
    let compiled = compile_with_options(
        &module,
        &config,
        CompileOptions {
            irVersion: ir_version,
        },
    )
    .map_err(|error| error.message)?;
    let result = compiled
        .check(SafetyBounds {
            max_depth: 64,
            max_states: 10000,
            max_work: 1000000,
            max_temporal_nodes: None,
        })
        .map_err(|error| error.message)?;
    Ok(serde_json::to_value(result).unwrap())
}

fn assert_oracle(case: &str, outcome: Result<Value, String>) {
    let expected = fixture(case);
    let result = outcome.unwrap_or_else(|message| {
        panic!("{case}: expected a verdict, got the refusal {message:?}")
    });
    let status = expected["status"].as_str().unwrap();
    if status == "success" {
        assert_eq!(result["status"], "complete", "{case}: {result}");
        assert_eq!(result["states"], expected["states"], "{case}");
        assert_eq!(result["generated"], expected["generated"], "{case}");
        assert!(result["violation"].is_null(), "{case}: {result}");
        return;
    }
    assert_eq!(result["status"], status, "{case}: {result}");
    assert!(result["reason"].is_null(), "{case}: {result}");
    if status == "invariant-violation" {
        assert_eq!(result["depth"], expected["depth"], "{case}");
        assert_eq!(result["states"], expected["states"], "{case}");
        assert_eq!(result["generated"], expected["generated"], "{case}");
    }
    assert_eq!(result["violation"], expected["violation"], "{case}");
}

macro_rules! framed {
    ($v1:ident, $v2:ident, $case:literal) => {
        #[test]
        fn $v1() {
            assert_oracle(
                $case,
                run(
                    include_str!(concat!(
                        "../../../fixtures/tlc-operator-sharing/",
                        $case,
                        "/Differential.tla"
                    )),
                    include_str!(concat!(
                        "../../../fixtures/tlc-operator-sharing/",
                        $case,
                        "/Differential.cfg"
                    )),
                    IrVersion::V1,
                ),
            );
        }

        #[test]
        fn $v2() {
            assert_oracle(
                $case,
                run(
                    include_str!(concat!(
                        "../../../fixtures/tlc-operator-sharing/",
                        $case,
                        "/Differential.tla"
                    )),
                    include_str!(concat!(
                        "../../../fixtures/tlc-operator-sharing/",
                        $case,
                        "/Differential.cfg"
                    )),
                    IrVersion::V2,
                ),
            );
        }
    };
}

framed!(
    v1_framed_negation,
    v2_framed_negation,
    "sharing-framed-negation-step3f-b20"
);
framed!(
    v1_framed_negation_with_constraint,
    v2_framed_negation_with_constraint,
    "sharing-framed-negation-constraint-step3f-b20"
);
framed!(
    v1_framed_conjunction,
    v2_framed_conjunction,
    "sharing-framed-conjunction-step3f-b20"
);
framed!(
    v1_framed_conjunction_with_constraint,
    v2_framed_conjunction_with_constraint,
    "sharing-framed-conjunction-constraint-step3f-b20"
);
framed!(
    v1_unframed_negation_with_constraint,
    v2_unframed_negation_with_constraint,
    "sharing-unframed-negation-constraint-step3f-b20"
);
framed!(
    v1_unframed_conjunction_with_constraint,
    v2_unframed_conjunction_with_constraint,
    "sharing-unframed-conjunction-constraint-step3f-b20"
);
framed!(
    v1_framed_forall,
    v2_framed_forall,
    "sharing-framed-forall-step3f-b20"
);
framed!(
    v1_framed_forall_negated,
    v2_framed_forall_negated,
    "sharing-framed-forall-negated-step3f-b20"
);
framed!(
    v1_framed_exists_negated,
    v2_framed_exists_negated,
    "sharing-framed-exists-negated-step3f-b20"
);
