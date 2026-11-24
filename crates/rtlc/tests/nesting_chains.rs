mod chain_programs;

use chain_programs::*;
use rtlc::ir::{Domain, Expression};

const CHECKER_LIMIT: &str = "error protocol: Safety expression nesting exceeds 48";
const IR_LIMIT: &str = "error protocol: Safety IR expression nesting exceeds 128";

#[derive(Clone, Copy, Debug)]
enum Shape {
    And,
    Or,
    If,
}

fn chain(shape: Shape, length: usize) -> Expression {
    let index = |k: usize| i32::try_from(k).unwrap();
    match shape {
        Shape::And => conjunction((0..length).map(|k| ne(x(), int(200 + index(k)))).collect()),
        Shape::Or => disjunction(
            (0..length)
                .map(|k| {
                    if k + 1 == length {
                        ge(x(), int(0))
                    } else {
                        eq(x(), int(200 + index(k)))
                    }
                })
                .collect(),
        ),
        Shape::If => else_if(
            (0..length)
                .map(|k| (eq(x(), int(20 + index(k))), boolean(false)))
                .collect(),
            boolean(true),
        ),
    }
}

fn top_level(shape: Shape, length: usize) -> String {
    outcome(&counter(2, chain(shape, length), vec![]))
}

#[test]
fn a_long_chain_in_an_invariant_is_checked_like_a_three_term_chain() {
    on_large_stack(|| {
        for shape in [Shape::And, Shape::Or, Shape::If] {
            let expected = top_level(shape, 3);
            assert!(expected.starts_with("complete "), "{shape:?}: {expected}");
            for length in [47, 48, 49, 54, 61, 100, 127] {
                assert_eq!(top_level(shape, length), expected, "{shape:?} {length}");
            }
        }
    });
}

fn split(shape: Shape, definitions: usize, links: usize) -> String {
    let index = |k: usize| i32::try_from(k).unwrap();
    let bodies = (0..definitions).map(|id| {
        let next = if id + 1 == definitions {
            boolean(true)
        } else {
            call(id + 1, vec![parameter(0)])
        };
        let body = match shape {
            Shape::If => else_if(
                (0..links)
                    .map(|k| (eq(x(), int(20 + index(k))), boolean(false)))
                    .collect(),
                next,
            ),
            _ => conjunction(
                (0..links)
                    .map(|k| ne(x(), int(200 + index(k))))
                    .chain([next])
                    .collect(),
            ),
        };
        definition(id, 1, body)
    });
    outcome(&counter(2, call(0, vec![int(1)]), bodies.collect()))
}

#[test]
fn chains_split_across_definitions_are_checked_like_two_link_chains() {
    on_large_stack(|| {
        for shape in [Shape::If, Shape::And] {
            for (definitions, links) in [(2, 47), (3, 40), (3, 41), (4, 31), (5, 25), (3, 100)] {
                let expected = split(shape, definitions, 2);
                assert!(expected.starts_with("complete "), "{shape:?}: {expected}");
                assert_eq!(
                    split(shape, definitions, links),
                    expected,
                    "{shape:?} {definitions} x {links}"
                );
            }
        }
    });
}

fn division_error() -> Expression {
    ne(div(int(10), sub(int(3), x())), int(0))
}

fn unreachable_error() -> Expression {
    eq(div(int(1), int(0)), int(0))
}

fn replaced(terms: &mut [Expression], position: usize, term: Expression) {
    if let Some(slot) = terms.get_mut(position - 1) {
        *slot = term;
    }
}

fn and_terms(length: usize) -> Vec<Expression> {
    (0..length)
        .map(|k| ne(x(), int(200 + i32::try_from(k).unwrap())))
        .collect()
}

fn from_position(terms: &[Expression], position: usize) -> Vec<Expression> {
    terms[position - 1..terms.len().min(position + 1)].to_vec()
}

