use crate::analysis::{Node, analysis_children};
use crate::rewrite::identifier;
use crate::{CompileError, Result, definition_body, unsupported};
use rtla::ast::{
    self, Declaration, Definition, DefinitionKind, Expression, ExpressionKind as E, Selector,
};
use std::collections::BTreeMap;

fn index(selector: &Selector, arity: usize) -> Result<usize> {
    let number = match selector {
        Selector::Index { index } => usize::try_from(*index).ok(),
        Selector::Left if arity > 0 => Some(1),
        Selector::Right if arity == 2 => Some(2),
        _ => None,
    };
    number
        .filter(|number| *number > 0 && *number <= arity)
        .map(|number| number - 1)
        .ok_or_else(|| CompileError {
            message: format!("invalid subexpression selector for {arity} operands"),
        })
}

fn label(value: &Expression, name: &str) -> Result<(Expression, Vec<String>)> {
    let mut pending = vec![value];
    let mut found = None;
    while let Some(value) = pending.pop() {
        if let E::Label {
            name: candidate,
            params,
            body,
        } = &value.node
        {
            if candidate == name {
                if found.is_some() {
                    return unsupported(format!("duplicate label {name}"));
                }
                found = Some((*body.clone(), params.clone()));
            }
            continue;
        }
        pending.extend(
            analysis_children(Node::Source(value))
                .into_iter()
                .filter_map(|edge| {
                    if let Node::Source(value) = edge.node {
                        Some(value)
                    } else {
                        None
                    }
                }),
        );
    }
    found.ok_or_else(|| CompileError {
        message: format!("unknown subexpression label {name}"),
    })
}

fn bound_body(value: &Expression) -> Option<(Expression, Vec<Declaration>)> {
    let (body, patterns) = match &value.node {
        E::Forall { bounds, body }
        | E::Exists { bounds, body }
        | E::FunctionConstruction { bounds, body }
        | E::SetMap {
            bounds,
            expression: body,
        } => (
            body.as_ref(),
            bounds
                .iter()
                .map(|bound| &bound.pattern)
                .collect::<Vec<_>>(),
        ),
        E::Choose { pattern, body, .. } => (body.as_ref(), vec![pattern]),
        E::SetFilter { bound, predicate } => (predicate.as_ref(), vec![&bound.pattern]),
        E::UnboundedQuantifier { names, body, .. } => {
            return Some((
                *body.clone(),
                names
                    .iter()
                    .map(|name| Declaration {
                        name: name.clone(),
                        arity: 0,
                        location: value.location,
                    })
                    .collect(),
            ));
        }
        _ => return None,
    };
    let params = patterns
        .into_iter()
        .flat_map(|pattern| match pattern {
            ast::Pattern::Name { name } => std::slice::from_ref(name),
            ast::Pattern::Tuple { names } => names.as_slice(),
        })
        .map(|name| Declaration {
            name: name.clone(),
            arity: 0,
            location: value.location,
        })
        .collect();
    Some((body.clone(), params))
}

fn operands(value: &Expression) -> Result<Vec<Expression>> {
    Ok(match &value.node {
        E::Apply { args, .. } | E::Bracket { args } => args.clone(),
        E::And { operands, .. } | E::Or { operands, .. } => operands.clone(),
        E::Tuple { elements } | E::SetEnumeration { elements } => elements.clone(),
        E::Product { factors } => factors.clone(),
        E::Prime { expression }
        | E::Unchanged { expression }
        | E::Enabled { expression }
        | E::Always { expression }
        | E::Eventually { expression } => vec![*expression.clone()],
        E::If {
            condition,
            consequent,
            alternative,
        } => vec![
            *condition.clone(),
            *consequent.clone(),
            *alternative.clone(),
        ],
        E::Record { fields } | E::RecordSet { fields } => {
            fields.iter().map(|field| field.value.clone()).collect()
        }
        E::FunctionSet { domain, range } => vec![*domain.clone(), *range.clone()],
        E::Forall { bounds, .. }
        | E::Exists { bounds, .. }
        | E::FunctionConstruction { bounds, .. }
        | E::SetMap { bounds, .. } => bounds.iter().map(|bound| bound.set.clone()).collect(),
        E::SetFilter { bound, .. } => vec![bound.set.clone()],
        E::Choose { set, .. } => set.iter().map(|set| *set.clone()).collect(),
        E::FieldAccess { record, field } => vec![
            *record.clone(),
            Expression {
                node: E::String {
                    value: field.clone(),
                },
                location: value.location,
            },
        ],
        E::FunctionApplication { function, args } => vec![
            *function.clone(),
            if args.len() == 1 {
                args[0].clone()
            } else {
                Expression {
                    node: E::Tuple {
                        elements: args.clone(),
                    },
                    location: value.location,
                }
            },
        ],
        E::BoxAction { action, subscript } | E::AngleAction { action, subscript } => {
            vec![*action.clone(), *subscript.clone()]
        }
        E::WeakFairness { action, subscript } | E::StrongFairness { action, subscript } => {
            vec![*subscript.clone(), *action.clone()]
        }
        E::Except { function, .. } => vec![*function.clone()],
        _ => return unsupported("selected expression has no selectable operands"),
    })
}

struct Parameters {
    params: Vec<Declaration>,
    arguments: Vec<Expression>,
    missing: Vec<Declaration>,
}

