use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;

const BOUNDS: SafetyBounds = SafetyBounds {
    max_depth: 10,
    max_states: 100,
    max_work: 1000000,
    max_temporal_nodes: None,
};

const VERSIONS: [IrVersion; 2] = [IrVersion::V1, IrVersion::V2];

const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Safe\nCHECK_DEADLOCK FALSE";

const QUINT_CONFIG: &str = "INIT init\nNEXT step\nINVARIANT inv\nCHECK_DEADLOCK FALSE";

const VIOLATED: &str = "Constant invariant or property is false";

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

fn model_with(extends: &str, init: &str, safe: &str, extra: &str) -> String {
    format!(
        "---- MODULE M ----\nEXTENDS {extends}\nVARIABLE f\n{extra}\nInit == f = {init}\nNext == f' = f\nSafe == {safe}\n===="
    )
}

fn model(init: &str, safe: &str, extra: &str) -> String {
    model_with("Integers, Sequences, Apalache", init, safe, extra)
}

fn assert_outcome(source: &str, config: &str, expected: Result<&str, &str>) {
    let expected = expected.map(String::from).map_err(String::from);
    for version in VERSIONS {
        assert_eq!(
            check(source, config, version),
            expected,
            "{version:?}: {source}"
        );
    }
}

fn assert_holds(init: &str, safe: &str, extra: &str) {
    assert_outcome(&model(init, safe, extra), CONFIG, Ok("complete"));
}

fn assert_fails(init: &str, safe: &str, extra: &str) {
    assert_outcome(&model(init, safe, extra), CONFIG, Err(VIOLATED));
}

fn assert_refused(source: &str, expected: &str) {
    for version in VERSIONS {
        let error = check(source, CONFIG, version).unwrap_err();
        assert!(error.contains(expected), "{version:?}: {error}");
    }
}

fn quint_range_fold(first: i32, last: i32, inv: &str) -> String {
    format!(
        "---------------------------- MODULE RangeFold ----------------------------\n\
         EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants\n\
         VARIABLE n\n\
         init ==\n\
         \x20 n\n\
         \x20   = (LET\n\
         \x20   __QUINT_LAMBDA1(acc_12, i_12) == acc_12 + i_12\n\
         \x20   IN\n\
         \x20   ApaFoldSeqLeft(__QUINT_LAMBDA1, 0, ((LET\n\
         \x20   __QUINT_LAMBDA0(__quint_var0) == ({first} + __quint_var0) - 1\n\
         \x20   IN\n\
         \x20   MkSeq(({last} - {first}), __QUINT_LAMBDA0)))))\n\
         step == n' = n\n\
         inv == {inv}\n\
         ============================================================================"
    )
}

#[test]
fn quint_range_fold_sums_the_range() {
    assert_outcome(
        &quint_range_fold(0, 3, "n = 3"),
        QUINT_CONFIG,
        Ok("complete"),
    );
    assert_outcome(
        &quint_range_fold(2, 5, "n = 9"),
        QUINT_CONFIG,
        Ok("complete"),
    );
    assert_outcome(
        &quint_range_fold(0, 3, "n = 4"),
        QUINT_CONFIG,
        Ok("invariant-violation"),
    );
}

#[test]
fn quint_range_fold_over_an_empty_or_reversed_range_is_the_initial_value() {
    assert_outcome(
        &quint_range_fold(3, 3, "n = 0"),
        QUINT_CONFIG,
        Ok("complete"),
    );
    assert_outcome(
        &quint_range_fold(5, 3, "n = 0"),
        QUINT_CONFIG,
        Ok("complete"),
    );
}

#[test]
fn quint_range_fold_with_bounds_taken_from_operator_parameters() {
    let source = "---- MODULE M ----\nEXTENDS Integers, Sequences, Apalache\nVARIABLE n\n\
        Total(a, b) == LET\n\
        \x20 Sum(acc, i) == acc + i\n\
        \x20 IN ApaFoldSeqLeft(Sum, 0, (LET\n\
        \x20 Index(v) == (a + v) - 1\n\
        \x20 IN MkSeq((b - a), Index)))\n\
        init == n = Total(1, 5)\nstep == n' = n\ninv == n = 10\n====";
    assert_outcome(source, QUINT_CONFIG, Ok("complete"));
}

