use crate::ir::{Definition, DefinitionBody, Expression};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OrderIndependence {
    Dependent,
    Constant,
    Union,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reference {
    Bound(usize),
    Parameter(usize),
}

impl Reference {
    fn is(self, expression: &Expression) -> bool {
        match (self, expression) {
            (Self::Bound(slot), Expression::Bound { slot: found })
            | (Self::Parameter(slot), Expression::Parameter { slot: found }) => slot == *found,
            _ => false,
        }
    }
}

fn mentions(expression: &Expression, references: &[Reference]) -> bool {
    if references.iter().any(|reference| reference.is(expression)) {
        return true;
    }
    match expression {
        Expression::Enabled { .. } => true,
        Expression::Frame {
            arguments,
            captures,
            body,
            ..
        } => arguments
            .iter()
            .chain(captures)
            .chain(std::iter::once(body.as_ref()))
            .any(|child| mentions(child, references)),
        _ => expression
            .children()
            .into_iter()
            .any(|child| mentions(child, references)),
    }
}

fn is_union(expression: &Expression, accumulator: Reference) -> bool {
    match expression {
        Expression::UnmatchedCase { .. } => true,
        Expression::Cup { left, right } => {
            (accumulator.is(left) && !mentions(right, &[accumulator]))
                || (accumulator.is(right) && !mentions(left, &[accumulator]))
        }
        Expression::If {
            condition,
            consequent,
            alternative,
        } => {
            !mentions(condition, &[accumulator])
                && is_union(consequent, accumulator)
                && is_union(alternative, accumulator)
        }
        _ => accumulator.is(expression),
    }
}

pub(crate) fn classify(
    accumulator: usize,
    body: &Expression,
    definitions: &[Definition],
) -> OrderIndependence {
    let (callee, accumulator, element) = match body {
        Expression::Call {
            definition,
            arguments,
            captures,
        } => {
            let [
                Expression::Bound { slot: first },
                Expression::Bound { slot: second },
            ] = arguments.as_slice()
            else {
                return OrderIndependence::Dependent;
            };
            let slots = [
                Reference::Bound(accumulator),
                Reference::Bound(accumulator + 1),
            ];
            let Some(Definition {
                body: DefinitionBody::Expression(callee),
                ..
            }) = definitions.get(*definition)
            else {
                return OrderIndependence::Dependent;
            };
            if *first != accumulator
                || *second != accumulator + 1
                || captures.iter().any(|capture| mentions(capture, &slots))
            {
                return OrderIndependence::Dependent;
            }
            (callee, Reference::Parameter(0), Reference::Parameter(1))
        }
        _ => (
            body,
            Reference::Bound(accumulator),
            Reference::Bound(accumulator + 1),
        ),
    };
    if !mentions(callee, &[element]) {
        OrderIndependence::Constant
    } else if is_union(callee, accumulator) {
        OrderIndependence::Union
    } else {
        OrderIndependence::Dependent
    }
}
