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

pub(crate) fn expand(set: &Expression, location: Location) -> Expansion {
    let first_keys = node(
        E::SetMap {
            expression: Box::new(component("__SetAsFun_entry", 1, location)),
            bounds: vec![bound("__SetAsFun_entry", set)],
        },
        location,
    );
    let chosen_entry = node(
        E::Choose {
            pattern: Pattern::Name {
                name: "__SetAsFun_match".into(),
            },
            set: Some(Box::new(set.clone())),
            body: Box::new(apply(
                "=",
                vec![
                    component("__SetAsFun_match", 1, location),
                    name("__SetAsFun_key", location),
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
                    name: "__SetAsFun_key".into(),
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
                    component("__SetAsFun_left", 1, location),
                    component("__SetAsFun_right", 1, location),
                ],
                location,
            ),
            apply(
                "=",
                vec![
                    component("__SetAsFun_left", 2, location),
                    component("__SetAsFun_right", 2, location),
                ],
                location,
            ),
        ],
        location,
    );
    let single_valued = node(
        E::Forall {
            bounds: vec![bound("__SetAsFun_left", set)],
            body: Box::new(node(
                E::Forall {
                    bounds: vec![bound("__SetAsFun_right", set)],
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