#[test]
fn elements_are_the_operator_applied_to_one_through_n() {
    assert_holds("0", "MkSeq(3, LAMBDA i: i * i) = <<1, 4, 9>>", "");
    assert_fails("0", "MkSeq(3, LAMBDA i: i * i) = <<1, 4, 8>>", "");
    assert_holds(
        "0",
        "MkSeq(2, LAMBDA i: IF i = 1 THEN \"a\" ELSE \"b\") = <<\"a\", \"b\">>",
        "",
    );
    assert_holds("0", "MkSeq(3, LAMBDA i: <<i, i + 1>>)[2] = <<2, 3>>", "");
}

#[test]
fn the_result_is_a_sequence() {
    assert_holds("0", "Len(MkSeq(4, LAMBDA i: i)) = 4", "");
    assert_holds("0", "Head(MkSeq(3, LAMBDA i: i + 5)) = 6", "");
    assert_holds("0", "Tail(MkSeq(3, LAMBDA i: i)) = <<2, 3>>", "");
    assert_holds("0", "Append(MkSeq(2, LAMBDA i: i), 9) = <<1, 2, 9>>", "");
    assert_holds(
        "0",
        "MkSeq(2, LAMBDA i: i) \\o MkSeq(2, LAMBDA i: i + 10) = <<1, 2, 11, 12>>",
        "",
    );
    assert_holds("0", "SubSeq(MkSeq(5, LAMBDA i: i), 2, 4) = <<2, 3, 4>>", "");
    assert_holds("0", "MkSeq(3, LAMBDA i: i) \\in Seq(1..3)", "");
    assert_holds("0", "DOMAIN MkSeq(3, LAMBDA i: i) = 1..3", "");
    assert_holds(
        "0",
        "MkSeq(3, LAMBDA i: 2 * i) = [i \\in 1..3 |-> 2 * i]",
        "",
    );
}

#[test]
fn a_length_of_zero_or_less_is_the_empty_sequence() {
    assert_holds("0", "MkSeq(0, LAMBDA i: i) = <<>>", "");
    assert_holds("0", "Len(MkSeq(0, LAMBDA i: i)) = 0", "");
    assert_holds("0", "MkSeq(-3, LAMBDA i: i) = <<>>", "");
    assert_holds("0", "MkSeq(2 - 5, LAMBDA i: i) = <<>>", "");
    assert_fails("0", "MkSeq(0, LAMBDA i: i) = <<1>>", "");
    assert_holds(
        "0",
        "ApaFoldSeqLeft(Add, 7, MkSeq(0, LAMBDA i: i)) = 7",
        "Add(acc, x) == acc + x",
    );
}

#[test]
fn the_operator_is_not_applied_when_the_length_is_not_positive() {
    assert_holds("0", "MkSeq(0, LAMBDA i: Head(<<>>)) = <<>>", "");
    assert_holds("0", "MkSeq(-1, LAMBDA i: Head(<<>>)) = <<>>", "");
}

#[test]
fn a_named_top_level_operator_is_accepted() {
    assert_holds("0", "MkSeq(3, Square) = <<1, 4, 9>>", "Square(i) == i * i");
}

#[test]
fn a_let_defined_operator_that_reads_the_caller_is_accepted() {
    assert_holds(
        "0",
        "Offsets(2, 5) = <<2, 3, 4>>",
        "Offsets(a, b) == LET Shift(v) == (a + v) - 1 IN MkSeq(b - a, Shift)",
    );
    assert_fails(
        "0",
        "Offsets(2, 5) = <<2, 3, 5>>",
        "Offsets(a, b) == LET Shift(v) == (a + v) - 1 IN MkSeq(b - a, Shift)",
    );
}