fn junction_cases(length: usize, position: usize) -> Vec<(&'static str, Vec<Expression>, bool)> {
    let mut false_at = and_terms(length);
    replaced(&mut false_at, position, ne(x(), int(3)));
    let mut skips = false_at.clone();
    replaced(&mut skips, position + 1, division_error());
    let mut error_first = and_terms(length);
    replaced(&mut error_first, position, division_error());
    replaced(&mut error_first, position + 1, ne(x(), int(3)));
    let mut non_boolean = and_terms(length);
    replaced(&mut non_boolean, position, int(1));
    let mut true_at: Vec<Expression> = (0..length)
        .map(|k| eq(x(), int(300 + i32::try_from(k).unwrap())))
        .collect();
    replaced(&mut true_at, position, ge(x(), int(0)));
    replaced(&mut true_at, position + 1, unreachable_error());
    vec![
        ("and-false", false_at, true),
        ("and-false-skips", skips, true),
        ("and-error-first", error_first, true),
        ("and-non-boolean", non_boolean, true),
        ("or-true-skips", true_at, false),
    ]
}

fn joined(terms: Vec<Expression>, conjunctive: bool) -> Expression {
    if conjunctive {
        conjunction(terms)
    } else {
        disjunction(terms)
    }
}

#[test]
fn short_circuits_and_first_errors_in_a_long_junction_match_the_chain_from_that_term() {
    on_large_stack(|| {
        let length = 60;
        for position in [1, 27, 28, 60] {
            for (name, terms, conjunctive) in junction_cases(length, position) {
                let expected = outcome(&counter(
                    2,
                    joined(from_position(&terms, position), conjunctive),
                    vec![],
                ));
                assert_eq!(
                    outcome(&counter(2, joined(terms, conjunctive), vec![])),
                    expected,
                    "{name} at {position}"
                );
            }
        }
    });
}

fn links(
    length: usize,
    position: usize,
    condition: Expression,
    consequent: Expression,
) -> Vec<(Expression, Expression)> {
    (0..length)
        .map(|k| {
            if k + 1 == position {
                (condition.clone(), consequent.clone())
            } else {
                (
                    eq(x(), int(1000 + i32::try_from(k).unwrap())),
                    unreachable_error(),
                )
            }
        })
        .collect()
}

#[test]
fn the_selected_branch_and_first_errors_in_a_long_if_chain_match_the_chain_from_that_link() {
    on_large_stack(|| {
        let length = 54;
        for position in [1, 27, 28, 54] {
            for (name, condition, consequent) in [
                ("selects", ge(x(), int(0)), ne(x(), int(3))),
                ("condition-non-boolean", int(5), boolean(true)),
                ("condition-error", division_error(), boolean(true)),
            ] {
                let all = links(length, position, condition, consequent);
                let expected = outcome(&counter(
                    2,
                    else_if(vec![all[position - 1].clone()], unreachable_error()),
                    vec![],
                ));
                assert_eq!(
                    outcome(&counter(2, else_if(all, unreachable_error()), vec![])),
                    expected,
                    "{name} at {position}"
                );
            }
        }
    });
}

fn membership(length: usize) -> Expression {
    let sets = (0..length)
        .map(|k| {
            let k = i32::try_from(k).unwrap();
            (eq(x(), int(20 + k)), set(vec![int(100 + k)]))
        })
        .collect();
    function_in(x(), else_if(sets, set((0..=5).map(int).collect())))
}

fn lazy(length: usize) -> Expression {
    let functions = (0..length)
        .map(|k| {
            let k = i32::try_from(k).unwrap();
            (
                eq(x(), int(20 + k)),
                function(0, range(0, 5), boolean(false)),
            )
        })
        .collect();
    at(
        else_if(functions, function(0, range(0, 5), ge(bound(0), int(0)))),
        x(),
    )
}

#[test]
fn a_long_if_chain_in_a_membership_set_or_a_lazy_function_is_checked_like_a_short_one() {
    on_large_stack(|| {
        for (name, build) in [
            ("membership", membership as fn(usize) -> Expression),
            ("lazy", lazy),
        ] {
            let expected = outcome(&counter(2, build(2), vec![]));
            assert!(expected.starts_with("complete "), "{name}: {expected}");
            for length in [47, 48, 54, 100] {
                assert_eq!(
                    outcome(&counter(2, build(length), vec![])),
                    expected,
                    "{name} {length}"
                );
            }
        }
    });
}

