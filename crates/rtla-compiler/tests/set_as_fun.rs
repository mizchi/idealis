use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

const BOUNDS: SafetyBounds = SafetyBounds {
    max_depth: 10,
    max_states: 100,
    max_work: 1000000,
    max_temporal_nodes: None,
};

fn check(source: &str, version: IrVersion) -> Result<String, String> {
    let module = rtla::parse_module(source).unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE").unwrap();
    let program = compile_with_options(&module, &config, CompileOptions { irVersion: version })
        .map_err(|error| error.to_string())?;
    program
        .check(BOUNDS)
        .map(|result| result.status.to_string())
        .map_err(|error| error.message)
}

fn model(extends: &str, init: &str, safe: &str, extra: &str) -> String {
    format!(
        "---- MODULE M ----\nEXTENDS {extends}\nVARIABLE f\n{extra}\nInit == f = {init}\nNext == f' = f\nSafe == {safe}\n===="
    )
}

const VERSIONS: [IrVersion; 2] = [IrVersion::V1, IrVersion::V2];

#[test]
fn empty_set_as_fun_is_the_function_with_an_empty_domain() {
    for version in VERSIONS {
        let source = model(
            "Integers, FiniteSets, Apalache",
            "SetAsFun({})",
            "DOMAIN f = {} /\\ Cardinality(DOMAIN f) = 0",
            "",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn literal_pairs_become_a_function_with_the_first_components_as_domain() {
    for version in VERSIONS {
        let source = model(
            "Integers, Apalache",
            "SetAsFun({<<1, 10>>, <<2, 20>>})",
            "DOMAIN f = {1, 2} /\\ f[1] = 10 /\\ f[2] = 20",
            "",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn computed_pair_sets_and_string_keys_are_accepted() {
    for version in VERSIONS {
        let source = model(
            "Integers, Apalache",
            "SetAsFun({<<x, x + 1>> : x \\in 1..3})",
            "DOMAIN f = 1..3 /\\ f[3] = 4",
            "",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
        let source = model(
            "Apalache",
            "SetAsFun({<<\"a\", TRUE>>, <<\"b\", FALSE>>})",
            "DOMAIN f = {\"a\", \"b\"} /\\ f[\"a\"] /\\ ~f[\"b\"]",
            "",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn nested_set_as_fun_keeps_inner_bindings_apart() {
    for version in VERSIONS {
        let source = model(
            "Integers, Apalache",
            "SetAsFun({<<k, SetAsFun({<<j, k + j>> : j \\in 1..2})>> : k \\in 1..2})",
            "f[2][1] = 3 /\\ f[1][2] = 3",
            "",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn the_same_pair_listed_twice_is_one_entry() {
    for version in VERSIONS {
        let source = model(
            "Integers, Apalache",
            "SetAsFun({<<1, 10>>, <<1, 10>>})",
            "DOMAIN f = {1} /\\ f[1] = 10",
            "",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn a_key_with_two_values_is_refused_instead_of_picking_one() {
    for version in VERSIONS {
        let source = model(
            "Integers, Apalache",
            "SetAsFun({<<1, 10>>, <<1, 20>>})",
            "TRUE",
            "",
        );
        let error = check(&source, version).unwrap_err();
        assert!(
            error.contains("SetAsFun: a key has more than one value"),
            "{version:?}: {error}"
        );
    }
}

#[test]
fn set_as_fun_inside_an_operator_with_an_operator_parameter_is_accepted() {
    for version in VERSIONS {
        let source = model(
            "Integers, Apalache",
            "SetAsFun({<<1, Update(LAMBDA n: n + 9, 1)>>})",
            "f[1] = 10",
            "Update(G(_), x) == G(x)",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}

#[test]
fn set_as_fun_requires_extends_apalache() {
    for version in VERSIONS {
        let source = model("Integers", "SetAsFun({})", "TRUE", "");
        assert!(check(&source, version).is_err(), "{version:?}");
    }
}

#[test]
fn caller_names_that_look_like_generated_binders_are_not_captured() {
    for version in VERSIONS {
        for name in [
            "__SetAsFun_left",
            "__SetAsFun_right",
            "__SetAsFun_key",
            "__SetAsFun_entry",
            "__SetAsFun_match",
        ] {
            let source = model(
                "Integers, Apalache",
                "Make({<<1, 10>>, <<2, 20>>})",
                "DOMAIN f = {1, 2} /\\ f[1] = 10 /\\ f[2] = 20",
                &format!("Make({name}) == SetAsFun({name})"),
            );
            assert_eq!(
                check(&source, version),
                Ok("complete".into()),
                "{version:?} {name}"
            );
        }
    }
}

#[test]
fn set_as_fun_can_be_passed_as_an_operator_argument() {
    for version in VERSIONS {
        let source = model(
            "Integers, Apalache",
            "Use(SetAsFun, {<<1, 10>>})",
            "DOMAIN f = {1} /\\ f[1] = 10",
            "Use(F(_), s) == F(s)",
        );
        assert_eq!(
            check(&source, version),
            Ok("complete".into()),
            "{version:?}"
        );
    }
}