#[test]
fn an_operator_parameter_can_be_forwarded() {
    assert_holds(
        "0",
        "Forward(LAMBDA k: k + 1, 2) = <<2, 3>> /\\ Forward(Double, 3) = <<2, 4, 6>>",
        "Forward(G(_), n) == MkSeq(n, G)\nDouble(k) == 2 * k",
    );
}

#[test]
fn nested_sequences_keep_their_indexes_apart() {
    assert_holds(
        "0",
        "MkSeq(2, LAMBDA i: MkSeq(i, LAMBDA j: 10 * i + j)) = <<<<11>>, <<21, 22>>>>",
        "",
    );
}

#[test]
fn the_sequence_can_be_the_value_of_a_state_variable() {
    let source = "---- MODULE M ----\nEXTENDS Integers, Sequences, Apalache\nVARIABLE s\n\
        Init == s = <<>>\n\
        Next == s' = IF Len(s) < 3 THEN MkSeq(Len(s) + 1, LAMBDA i: 2 * i) ELSE s\n\
        Safe == Len(s) <= 3 /\\ s = MkSeq(Len(s), LAMBDA i: 2 * i)\n====";
    for version in VERSIONS {
        let program = rtla_compiler::compile_with_options(
            &rtla::parse_module(source).unwrap(),
            &rtla::parse_config(CONFIG).unwrap(),
            CompileOptions { irVersion: version },
        )
        .unwrap();
        let result = program.check(BOUNDS).unwrap();
        assert_eq!(result.status.to_string(), "complete", "{version:?}");
        assert_eq!(result.states, 4, "{version:?}");
    }
}

#[test]
fn the_operator_may_read_state_variables() {
    let source = "---- MODULE M ----\nEXTENDS Integers, Sequences, Apalache\nVARIABLE x\n\
        Init == x = 1\n\
        Next == x' = IF x < 3 THEN x + 1 ELSE x\n\
        Safe == MkSeq(2, LAMBDA i: i + x) = <<1 + x, 2 + x>>\n====";
    assert_outcome(source, CONFIG, Ok("complete"));
}

#[test]
fn caller_names_that_look_like_the_generated_binder_are_not_captured() {
    for name in ["__MkSeq_index", "i", "v", "__quint_var0"] {
        assert_holds(
            "0",
            "Make(3) = <<3, 6, 9>>",
            &format!("Make({name}) == MkSeq({name}, LAMBDA k: k * {name})"),
        );
        assert_holds(
            "0",
            "Make(2) = <<3, 4>>",
            &format!("Make({name}) == LET Shift(k) == k + {name} IN MkSeq({name}, Shift)"),
        );
        assert_holds(
            "0",
            "Make(LAMBDA k: k + 5) = <<6, 7>>",
            &format!("Make({name}(_)) == MkSeq(2, {name})"),
        );
    }
}

#[test]
fn an_enclosing_binding_with_the_generated_name_is_not_captured() {
    assert_holds(
        "0",
        "\\A __MkSeq_index \\in 1..2 : \
         MkSeq(2, LAMBDA k: k + __MkSeq_index) = <<1 + __MkSeq_index, 2 + __MkSeq_index>>",
        "",
    );
    assert_holds(
        "0",
        "LET __MkSeq_index == 10 IN MkSeq(2, LAMBDA k: k + __MkSeq_index) = <<11, 12>>",
        "",
    );
    assert_holds(
        "0",
        "MkSeq(2, LAMBDA __MkSeq_index: MkSeq(2, LAMBDA k: k + __MkSeq_index)) \
         = <<<<2, 3>>, <<3, 4>>>>",
        "",
    );
    assert_holds(
        "0",
        "MkSeq(2, __MkSeq_index) = <<2, 3>>",
        "__MkSeq_index(k) == k + 1",
    );
}

#[test]
fn a_lambda_parameter_that_matches_a_caller_name_is_not_captured() {
    assert_holds(
        "0",
        "Make(10) = <<11, 12, 13>>",
        "Make(v) == LET Shift(__quint_var0) == __quint_var0 + v IN MkSeq(3, Shift)",
    );
    assert_holds(
        "0",
        "Make(10) = <<11, 12, 13>>",
        "Make(__quint_var0) == LET Shift(v) == v + __quint_var0 IN MkSeq(3, Shift)",
    );
}

