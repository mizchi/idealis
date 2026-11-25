use rtla::ast::{Bound, Expression, ExpressionKind as E, Location, Pattern};

fn node(node: E, location: Location) -> Expression {
    Expression { node, location }
}

fn fresh_prefix(factors: &[Expression]) -> String {
    let text = serde_json::to_string(factors).unwrap_or_default();
    let mut prefix = String::from("__Product");
    while text.contains(&prefix) {
        prefix.push('_');
    }
    prefix
}

pub(crate) fn expand(factors: &[Expression], location: Location) -> Expression {
    let prefix = fresh_prefix(factors);
    let names: Vec<String> = (0..factors.len())
        .map(|index| format!("{prefix}_{index}"))
        .collect();
    let tuple = node(
        E::Tuple {
            elements: names
                .iter()
                .map(|name| node(E::Identifier { name: name.clone() }, location))
                .collect(),
        },
        location,
    );
    let mut result = tuple;
    for (index, (name, factor)) in names.iter().zip(factors).enumerate().rev() {
        let map = node(
            E::SetMap {
                expression: Box::new(result),
                bounds: vec![Bound {
                    pattern: Pattern::Name { name: name.clone() },
                    set: factor.clone(),
                }],
            },
            location,
        );
        result = if index + 1 == factors.len() {
            map
        } else {
            node(
                E::Apply {
                    operator: "UNION".into(),
                    args: vec![map],
                },
                location,
            )
        };
    }
    result
}
