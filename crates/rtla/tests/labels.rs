use rtla::{ErrorKind, parse_expression, parse_module};

#[test]
fn labels_cannot_change_the_enclosing_operator_precedence() {
    for source in [
        "a * lab :: b + c",
        "a + lab :: b + c",
        "a * lab :: (b) + (c)",
        "- lab :: a + b",
        "[] lab :: x'",
        r"a + lab :: b /\ c",
        r"a /\ lab :: b /\ c",
        r"~ lab :: a \/ b",
        r"a * lab :: b \/ c",
    ] {
        assert_eq!(
            parse_expression(source).unwrap_err().kind,
            ErrorKind::Syntax,
            "{source}"
        );
    }
    assert_eq!(
        parse_module("---- MODULE Test ----\nop == a * lab :: b + c\n====")
            .unwrap_err()
            .kind,
        ErrorKind::Syntax
    );
}

#[test]
fn labels_keep_higher_precedence_and_explicit_parentheses() {
    for source in [
        "a + lab :: b * c",
        "a * lab :: b",
        "a * (lab :: b + c)",
        "a * lab :: (b + c)",
        "- (lab :: a + b)",
        "[] lab :: (x')",
        "a * outer :: inner :: b + c",
        r"a + lab :: B \X C",
        r"a => lab :: b /\ c",
        r"a + lab :: (b /\ c)",
        "a + lab ::\n  /\\ b\n  /\\ c",
    ] {
        assert!(parse_expression(source).is_ok(), "{source}");
    }
}
