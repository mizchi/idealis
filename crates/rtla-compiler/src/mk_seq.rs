use crate::{Result, unsupported};
use rtla::ast::{Bound, Expression, ExpressionKind as E, Pattern};

const INDEX: &str = "__MkSeq_index";

pub(crate) fn expand(args: &[Expression]) -> Result<Expression> {
    let [length, operator] = args else {
        return unsupported("operator arity MkSeq");
    };
    let E::Identifier { name } = &operator.node else {
        return unsupported("MkSeq operator argument");
    };
    let location = length.location;
    let node = |node| Expression { node, location };
    let positions = node(E::Apply {
        operator: "..".into(),
        args: vec![node(E::Number { value: 1 }), length.clone()],
    });
    let element = node(E::Apply {
        operator: name.clone(),
        args: vec![node(E::Identifier { name: INDEX.into() })],
    });
    Ok(node(E::FunctionConstruction {
        bounds: vec![Bound {
            pattern: Pattern::Name { name: INDEX.into() },
            set: positions,
        }],
        body: Box::new(element),
    }))
}
