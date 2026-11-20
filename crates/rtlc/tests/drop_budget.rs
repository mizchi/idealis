use rtlc::ir::{
    Binding, BodyKind, Branch, Definition, Domain, Expression, FoldOrder, Scalar, Step,
    TemporalFormula, TemporalProperty, Update,
};
use rtlc::{SafetyProgram, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

const STACK_BYTES: usize = 8 * 1024 * 1024;
const ORDINARY_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Counts {
    blocks: isize,
    bytes: isize,
    events: usize,
}

thread_local! {
    static COUNTS: Cell<Counts> = const {
        Cell::new(Counts { blocks: 0, bytes: 0, events: 0 })
    };
}

fn record(blocks: isize, bytes: isize, event: bool) {
    let _ = COUNTS.try_with(|cell| {
        let counts = cell.get();
        cell.set(Counts {
            blocks: counts.blocks + blocks,
            bytes: counts.bytes + bytes,
            events: counts.events + usize::from(event),
        });
    });
}

fn counts() -> Counts {
    COUNTS.with(Cell::get)
}

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(1, layout.size() as isize, true);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(-1, -(layout.size() as isize), false);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            record(0, new_size as isize - layout.size() as isize, true);
        }
        resized
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

struct Measurement {
    before: Counts,
    built: Counts,
    after: Counts,
}

impl Measurement {
    fn built_blocks(&self) -> isize {
        self.built.blocks - self.before.blocks
    }

    fn remaining_blocks(&self) -> isize {
        self.after.blocks - self.before.blocks
    }

    fn remaining_bytes(&self) -> isize {
        self.after.bytes - self.before.bytes
    }

    fn drop_events(&self) -> usize {
        self.after.events - self.built.events
    }
}

fn measure<T: 'static>(
    stack_bytes: usize,
    depth: usize,
    build: fn(usize) -> T,
    dispose: fn(T),
) -> Measurement {
    std::thread::Builder::new()
        .stack_size(stack_bytes)
        .spawn(move || {
            let before = counts();
            let value = build(depth);
            let built = counts();
            dispose(value);
            let after = counts();
            Measurement {
                before,
                built,
                after,
            }
        })
        .unwrap()
        .join()
        .expect("the value is built and disposed without panicking")
}

fn assert_drop_allocations<T: 'static>(case: usize, build: fn(usize) -> T, expected: usize) {
    let run = measure(STACK_BYTES, case, build, drop);
    assert!(run.built_blocks() > 0, "the counter sees the construction");
    assert_eq!(
        run.drop_events(),
        expected,
        "allocations during the drop for case {case}"
    );
    assert_eq!(run.remaining_blocks(), 0, "live blocks for case {case}");
    assert_eq!(run.remaining_bytes(), 0, "live bytes for case {case}");
}

fn truth() -> Expression {
    Expression::Literal {
        value: Scalar::Bool(true),
    }
}

fn atom() -> TemporalFormula {
    TemporalFormula::Atom {
        expression: truth(),
    }
}

fn small_value() -> Value {
    Value::List(vec![Value::Bool(true)])
}

fn small_expression() -> Expression {
    Expression::Not {
        argument: Box::new(truth()),
    }
}

fn boxed(expression: Expression) -> Box<Expression> {
    Box::new(expression)
}

fn binding(domain: Domain) -> Binding {
    Binding {
        tuple_size: None,
        slot: 0,
        domain,
    }
}

fn guard_branch(steps: Vec<Step>) -> Branch {
    Branch {
        name: "Step".into(),
        steps,
    }
}

