use rtlc::ir::{
    Binding, BodyKind, Branch, Definition, Domain, Expression, FoldOrder, Scalar, Step,
    TemporalFormula, TemporalProperty, Update,
};
use rtlc::{SafetyProgram, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

const SHALLOW_DEPTH: usize = 1_000;
const SHALLOW_STACK_BYTES: usize = 8 * 1024 * 1024;
const DEEP_DEPTH: usize = 200_000;
const DEEP_STACK_BYTES: usize = 512 * 1024;

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

    fn built_bytes(&self) -> isize {
        self.built.bytes - self.before.bytes
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

fn assert_freed(run: &Measurement, depth: usize) {
    assert!(
        run.built_blocks() >= depth as isize,
        "the counter sees at least one block per level at depth {depth}"
    );
    assert_eq!(run.remaining_blocks(), 0, "live blocks at depth {depth}");
    assert_eq!(run.remaining_bytes(), 0, "live bytes at depth {depth}");
}

fn assert_chain_freed<T: 'static>(build: fn(usize) -> T) {
    let shallow = measure(SHALLOW_STACK_BYTES, SHALLOW_DEPTH, build, drop);
    assert_freed(&shallow, SHALLOW_DEPTH);
    let deep = measure(DEEP_STACK_BYTES, DEEP_DEPTH, build, drop);
    assert_freed(&deep, DEEP_DEPTH);
    assert_eq!(
        shallow.drop_events(),
        deep.drop_events(),
        "allocations during the drop do not depend on the depth"
    );
}

fn assert_leak_reported<T: 'static>(build: fn(usize) -> T, dispose: fn(T)) {
    let run = measure(DEEP_STACK_BYTES, DEEP_DEPTH, build, dispose);
    assert!(run.built_blocks() >= DEEP_DEPTH as isize);
    assert_eq!(run.built_bytes() % run.built_blocks(), 0);
    let root_bytes = run.built_bytes() / run.built_blocks();
    assert_eq!(run.remaining_blocks(), run.built_blocks() - 1);
    assert_eq!(run.remaining_bytes(), run.built_bytes() - root_bytes);
}

fn assert_no_allocation_on_drop<T: 'static>(build: fn(usize) -> T) {
    let run = measure(SHALLOW_STACK_BYTES, 0, build, drop);
    assert!(run.built_blocks() > 0, "the counter sees the construction");
    assert_eq!(run.drop_events(), 0, "allocations during the drop");
    assert_eq!(run.remaining_blocks(), 0);
    assert_eq!(run.remaining_bytes(), 0);
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

fn forget_value_child(mut root: Value) {
    let Value::List(items) = &mut root else {
        panic!("the chain root is a list")
    };
    std::mem::forget(std::mem::replace(&mut items[0], Value::Null));
    drop(root);
}

fn forget_expression_child(mut root: Expression) {
    let Expression::Not { argument } = &mut root else {
        panic!("the chain root is a negation")
    };
    std::mem::forget(std::mem::replace(&mut **argument, Expression::Old));
    drop(root);
}

#[test]
fn expression_chain_through_every_rule_is_freed_without_per_node_allocation() {
    assert_chain_freed(expression_cycle);
}

#[test]
fn step_frame_branch_chain_is_freed_without_per_node_allocation() {
    assert_chain_freed(frame_chain);
}

#[test]
fn temporal_formula_chain_through_every_rule_is_freed_without_per_node_allocation() {
    assert_chain_freed(formula_cycle);
}

#[test]
fn value_chain_through_every_container_is_freed_without_per_node_allocation() {
    assert_chain_freed(value_cycle);
}

#[test]
fn program_with_deep_fields_is_freed_without_per_node_allocation() {
    assert_chain_freed(program_with_deep_fields);
}

#[test]
fn leaked_value_chain_is_reported_by_the_counter() {
    assert_leak_reported(list_chain, forget_value_child);
}

#[test]
fn leaked_expression_chain_is_reported_by_the_counter() {
    assert_leak_reported(not_chain, forget_expression_child);
}

#[test]
fn nodes_whose_children_are_all_leaves_drop_without_allocation() {
    assert_no_allocation_on_drop(|_| {
        Value::Record(vec![
            ("count".into(), Value::Int("1".into())),
            ("ready".into(), Value::Bool(true)),
        ])
    });
    assert_no_allocation_on_drop(|_| Expression::Add {
        left: boxed(truth()),
        right: boxed(truth()),
    });
    assert_no_allocation_on_drop(|_| {
        guard_branch(vec![Step::Guard {
            expression: truth(),
        }])
    });
    assert_no_allocation_on_drop(|_| TemporalFormula::And {
        left: Box::new(atom()),
        right: Box::new(atom()),
    });
}
