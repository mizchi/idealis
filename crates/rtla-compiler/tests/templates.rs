use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

#[test]
fn parameterized_actions_and_temporal_helpers_share_one_body_per_signature() {
    for (kind, definitions, next, property) in [
        (
            "action",
            "Step(v) == x'=v",
            "Step((x+1)%3) \\/ Step((x+1)%3)",
            "",
        ),
        (
            "temporal",
            "T(p) == []p\nLive == T(x<3) /\\ T(x<3)",
            "x'=(x+1)%3",
            "PROPERTY Live",
        ),
    ] {
        let module = rtla::parse_module(&format!("---- MODULE Templates ----\nEXTENDS Naturals\nVARIABLE x\n{definitions}\nInit == x=0\nNext == {next}\n====")).unwrap();
        let config = rtla::parse_config(&format!(
            "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n{property}"
        ))
        .unwrap();
        let program = compile_with_options(
            &module,
            &config,
            CompileOptions {
                irVersion: IrVersion::V2,
            },
        )
        .unwrap();
        let wire = serde_json::to_value(&program).unwrap();
        assert_eq!(
            wire["definitions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|definition| definition["bodyKind"] == kind)
                .count(),
            1,
            "{wire}"
        );
        let result = program
            .check(SafetyBounds {
                max_depth: 100,
                max_states: 100,
                max_work: 100_000,
                max_temporal_nodes: Some(10000),
            })
            .unwrap();
        assert_eq!(result.status, "complete");
        assert_eq!(result.states, 3);
    }
}