fn wrap_expression(level: usize, inner: Expression) -> Expression {
    match level % 24 {
        0 => Expression::Frame {
            arguments: vec![],
            captures: vec![],
            parameter_count: 0,
            capture_count: 0,
            local_scope: 0,
            body: boxed(inner),
        },
        1 => Expression::Frame {
            arguments: vec![inner],
            captures: vec![],
            parameter_count: 1,
            capture_count: 0,
            local_scope: 0,
            body: boxed(truth()),
        },
        2 => Expression::Frame {
            arguments: vec![],
            captures: vec![truth(), inner],
            parameter_count: 0,
            capture_count: 2,
            local_scope: 0,
            body: boxed(truth()),
        },
        3 => Expression::Call {
            definition: 0,
            arguments: vec![inner],
            captures: vec![truth()],
        },
        4 => Expression::Call {
            definition: 0,
            arguments: vec![truth()],
            captures: vec![inner],
        },
        5 => Expression::Prime {
            argument: boxed(inner),
        },
        6 => Expression::Add {
            left: boxed(inner),
            right: boxed(truth()),
        },
        7 => Expression::If {
            condition: boxed(truth()),
            consequent: boxed(truth()),
            alternative: boxed(inner),
        },
        8 => Expression::Subseq {
            sequence: boxed(truth()),
            from: boxed(truth()),
            to: boxed(inner),
        },
        9 => Expression::Set {
            values: vec![truth(), inner],
        },
        10 => Expression::Tuple {
            values: vec![inner, truth()],
        },
        11 => Expression::Record {
            fields: vec![("field".into(), inner)],
        },
        12 => Expression::RecordSet {
            fields: vec![("field".into(), truth()), ("other".into(), inner)],
        },
        13 => Expression::At {
            target: boxed(inner),
            key: boxed(truth()),
        },
        14 => Expression::Update {
            target: boxed(truth()),
            updates: vec![Update {
                path: vec![inner],
                value: truth(),
            }],
        },
        15 => Expression::Update {
            target: boxed(truth()),
            updates: vec![Update {
                path: vec![],
                value: inner,
            }],
        },
        16 => Expression::Forall {
            bounds: vec![binding(Domain::Value {
                value: boxed(inner),
            })],
            body: boxed(truth()),
        },
        17 => Expression::Exists {
            bounds: vec![binding(Domain::Range {
                lower: boxed(inner),
                upper: boxed(truth()),
            })],
            body: boxed(truth()),
        },
        18 => Expression::Choose {
            binding: binding(Domain::Set {
                values: vec![inner],
            }),
            body: boxed(truth()),
        },
        19 => Expression::Fold {
            accumulator: 0,
            initial: boxed(truth()),
            collection: boxed(truth()),
            order: FoldOrder::SequenceLeft,
            body: boxed(inner),
        },
        20 => Expression::In {
            value: boxed(truth()),
            domain: Domain::Value {
                value: boxed(inner),
            },
        },
        21 => Expression::Enabled {
            branches: vec![],
            subscript: Some(boxed(inner)),
        },
        22 => Expression::Enabled {
            branches: vec![guard_branch(vec![
                Step::Bind {
                    slot: 0,
                    domain: Domain::Set {
                        values: vec![truth()],
                    },
                    tuple_size: None,
                },
                Step::Assign {
                    slot: 0,
                    value: inner,
                },
            ])],
            subscript: None,
        },
        _ => Expression::Enabled {
            branches: vec![guard_branch(vec![Step::Frame {
                arguments: vec![],
                captures: vec![],
                parameter_count: 0,
                capture_count: 0,
                local_scope: 0,
                branches: vec![guard_branch(vec![Step::Guard { expression: inner }])],
            }])],
            subscript: None,
        },
    }
}

fn expression_cycle(depth: usize) -> Expression {
    let base = Expression::Data {
        value: small_value(),
    };
    (0..depth).fold(base, |inner, level| wrap_expression(level, inner))
}

fn frame_chain(depth: usize) -> Vec<Branch> {
    let leaf = vec![guard_branch(vec![Step::Guard {
        expression: truth(),
    }])];
    (0..depth).fold(leaf, |branches, _| {
        vec![Branch {
            name: "Nested".into(),
            steps: vec![Step::Frame {
                arguments: vec![],
                captures: vec![],
                parameter_count: 0,
                capture_count: 0,
                local_scope: 0,
                branches,
            }],
        }]
    })
}