impl Parameters {
    fn add(&mut self, params: &[Declaration], args: Option<&[Expression]>) -> Result<()> {
        if let Some(args) = args
            && args.len() != params.len()
        {
            return unsupported(format!(
                "subexpression expects {} arguments, received {}",
                params.len(),
                args.len()
            ));
        }
        for (index, param) in params.iter().enumerate() {
            if self
                .params
                .iter()
                .any(|previous| previous.name == param.name)
            {
                return unsupported("repeated subexpression parameter");
            }
            self.params.push(param.clone());
            self.arguments.push(args.map_or_else(
                || identifier(&param.name, param.location),
                |args| args[index].clone(),
            ));
            if args.is_none() {
                self.missing.push(param.clone());
            }
        }
        Ok(())
    }
}

pub(crate) fn resolve(
    base: &Expression,
    selectors: &[Selector],
    definitions: &BTreeMap<String, Definition>,
    names: &BTreeMap<String, String>,
    helper: String,
) -> Result<Expression> {
    let (name, args) = match &base.node {
        E::Identifier { name } => (name, None),
        E::Apply { operator, args } => (operator, Some(args.as_slice())),
        _ => return unsupported("subexpression selector requires an operator name"),
    };
    let definition = definitions.get(name).ok_or_else(|| CompileError {
        message: format!("unknown subexpression operator {name}"),
    })?;
    let mut parameters = Parameters {
        params: vec![],
        arguments: vec![],
        missing: vec![],
    };
    if let DefinitionKind::Operator { params } = &definition.head {
        parameters.add(params, args)?;
    } else if args.is_some_and(|args| !args.is_empty()) {
        return unsupported("function definition has no operator parameters");
    }
    let mut current = definition_body(definition);
    let mut scopes = vec![];
    let mut named = true;
    let mut operator = true;
    let mut lookup = false;
    let mut cursor = 0;
    while let Some(selector) = selectors.get(cursor) {
        match selector {
            Selector::Body => {
                if !named
                    || !(cursor + 1 == selectors.len() && operator
                        || matches!(selectors.get(cursor + 1), Some(Selector::Name { .. })))
                {
                    return unsupported(
                        "!: must follow a name and precede a LET operator, or end an operator reference",
                    );
                }
                lookup = true;
            }
            Selector::Name { name, args } if lookup => {
                let E::Let { definitions, .. } = &current.node else {
                    return unsupported("!: name selection requires a LET expression");
                };
                let local = definitions
                    .iter()
                    .find(|definition| {
                        definition.name == *name || names.get(&definition.name) == Some(name)
                    })
                    .ok_or_else(|| CompileError {
                        message: format!("unknown LET subexpression operator {name}"),
                    })?;
                if let DefinitionKind::Operator { params } = &local.head {
                    parameters.add(
                        params,
                        (!args.is_empty() || params.is_empty()).then_some(args.as_slice()),
                    )?;
                }
                scopes.push(definitions.clone());
                current = definition_body(local);
                lookup = false;
                named = true;
                operator = true;
            }
            Selector::Name { name, args } => {
                if !named {
                    return unsupported("label selection must follow an operator or label name");
                }
                let (body, params) = label(&current, name)?;
                let params: Vec<_> = params
                    .into_iter()
                    .map(|name| Declaration {
                        name,
                        arity: 0,
                        location: current.location,
                    })
                    .collect();
                parameters.add(
                    &params,
                    (!args.is_empty() || params.is_empty()).then_some(args.as_slice()),
                )?;
                current = body;
                operator = false;
            }
            Selector::Arguments { .. } | Selector::Bound => {
                let (body, params) = bound_body(&current).ok_or_else(|| CompileError {
                    message: "!@ or !(...) requires an expression with bound names".into(),
                })?;
                parameters.add(
                    &params,
                    if let Selector::Arguments { args } = selector {
                        Some(args)
                    } else {
                        None
                    },
                )?;
                current = body;
                named = false;
            }
            _ => {
                if let E::Let { definitions, body } = &current.node {
                    index(selector, 1)?;
                    scopes.push(definitions.clone());
                    current = *body.clone();
                } else if let E::Case { arms, other } = &current.node {
                    let arm = index(selector, arms.len() + usize::from(other.is_some()))?;
                    cursor += 1;
                    let component = index(
                        selectors.get(cursor).ok_or_else(|| CompileError {
                            message: "CASE selection requires an arm and a condition/body index"
                                .into(),
                        })?,
                        2,
                    )?;
                    current = if let Some(arm) = arms.get(arm) {
                        if component == 0 {
                            arm.condition.clone()
                        } else {
                            arm.body.clone()
                        }
                    } else if component == 1 {
                        *other.clone().unwrap()
                    } else {
                        return unsupported("OTHER has no selectable condition");
                    };
                } else {
                    let operands = operands(&current)?;
                    current = operands[index(selector, operands.len())?].clone();
                }
                named = false;
                lookup = false;
            }
        }
        cursor += 1;
    }
    for scope in scopes.into_iter().rev() {
        current = Expression {
            location: current.location,
            node: E::Let {
                definitions: scope,
                body: Box::new(current),
            },
        };
    }
    let definition = Definition {
        name: helper.clone(),
        head: DefinitionKind::Operator {
            params: parameters.params,
        },
        body: current,
        is_local: false,
        is_recursive: false,
        location: base.location,
    };
    let mut result = Expression {
        location: base.location,
        node: E::Let {
            definitions: vec![definition],
            body: Box::new(Expression {
                location: base.location,
                node: E::Apply {
                    operator: helper,
                    args: parameters.arguments,
                },
            }),
        },
    };
    if !parameters.missing.is_empty() {
        result = Expression {
            location: base.location,
            node: E::Lambda {
                params: parameters.missing,
                body: Box::new(result),
            },
        };
    }
    Ok(result)
}
