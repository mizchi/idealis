use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};

#[test]
fn invalid_labels_lambda_bindings_and_recursive_declarations_are_rejected_when_unused() {
    for declaration in [
        "Unused == \\A bound \\in {1}: label::TRUE",
        "Unused == \\A bound \\in {1}: label(other)::TRUE",
        "Unused == IF TRUE THEN label::1 ELSE label::2",
        "Unused == [<<1>> EXCEPT ![1]=label::@]",
        "Apply(F(_), value) == F(value)\nUnused == Apply(LAMBDA TRUE: TRUE, 1)",
        "RECURSIVE Unused(_,_)\nUnused(value) == value",
    ] {
        let module = rtla::parse_module(&format!("---- MODULE Test ----\nVARIABLE x\n{declaration}\nInit == x=0\nNext == UNCHANGED x\n====")).unwrap();
        let config = rtla::parse_config("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            assert!(
                compile_with_options(&module, &config, CompileOptions { irVersion: version })
                    .is_err(),
                "{version:?}: {declaration}"
            );
        }
    }
}

#[test]
fn nested_labels_and_function_and_lambda_parameters_have_their_own_label_scopes() {
    for declaration in [
        "Unused == \\A outer \\in {1}: first(outer):: \\A inner \\in {2}: second(inner)::TRUE",
        "Unused[bound \\in {1}] == label(bound)::bound",
        "Apply(F(_), value) == F(value)\nUnused == \\A outer \\in {1}: Apply(LAMBDA inner: label(outer,inner)::TRUE, 2)",
    ] {
        let module = rtla::parse_module(&format!("---- MODULE Test ----\nVARIABLE x\n{declaration}\nInit == x=0\nNext == UNCHANGED x\n====")).unwrap();
        let config = rtla::parse_config("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE").unwrap();
        for version in [IrVersion::V1, IrVersion::V2] {
            compile_with_options(&module, &config, CompileOptions { irVersion: version })
                .unwrap_or_else(|error| panic!("{declaration}: {error}"));
        }
    }
}

#[test]
fn sequence_symbol_collision_points_to_top_level_declaration() {
    let config = rtla::parse_config("INIT Init\nNEXT Next").unwrap();
    let source = "---- MODULE M ----\nEXTENDS Sequences\nLen == 1\nVARIABLE x\nInit == x=0\nNext == x'=Len\n====";
    for version in [IrVersion::V1, IrVersion::V2] {
        let compile = |text: &str| {
            compile_with_options(
                &rtla::parse_module(text).unwrap(),
                &config,
                CompileOptions { irVersion: version },
            )
        };
        let error = compile(source).unwrap_err();
        assert!(
            error.message.contains("Multiply-defined symbol 'Len'")
                && error.message.contains("Sequences")
                && error.message.contains("line 3, column 1"),
            "{error}"
        );
        compile(&source.replace("EXTENDS Sequences", "EXTENDS Naturals")).unwrap();
    }
}