#[test]
fn mk_seq_requires_extends_apalache() {
    let source = model_with(
        "Integers, Sequences",
        "0",
        "MkSeq(2, LAMBDA i: i) = <<1, 2>>",
        "",
    );
    for version in VERSIONS {
        assert!(check(&source, CONFIG, version).is_err(), "{version:?}");
    }
}

#[test]
fn a_module_that_cannot_write_a_range_is_refused() {
    let source = model_with("Apalache", "MkSeq(2, Identity)", "TRUE", "Identity(i) == i");
    assert_refused(&source, "range requires Naturals");
}

#[test]
fn a_user_definition_named_mk_seq_is_not_replaced() {
    let source = model_with(
        "Integers, Sequences",
        "0",
        "MkSeq(2, LAMBDA i: i) = <<101, 102>>",
        "MkSeq(n, G(_)) == [k \\in 1..n |-> G(k) + 100]",
    );
    assert_outcome(&source, CONFIG, Ok("complete"));
}

#[test]
fn a_wrong_number_of_arguments_is_refused() {
    assert_refused(
        &model("0", "MkSeq(2) = <<1, 2>>", ""),
        "operator arity MkSeq",
    );
    assert_refused(
        &model("0", "MkSeq(2, LAMBDA i: i, 3) = <<1, 2>>", ""),
        "operator arity MkSeq",
    );
}

#[test]
fn a_second_argument_that_is_not_an_operator_is_refused() {
    assert_refused(
        &model("0", "MkSeq(2, 5) = <<1, 2>>", ""),
        "MkSeq operator argument",
    );
}

#[test]
fn an_operator_that_does_not_take_one_argument_is_refused() {
    assert_refused(
        &model("0", "MkSeq(2, Pair) = <<1, 2>>", "Pair(a, b) == a + b"),
        "operator Pair",
    );
}

#[test]
fn a_length_that_is_not_an_integer_is_an_evaluation_error() {
    assert_refused(
        &model("0", "MkSeq(\"two\", LAMBDA i: i) = <<>>", ""),
        "Expected an integer",
    );
}

fn check_modules(root: &str, library: &str) -> Result<String, String> {
    let program = rtla_compiler::compile_modules(
        &rtla::parse_module(root).unwrap(),
        &rtla::parse_config(CONFIG).unwrap(),
        &[rtla::parse_module(library).unwrap()],
    )
    .map_err(|error| error.to_string())?;
    program
        .check(BOUNDS)
        .map(|result| result.status.to_string())
        .map_err(|error| error.message)
}

#[test]
fn a_defining_module_that_extends_apalache_may_use_mk_seq() {
    let library = "---- MODULE Ranges ----\nEXTENDS Integers, Apalache\n\
        Range(a, b) == LET Shift(v) == (a + v) - 1 IN MkSeq(b - a, Shift)\n====";
    let root = "---- MODULE M ----\nEXTENDS Integers, Sequences, Ranges\nVARIABLE f\n\
        Init == f = Range(2, 5)\nNext == f' = f\nSafe == f = <<2, 3, 4>>\n====";
    assert_eq!(check_modules(root, library), Ok("complete".into()));
}

#[test]
fn a_defining_module_that_does_not_extend_apalache_may_not_use_mk_seq() {
    let library = "---- MODULE Ranges ----\nEXTENDS Integers\n\
        Range(a, b) == LET Shift(v) == (a + v) - 1 IN MkSeq(b - a, Shift)\n====";
    let root = "---- MODULE M ----\nEXTENDS Integers, Sequences, Apalache, Ranges\nVARIABLE f\n\
        Init == f = Range(2, 5)\nNext == f' = f\nSafe == f = <<2, 3, 4>>\n====";
    assert_eq!(
        check_modules(root, library),
        Err("Unsupported TLA input: operator MkSeq is not imported in its defining module".into())
    );
}