fn wrap_formula(level: usize, inner: TemporalFormula) -> TemporalFormula {
    let sibling = || Box::new(atom());
    match level % 9 {
        0 => TemporalFormula::Frame {
            arguments: vec![truth()],
            captures: vec![truth()],
            parameter_count: 1,
            capture_count: 1,
            local_scope: 0,
            formula: Box::new(inner),
        },
        1 => TemporalFormula::Not {
            formula: Box::new(inner),
        },
        2 => TemporalFormula::Always {
            formula: Box::new(inner),
        },
        3 => TemporalFormula::Eventually {
            formula: Box::new(inner),
        },
        4 => TemporalFormula::And {
            left: Box::new(inner),
            right: sibling(),
        },
        5 => TemporalFormula::And {
            left: sibling(),
            right: Box::new(inner),
        },
        6 => TemporalFormula::Or {
            left: Box::new(inner),
            right: sibling(),
        },
        7 => TemporalFormula::Forall {
            bounds: vec![binding(Domain::Value {
                value: boxed(truth()),
            })],
            formula: Box::new(inner),
        },
        _ => TemporalFormula::Exists {
            bounds: vec![binding(Domain::Set {
                values: vec![truth()],
            })],
            formula: Box::new(inner),
        },
    }
}

fn formula_cycle(depth: usize) -> TemporalFormula {
    (0..depth).fold(
        TemporalFormula::Atom {
            expression: small_expression(),
        },
        |inner, level| wrap_formula(level, inner),
    )
}

fn wrap_value(level: usize, inner: Value) -> Value {
    match level % 5 {
        0 => Value::List(vec![inner]),
        1 => Value::Set(vec![inner]),
        2 => Value::Record(vec![("field".into(), inner)]),
        3 => Value::Map(vec![(inner, Value::Bool(true))]),
        _ => Value::Map(vec![(Value::Bool(true), inner)]),
    }
}

fn value_cycle(depth: usize) -> Value {
    (0..depth).fold(Value::Bool(true), |inner, level| wrap_value(level, inner))
}

fn list_chain(depth: usize) -> Value {
    (0..depth).fold(Value::Bool(true), |inner, _| Value::List(vec![inner]))
}

fn not_chain(depth: usize) -> Expression {
    (0..depth).fold(truth(), |inner, _| Expression::Not {
        argument: boxed(inner),
    })
}

fn definition(id: usize, body: Expression) -> Definition {
    Definition {
        recursive: false,
        id,
        parameter_count: 0,
        capture_count: 0,
        body_kind: BodyKind::Expression,
        local_scope: 0,
        body: body.into(),
    }
}

fn program_with_deep_fields(depth: usize) -> SafetyProgram {
    SafetyProgram {
        format: "modelt.safety-program".into(),
        version: 2,
        name: "DropAllocations".into(),
        variables: vec!["x".into()],
        initial: vec![],
        next: frame_chain(depth),
        invariants: vec![],
        assumptions: vec![],
        check_deadlock: false,
        symmetry: None,
        initial_properties: vec![],
        action_properties: vec![],
        properties: vec![TemporalProperty {
            name: "Deep".into(),
            formula: formula_cycle(depth),
        }],
        fairness: vec![],
        constraints: vec![],
        action_constraints: vec![],
        definitions: Some(vec![
            definition(0, expression_cycle(depth)),
            definition(
                1,
                Expression::Data {
                    value: value_cycle(depth),
                },
            ),
        ]),
    }
}

fn observation(granted: &[&str]) -> Value {
    Value::Record(vec![
        (
            "granted".into(),
            Value::List(
                granted
                    .iter()
                    .map(|id| Value::String((*id).into()))
                    .collect(),
            ),
        ),
        ("locked".into(), Value::Bool(!granted.is_empty())),
    ])
}

fn ordinary_state(_: usize) -> Value {
    Value::Map(vec![
        (
            Value::ModelValue("a".into()),
            Value::Record(vec![
                (
                    "waiting".into(),
                    Value::Set(vec![observation(&["a"]), observation(&[])]),
                ),
                ("seen".into(), Value::List(vec![Value::Int("1".into())])),
            ]),
        ),
        (
            Value::ModelValue("b".into()),
            Value::List(vec![observation(&["a", "b"]), observation(&["b"])]),
        ),
    ])
}

