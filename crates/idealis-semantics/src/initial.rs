//! Initial constraints that bind relations without traversing candidates.
use idealis_syntax::{Compare, Expr, Formula, Model};
use std::collections::BTreeMap;

/// Closed value expressions can be evaluated before any relation is bound.
fn closed_value(model: &Model, expression: &Expr) -> bool {
    match expression {
        Expr::None | Expr::Literal(_) => true,
        Expr::Name(name) => model.relation(name).is_none(),
        Expr::Binary(_, left, right)
        | Expr::Arith(_, left, right)
        | Expr::Append(left, right)
        | Expr::Take(left, right)
        | Expr::RemoveAt(left, right)
        | Expr::RemoveAll(left, right)
        | Expr::ReachableFrom(left, right)
        | Expr::Index(left, right) => closed_value(model, left) && closed_value(model, right),
        Expr::Count(inner)
        | Expr::Tail(inner)
        | Expr::Keys(inner)
        | Expr::Closure(inner)
        | Expr::Length(inner)
        | Expr::Elements(inner)
        | Expr::Field(inner, _)
        | Expr::Payload(inner, _) => closed_value(model, inner),
        Expr::Record(_, fields) | Expr::Sequence(_, fields) => {
            fields.iter().all(|field| closed_value(model, field))
        }
        Expr::Variant(_, payload) => payload
            .as_deref()
            .is_none_or(|value| closed_value(model, value)),
        Expr::If(..) | Expr::Select(..) | Expr::At(..) => false,
    }
}

pub fn fixed_initial_values(model: &Model) -> BTreeMap<&str, &Expr> {
    fn collect<'a>(
        model: &Model,
        formula: &'a Formula,
        variables: bool,
        fixed: &mut BTreeMap<&'a str, &'a Expr>,
    ) {
        let mut bind = |target: &'a Expr, value: &'a Expr| {
            if let Expr::Name(name) = target
                && model
                    .relation(name)
                    .is_some_and(|relation| relation.var == variables)
                && closed_value(model, value)
            {
                fixed.entry(name).or_insert(value);
            }
        };
        match formula {
            Formula::And(parts) => {
                for part in parts {
                    collect(model, part, variables, fixed);
                }
            }
            Formula::Compare(Compare::Equal, left, right) => {
                bind(left, right);
                bind(right, left);
            }
            Formula::Empty(target) => bind(target, &Expr::None),
            _ => {}
        }
    }
    let mut fixed = BTreeMap::new();
    for fact in &model.facts {
        collect(model, fact, false, &mut fixed);
    }
    for initial in &model.init {
        collect(model, initial, true, &mut fixed);
    }
    fixed
}
