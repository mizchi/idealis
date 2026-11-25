use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

const BOUNDS: SafetyBounds = SafetyBounds {
    max_depth: 10,
    max_states: 1000,
    max_work: 10000000,
    max_temporal_nodes: None,
};

const VERSIONS: [IrVersion; 2] = [IrVersion::V1, IrVersion::V2];

fn check(source: &str, config: &str, version: IrVersion) -> Result<String, String> {
    let module = rtla::parse_module(source).unwrap();
    let config = rtla::parse_config(config).unwrap();
    let program = compile_with_options(&module, &config, CompileOptions { irVersion: version })
        .map_err(|error| error.to_string())?;
    program
        .check(BOUNDS)
        .map(|result| result.status.to_string())
        .map_err(|error| error.message)
}

fn model(init: &str, safe: &str) -> String {
    format!(
        "---- MODULE M ----\nEXTENDS Integers, FiniteSets\nVARIABLE f\nInit == f = {init}\nNext == f' = f\nSafe == {safe}\n===="
    )
}

fn assert_holds(init: &str, safe: &str) {
    for version in VERSIONS {
        assert_eq!(
            check(
                &model(init, safe),
                "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
                version
            ),
            Ok("complete".into()),
            "{version:?}: {safe}"
        );
    }
}

fn assert_fails(init: &str, safe: &str) {
    for version in VERSIONS {
        let result = check(
            &model(init, safe),
            "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
            version,
        );
        assert!(
            result == Err("Constant invariant or property is false".to_string()),
            "{version:?}: {safe}: {result:?}"
        );
    }
}

#[test]
fn binary_product_is_the_set_of_pairs() {
    assert_holds(
        "0",
        "{1, 2} \\X {10, 20} = {<<1, 10>>, <<1, 20>>, <<2, 10>>, <<2, 20>>}",
    );
    assert_fails("0", "{1, 2} \\X {10, 20} = {<<1, 10>>}");
}

#[test]
fn ternary_product_is_the_set_of_triples_not_nested_pairs() {
    assert_holds(
        "0",
        "{1, 2} \\X {3} \\X {4, 5} = {<<1, 3, 4>>, <<1, 3, 5>>, <<2, 3, 4>>, <<2, 3, 5>>}",
    );
    assert_fails("0", "<<<<1, 3>>, 4>> \\in {1} \\X {3} \\X {4}");
    assert_holds("0", "<<1, 3, 4>> \\in {1} \\X {3} \\X {4}");
}

#[test]
fn membership_tests_each_component() {
    assert_holds("0", "<<1, 20>> \\in {1, 2} \\X {10, 20}");
    assert_holds("0", "<<1, 30>> \\notin {1, 2} \\X {10, 20}");
    assert_holds("0", "<<3, 10>> \\notin {1, 2} \\X {10, 20}");
}

#[test]
fn cardinality_counts_the_pairs() {
    assert_holds("0", "Cardinality({1, 2, 3} \\X {10, 20}) = 6");
    assert_holds("0", "Cardinality((1..2) \\X (1..3) \\X (1..4)) = 24");
}

#[test]
fn product_is_a_quantifier_and_choice_domain() {
    assert_holds("0", "\\A p \\in {1, 2} \\X {10, 20} : p[1] < p[2]");
    assert_holds("0", "\\E p \\in {1, 2} \\X {10, 20} : p[1] + p[2] = 22");
    assert_fails("0", "\\E p \\in {1, 2} \\X {10, 20} : p[1] + p[2] = 23");
    assert_holds("0", "(CHOOSE p \\in {1} \\X {10} : TRUE) = <<1, 10>>");
}

#[test]
fn product_maps_and_filters_like_any_set() {
    assert_holds(
        "0",
        "{p[1] + p[2] : p \\in {1, 2} \\X {10, 20}} = {11, 21, 12, 22}",
    );
    assert_holds(
        "0",
        "{p \\in {1, 2} \\X {10, 20} : p[1] = 2} = {<<2, 10>>, <<2, 20>>}",
    );
}

#[test]
fn an_empty_operand_gives_the_empty_set() {
    assert_holds("0", "{} \\X {1, 2} = {}");
    assert_holds("0", "{1, 2} \\X {} = {}");
    assert_holds("0", "{1} \\X {} \\X {2} = {}");
    assert_holds("0", "Cardinality({1, 2} \\X {}) = 0");
}

#[test]
fn operands_may_be_comprehensions() {
    assert_holds(
        "0",
        "{x \\in 1..4 : x > 2} \\X {y + 1 : y \\in 1..2} = {<<3, 2>>, <<3, 3>>, <<4, 2>>, <<4, 3>>}",
    );
    assert_holds(
        "0",
        "{1, 2} \\X {y \\in 5..8 : y # 6} \\X {z * 2 : z \\in {1}} \
         = {<<a, b, 2>> : a \\in {1, 2}, b \\in {5, 7, 8}}",
    );
    assert_holds(
        "0",
        "Cardinality({x \\in 1..4 : x > 1} \\X {y \\in 1..3 : y < 3}) = 6",
    );
}

#[test]
fn caller_names_are_not_captured() {
    let source = "---- MODULE M ----\nEXTENDS Integers, FiniteSets\nVARIABLE f\n\
        Pairs(__Product_a, __Product_b) == {1, 2} \\X {__Product_a} \\X {__Product_b}\n\
        Init == f = 0\nNext == f' = f\n\
        Safe == Pairs(3, 4) = {<<1, 3, 4>>, <<2, 3, 4>>}\n====";
    for version in VERSIONS {
        assert_eq!(
            check(
                source,
                "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
                version
            ),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn product_in_a_next_state_relation() {
    let source = "---- MODULE M ----\nEXTENDS Integers\nVARIABLE x\n\
        Init == x = 0\n\
        Step == \\E p \\in {1, 2} \\X {10, 20} : x' = p[1] + p[2]\n\
        Next == Step\n\
        Safe == x \\in {0, 11, 21, 12, 22}\n====";
    for version in VERSIONS {
        assert_eq!(
            check(
                source,
                "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE",
                version
            ),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn quint_exported_product_forms_are_accepted() {
    let map = "---- MODULE Tuples ----\nEXTENDS Integers, FiniteSets\nVARIABLE x\n\
        init == x = {}\n\
        step == x' = { (p_23[1] + p_23[2]): p_23 \\in { 1, 2 } \\X { 10, 20 } }\n\
        Next == step\n\
        inv == Cardinality(x) <= 4\n====";
    let choice = "---- MODULE TuplesExists ----\nEXTENDS Integers\nVARIABLE x\n\
        init == x = 0\n\
        step == \\E p \\in { 1, 2 } \\X { 10, 20 }: x' = p[1] + p[2]\n\
        Next == step\n\
        inv == x <= 22\n====";
    for version in VERSIONS {
        for source in [map, choice] {
            assert_eq!(
                check(
                    source,
                    "INIT init\nNEXT Next\nINVARIANT inv\nCHECK_DEADLOCK FALSE",
                    version
                ),
                Ok("complete".into()),
                "{version:?}"
            );
        }
    }
}