fn countdown(links: usize, start: i32) -> String {
    let base = if_then(
        eq(parameter(0), int(0)),
        ge(x(), int(0)),
        call(0, vec![sub(parameter(0), int(1))]),
    );
    let body = (0..links).rev().fold(base, |alternative, k| {
        if_then(
            eq(x(), int(1000 + i32::try_from(k).unwrap())),
            boolean(false),
            alternative,
        )
    });
    outcome(&counter(
        2,
        call(0, vec![int(start)]),
        vec![recursive(0, 1, body)],
    ))
}

#[test]
fn a_recursive_definition_with_a_long_if_chain_recurses_like_one_without_the_chain() {
    on_large_stack(|| {
        let mut bounded = false;
        for start in [3, 30, 40, 60, 62, 63, 64, 70] {
            let expected = countdown(0, start);
            bounded |= expected.starts_with("bounded reason=evaluation");
            for links in [47, 54] {
                assert_eq!(
                    countdown(links, start),
                    expected,
                    "{links} links from {start}"
                );
            }
        }
        assert!(bounded, "a start reaches the evaluation bound");
    });
}

type Wrap = fn(Expression) -> Expression;

fn nested(length: usize, wrap: impl Fn(Expression) -> Expression) -> Expression {
    (0..length).fold(ne(x(), int(200)), |inner, _| wrap(inner))
}

#[test]
fn nesting_that_is_not_a_chain_keeps_the_checker_limit() {
    on_large_stack(|| {
        let shapes: [(&str, Wrap); 6] = [
            ("right-and", |inner| and(ne(x(), int(201)), inner)),
            ("right-or", |inner| or(eq(x(), int(201)), inner)),
            ("consequent-if", |inner| {
                if_then(ge(x(), int(0)), inner, boolean(false))
            }),
            ("condition-if", |inner| {
                if_then(inner, boolean(true), boolean(true))
            }),
            ("implies", |inner| implies(inner, boolean(true))),
            ("negation", not),
        ];
        for (name, wrap) in shapes {
            let accepted = outcome(&counter(2, nested(46, wrap), vec![]));
            assert!(!accepted.starts_with("error"), "{name}: {accepted}");
            assert_eq!(
                outcome(&counter(2, nested(48, wrap), vec![])),
                CHECKER_LIMIT,
                "{name}"
            );
        }
        let alternating = (0..48).fold(ne(x(), int(200)), |inner, level| {
            if level % 2 == 0 {
                and(inner, ne(x(), int(201)))
            } else {
                or(inner, eq(x(), int(201)))
            }
        });
        assert_eq!(outcome(&counter(2, alternating, vec![])), CHECKER_LIMIT);
    });
}

#[test]
fn version_one_keeps_counting_every_level_of_a_chain() {
    on_large_stack(|| {
        for shape in [Shape::And, Shape::Or, Shape::If] {
            let expected = outcome(&counter(1, chain(shape, 3), vec![]));
            assert!(expected.starts_with("complete "), "{shape:?}: {expected}");
            assert_eq!(
                outcome(&counter(1, chain(shape, 47), vec![])),
                expected,
                "{shape:?}"
            );
            assert_eq!(
                outcome(&counter(1, chain(shape, 49), vec![])),
                CHECKER_LIMIT,
                "{shape:?}"
            );
        }
    });
}

#[test]
fn a_chain_deeper_than_the_ir_boundary_is_refused_before_evaluation() {
    on_large_stack(|| {
        let expected = top_level(Shape::And, 3);
        assert_eq!(top_level(Shape::And, 128), expected);
        assert_eq!(top_level(Shape::And, 129), IR_LIMIT);
        assert_eq!(top_level(Shape::If, 127), top_level(Shape::If, 3));
        assert_eq!(top_level(Shape::If, 128), IR_LIMIT);
        assert_eq!(top_level(Shape::If, 100_000), IR_LIMIT);
        let deep = definition(0, 0, chain(Shape::Or, 100_000));
        assert_eq!(outcome(&counter(2, boolean(true), vec![deep])), IR_LIMIT);
        let within = Domain::Value {
            value: Box::new(chain(Shape::If, 100_000)),
        };
        let membership = Expression::In {
            value: Box::new(x()),
            domain: within,
        };
        assert_eq!(outcome(&counter(2, membership, vec![])), IR_LIMIT);
    });
}