fn shallow_expression() -> Expression {
    Expression::Not {
        argument: boxed(truth()),
    }
}

fn shallow_formula() -> TemporalFormula {
    TemporalFormula::Atom {
        expression: shallow_expression(),
    }
}

fn non_flat_value() -> Value {
    Value::List(vec![Value::List(vec![Value::Bool(true)])])
}

fn shallow_quantifier(_: usize) -> Expression {
    Expression::Forall {
        bounds: vec![binding(Domain::Value {
            value: boxed(shallow_expression()),
        })],
        body: boxed(truth()),
    }
}

fn wrapped_shallow_expression(rule: usize) -> Expression {
    wrap_expression(rule, shallow_expression())
}

fn enabled_step(step: Step) -> Expression {
    Expression::Enabled {
        branches: vec![guard_branch(vec![step])],
        subscript: None,
    }
}

fn frame_step(arguments: Vec<Expression>, captures: Vec<Expression>) -> Step {
    Step::Frame {
        arguments,
        captures,
        parameter_count: 0,
        capture_count: 0,
        local_scope: 0,
        branches: vec![],
    }
}

fn other_owner_edge(edge: usize) -> Expression {
    let inner = shallow_expression();
    match edge {
        0 => Expression::Add {
            left: boxed(truth()),
            right: boxed(inner),
        },
        1 => Expression::If {
            condition: boxed(inner),
            consequent: boxed(truth()),
            alternative: boxed(truth()),
        },
        2 => Expression::If {
            condition: boxed(truth()),
            consequent: boxed(inner),
            alternative: boxed(truth()),
        },
        3 => Expression::Subseq {
            sequence: boxed(inner),
            from: boxed(truth()),
            to: boxed(truth()),
        },
        4 => Expression::Subseq {
            sequence: boxed(truth()),
            from: boxed(inner),
            to: boxed(truth()),
        },
        5 => Expression::At {
            target: boxed(truth()),
            key: boxed(inner),
        },
        6 => Expression::Update {
            target: boxed(inner),
            updates: vec![],
        },
        7 => Expression::Forall {
            bounds: vec![binding(Domain::Value {
                value: boxed(truth()),
            })],
            body: boxed(inner),
        },
        8 => Expression::Function {
            bounds: vec![binding(Domain::Range {
                lower: boxed(truth()),
                upper: boxed(inner),
            })],
            body: boxed(truth()),
        },
        9 => Expression::Filter {
            bounds: vec![binding(Domain::Set {
                values: vec![truth(), inner],
            })],
            body: boxed(truth()),
        },
        10 => Expression::Choose {
            binding: binding(Domain::Value {
                value: boxed(truth()),
            }),
            body: boxed(inner),
        },
        11 => Expression::Fold {
            accumulator: 0,
            initial: boxed(inner),
            collection: boxed(truth()),
            order: FoldOrder::SequenceLeft,
            body: boxed(truth()),
        },
        12 => Expression::Fold {
            accumulator: 0,
            initial: boxed(truth()),
            collection: boxed(inner),
            order: FoldOrder::SequenceLeft,
            body: boxed(truth()),
        },
        13 => Expression::In {
            value: boxed(inner),
            domain: Domain::Value {
                value: boxed(truth()),
            },
        },
        14 => enabled_step(Step::Bind {
            slot: 0,
            domain: Domain::Value {
                value: boxed(inner),
            },
            tuple_size: None,
        }),
        15 => enabled_step(Step::Choose {
            slot: 0,
            domain: Domain::Range {
                lower: boxed(inner),
                upper: boxed(truth()),
            },
        }),
        16 => enabled_step(frame_step(vec![inner], vec![])),
        17 => enabled_step(frame_step(vec![], vec![inner])),
        18 => enabled_step(Step::Guard { expression: inner }),
        19 => Expression::Data {
            value: Value::List(vec![non_flat_value()]),
        },
        _ => Expression::MapSet {
            bounds: vec![binding(Domain::Value {
                value: boxed(inner),
            })],
            body: boxed(truth()),
        },
    }
}

