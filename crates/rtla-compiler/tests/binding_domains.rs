use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};

#[test]
fn nested_binders_in_a_later_domain_get_their_own_slots() {
    let module = rtla::parse_module(
        r#"---- MODULE BindingDomains ----
EXTENDS Integers
VARIABLE state
Init == state = 0
Next == state' = state
Safe == LET base == 7 IN
  {<<index, value>> : index \in {1}, value \in {inner : inner \in {base}}}
    = {<<1, 7>>}
===="#,
    )
    .unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();

    for version in [IrVersion::V2, IrVersion::V1] {
        let program =
            compile_with_options(&module, &config, CompileOptions { irVersion: version }).unwrap();
        let result = program
            .check(rtlc::SafetyBounds {
                max_depth: 10,
                max_states: 100,
                max_work: 100_000,
                max_temporal_nodes: None,
            })
            .unwrap_or_else(|error| panic!("{version:?}: {}", error.message));
        assert_eq!(result.status, "complete", "{version:?}");
        assert!(result.violation.is_none(), "{version:?}");
    }
}
