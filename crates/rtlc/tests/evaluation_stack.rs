mod stack_programs;

const DEFAULT_TEST_STACK: usize = 2 * 1024 * 1024;
const NESTING_LIMIT: &str = "error protocol: Safety evaluation nesting exceeds 128";

fn complete(work: u32) -> String {
    format!("complete reason=- states=1 generated=2 work={work}")
}

fn assert_boundary(kind: &str, accepted: usize, work: u32) {
    assert_eq!(
        stack_programs::on_stack(DEFAULT_TEST_STACK, stack_programs::program(kind, accepted)),
        complete(work),
        "{kind} {accepted}"
    );
    assert_eq!(
        stack_programs::on_stack(
            DEFAULT_TEST_STACK,
            stack_programs::program(kind, accepted + 1)
        ),
        NESTING_LIMIT,
        "{kind} {}",
        accepted + 1
    );
}

#[test]
fn expression_operators_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("operator", 121, 9);
}

#[test]
fn value_operators_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("value", 60, 133);
}

#[test]
fn calls_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("call", 127, 258);
}

#[test]
fn membership_sets_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("membership", 120, 15);
}

#[test]
fn quantifier_bindings_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("binding", 115, 242);
}

#[test]
fn enabled_steps_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("enabled", 119, 131);
}

#[test]
fn unread_binder_locals_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("bind-run", 115, 131);
}

#[test]
fn lazy_functions_reach_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("lazy", 60, 68);
}

#[test]
fn temporal_lowering_reaches_the_nesting_limit_within_the_default_test_stack() {
    assert_boundary("temporal", 122, 697_240);
}