const OTHER_OWNER_EDGES: usize = 21;

fn formula_owner_edge(edge: usize) -> TemporalFormula {
    match edge {
        0..=8 => wrap_formula(edge, shallow_formula()),
        9 => TemporalFormula::Frame {
            arguments: vec![shallow_expression()],
            captures: vec![],
            parameter_count: 1,
            capture_count: 0,
            local_scope: 0,
            formula: Box::new(atom()),
        },
        10 => TemporalFormula::Frame {
            arguments: vec![],
            captures: vec![shallow_expression()],
            parameter_count: 0,
            capture_count: 1,
            local_scope: 0,
            formula: Box::new(atom()),
        },
        11 => shallow_formula(),
        12 => TemporalFormula::Or {
            left: Box::new(atom()),
            right: Box::new(shallow_formula()),
        },
        13 => TemporalFormula::Forall {
            bounds: vec![binding(Domain::Value {
                value: boxed(shallow_expression()),
            })],
            formula: Box::new(atom()),
        },
        _ => TemporalFormula::Exists {
            bounds: vec![binding(Domain::Set {
                values: vec![shallow_expression()],
            })],
            formula: Box::new(atom()),
        },
    }
}

const FORMULA_OWNER_EDGES: usize = 15;

fn wrapped_non_flat_value(rule: usize) -> Value {
    wrap_value(rule, non_flat_value())
}

#[test]
fn shallow_quantifier_with_a_non_leaf_value_domain_drops_without_allocation() {
    assert_drop_allocations(0, shallow_quantifier, 0);
}

#[test]
fn every_expression_rule_over_a_shallow_non_leaf_child_drops_without_allocation() {
    for rule in 0..24 {
        assert_drop_allocations(rule, wrapped_shallow_expression, 0);
    }
}

#[test]
fn every_other_expression_owner_edge_over_a_shallow_non_leaf_child_drops_without_allocation() {
    for edge in 0..OTHER_OWNER_EDGES {
        assert_drop_allocations(edge, other_owner_edge, 0);
    }
}

#[test]
fn every_formula_owner_edge_over_a_shallow_non_leaf_child_drops_without_allocation() {
    for edge in 0..FORMULA_OWNER_EDGES {
        assert_drop_allocations(edge, formula_owner_edge, 0);
    }
}

#[test]
fn every_value_container_over_a_non_flat_child_drops_without_allocation() {
    for rule in 0..5 {
        assert_drop_allocations(rule, wrapped_non_flat_value, 0);
    }
}

#[test]
fn value_chain_up_to_the_depth_budget_drops_without_allocation() {
    assert_drop_allocations(34, list_chain, 0);
}

#[test]
fn value_chain_one_level_past_the_depth_budget_allocates_one_worklist() {
    assert_drop_allocations(35, list_chain, 1);
}

#[test]
fn expression_chain_up_to_the_depth_budget_drops_without_allocation() {
    assert_drop_allocations(33, not_chain, 0);
}

#[test]
fn expression_chain_one_level_past_the_depth_budget_allocates_one_worklist() {
    assert_drop_allocations(34, not_chain, 1);
}

#[test]
fn ordinary_values_and_programs_drop_without_allocation() {
    assert_drop_allocations(0, |_| observation(&["a", "b"]), 0);
    assert_drop_allocations(
        0,
        |_| Value::List(vec![observation(&["a"]), observation(&[])]),
        0,
    );
    assert_drop_allocations(0, ordinary_state, 0);
    assert_drop_allocations(ORDINARY_DEPTH, value_cycle, 0);
    assert_drop_allocations(ORDINARY_DEPTH, expression_cycle, 0);
    assert_drop_allocations(ORDINARY_DEPTH, frame_chain, 0);
    assert_drop_allocations(ORDINARY_DEPTH, formula_cycle, 0);
    assert_drop_allocations(ORDINARY_DEPTH, program_with_deep_fields, 0);
}
