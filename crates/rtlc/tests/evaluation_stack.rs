mod stack_programs;

const REQUIRED_STACK: usize = if cfg!(debug_assertions) {
    8 * 1024 * 1024
} else {
    2 * 1024 * 1024
};
const NESTING_LIMIT: &str = "error protocol: Safety evaluation nesting exceeds 256";

fn complete(work: u32) -> String {
    format!("complete reason=- states=1 generated=2 work={work}")
}

fn assert_boundary(kind: &str, accepted: usize, work: u32) {
    assert_eq!(
        stack_programs::on_stack(REQUIRED_STACK, stack_programs::program(kind, accepted)),
        complete(work),
        "{kind} {accepted}"
    );
    assert_eq!(
        stack_programs::on_stack(REQUIRED_STACK, stack_programs::program(kind, accepted + 1)),
        NESTING_LIMIT,
        "{kind} {}",
        accepted + 1
    );
}

#[test]
fn expression_operators_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("operator", 245, 13);
}

#[test]
fn value_operators_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("value", 122, 267);
}

#[test]
fn calls_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("call", 255, 514);
}

#[test]
fn membership_sets_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("membership", 243, 25);
}

#[test]
fn quantifier_bindings_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("binding", 231, 482);
}

#[test]
fn enabled_steps_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("enabled", 239, 259);
}

#[test]
fn unread_binder_locals_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("bind-run", 231, 259);
}

#[test]
fn lazy_functions_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("lazy", 122, 135);
}

#[test]
fn temporal_calls_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("temporal", 255, 775);
}

#[test]
fn forwarded_parameters_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("forward", 127, 512);
}

#[test]
fn nested_updates_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("update", 243, 512);
}

#[test]
fn function_applications_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("function", 62, 138);
}

#[test]
fn action_steps_reach_the_nesting_limit_within_the_required_stack() {
    assert_boundary("action", 255, 274);
}
