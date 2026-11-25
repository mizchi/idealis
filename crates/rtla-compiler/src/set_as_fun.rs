use rtla::ast::{Bound, Expression, ExpressionKind as E, Location, Pattern};

pub(crate) const DUPLICATE_KEY: &str = "SetAsFun: a key has more than one value";

pub(crate) struct Expansion {
    pub(crate) single_valued: Expression,
    pub(crate) function: Expression,
}

fn node(node: E, location: Location) -> Expression {
    Expression { node, location }
}

fn name(value: &str, location: Location) -> Expression {
    node(E::Identifier { name: value.into() }, location)
}

fn component(tuple: &str, index: u64, location: Location) -> Expression {
    node(
        E::FunctionApplication {
            function: Box::new(name(tuple, location)),
            args: vec![node(E::Number { value: index }, location)],
        },
        location,
    )
}

fn apply(operator: &str, args: Vec<Expression>, location: Location) -> Expression {
    node(
        E::Apply {
            operator: operator.into(),
            args,
        },
        location,
    )
}

fn bound(variable: &str, set: &Expression) -> Bound {
    Bound {
        pattern: Pattern::Name {
            name: variable.into(),
        },
        set: set.clone(),
    }
}

struct Names {
    entry: String,
    key: String,
    matched: String,
    left: String,
    right: String,
}

fn fresh_names(set: &Expression) -> Names {
    let text = serde_json::to_string(set).unwrap_or_default();
    let mut prefix = String::from("__SetAsFun");
    while text.contains(&prefix) {
        prefix.push('_');
    }
    let derive = |suffix: &str| format!("{prefix}_{suffix}");
    Names {
        entry: derive("entry"),
        key: derive("key"),
        matched: derive("match"),
        left: derive("left"),
        right: derive("right"),
    }
}

pub(crate) fn expand(set: &Expression, location: Location) -> Expansion {
    let names = fresh_names(set);
    let first_keys = node(
        E::SetMap {
            expression: Box::new(component(&names.entry, 1, location)),
            bounds: vec![bound(&names.entry, set)],
        },
        location,
    );
    let chosen_entry = node(
        E::Choose {
            pattern: Pattern::Name {
                name: names.matched.clone(),
            },
            set: Some(Box::new(set.clone())),
            body: Box::new(apply(
                "=",
                vec![
                    component(&names.matched, 1, location),
                    name(&names.key, location),
                ],
                location,
            )),
        },
        location,
    );
    let function = node(
        E::FunctionConstruction {
            bounds: vec![Bound {
                pattern: Pattern::Name {
                    name: names.key.clone(),
                },
                set: first_keys,
            }],
            body: Box::new(node(
                E::FunctionApplication {
                    function: Box::new(chosen_entry),
                    args: vec![node(E::Number { value: 2 }, location)],
                },
                location,
            )),
        },
        location,
    );
    let agreement = apply(
        "=>",
        vec![
            apply(
                "=",
                vec![
                    component(&names.left, 1, location),
                    component(&names.right, 1, location),
                ],
                location,
            ),
            apply(
                "=",
                vec![
                    component(&names.left, 2, location),
                    component(&names.right, 2, location),
                ],
                location,
            ),
        ],
        location,
    );
    let single_valued = node(
        E::Forall {
            bounds: vec![bound(&names.left, set)],
            body: Box::new(node(
                E::Forall {
                    bounds: vec![bound(&names.right, set)],
                    body: Box::new(agreement),
                },
                location,
            )),
        },
        location,
    );
    Expansion {
        single_valued,
        function,
    }
}
