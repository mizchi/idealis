use crate::Result;
use crate::analysis::{Node, analysis_children};
use crate::rewrite::identifier;
use rtla::ast::{
    self, Declaration, Definition, DefinitionKind, Expression, ExpressionKind as E, Pattern,
};
use std::collections::BTreeMap;

pub(crate) struct Function {
    pub helper: String,
    pub domain: Expression,
}

pub(crate) fn recursive(definition: &Definition) -> bool {
    if !matches!(definition.head, DefinitionKind::Function { .. }) {
        return false;
    }
    if definition.is_recursive {
        return true;
    }
    let mut pending = vec![Node::Source(&definition.body)];
    while let Some(node) = pending.pop() {
        if matches!(node, Node::Source(Expression { node: E::Identifier { name } | E::Apply { operator: name, .. }, .. }) if *name == definition.name)
        {
            return true;
        }
        pending.extend(analysis_children(node).into_iter().map(|edge| edge.node));
    }
    false
}

fn pattern_value(pattern: &Pattern, location: ast::Location) -> Expression {
    match pattern {
        Pattern::Name { name } => identifier(name, location),
        Pattern::Tuple { names } => Expression {
            location,
            node: E::Tuple {
                elements: names
                    .iter()
                    .map(|name| identifier(name, location))
                    .collect(),
            },
        },
    }
}

fn component(value: Expression, index: usize) -> Expression {
    Expression {
        location: value.location,
        node: E::FunctionApplication {
            function: Box::new(value.clone()),
            args: vec![Expression {
                location: value.location,
                node: E::Number {
                    value: index as u64 + 1,
                },
            }],
        },
    }
}

pub(crate) fn expand(
    definitions: &mut Vec<Definition>,
    fresh: &mut impl FnMut() -> Result<String>,
) -> Result<BTreeMap<String, Function>> {
    let mut functions = BTreeMap::new();
    let mut generated = vec![];
    for definition in definitions
        .iter_mut()
        .filter(|definition| recursive(definition))
    {
        let DefinitionKind::Function { bounds } = &definition.head else {
            unreachable!()
        };
        let key = fresh()?;
        let helper = fresh()?;
        let location = definition.location;
        let components: Vec<_> = bounds
            .iter()
            .map(|bound| pattern_value(&bound.pattern, location))
            .collect();
        let construction_key = if components.len() == 1 {
            components[0].clone()
        } else {
            Expression {
                location,
                node: E::Tuple {
                    elements: components,
                },
            }
        };
        let domain = if bounds.len() == 1 {
            bounds[0].set.clone()
        } else {
            Expression {
                location,
                node: E::SetMap {
                    expression: Box::new(construction_key.clone()),
                    bounds: bounds.clone(),
                },
            }
        };
        let mut bindings = vec![];
        for (index, bound) in bounds.iter().enumerate() {
            let value = if bounds.len() == 1 {
                identifier(&key, location)
            } else {
                component(identifier(&key, location), index)
            };
            let names = match &bound.pattern {
                Pattern::Name { name } => std::slice::from_ref(name),
                Pattern::Tuple { names } => names.as_slice(),
            };
            for (index, name) in names.iter().enumerate() {
                bindings.push(Definition {
                    name: name.clone(),
                    head: DefinitionKind::Operator { params: vec![] },
                    body: if matches!(bound.pattern, Pattern::Tuple { .. }) {
                        component(value.clone(), index)
                    } else {
                        value.clone()
                    },
                    is_local: false,
                    is_recursive: false,
                    location,
                });
            }
        }
        let body = Expression {
            location,
            node: E::Let {
                definitions: bindings,
                body: Box::new(definition.body.clone()),
            },
        };
        let condition = Expression {
            location,
            node: E::Apply {
                operator: "\\in".into(),
                args: vec![identifier(&key, location), domain.clone()],
            },
        };
        generated.push(Definition {
            name: helper.clone(),
            head: DefinitionKind::Operator {
                params: vec![Declaration {
                    name: key,
                    arity: 0,
                    location,
                }],
            },
            body: Expression {
                location,
                node: E::Case {
                    arms: vec![ast::CaseArm { condition, body }],
                    other: None,
                },
            },
            is_local: false,
            is_recursive: true,
            location,
        });
        let body = Expression {
            location,
            node: E::Apply {
                operator: helper.clone(),
                args: vec![construction_key],
            },
        };
        definition.body = Expression {
            location,
            node: E::FunctionConstruction {
                bounds: bounds.clone(),
                body: Box::new(body),
            },
        };
        definition.head = DefinitionKind::Operator { params: vec![] };
        definition.is_recursive = false;
        functions.insert(definition.name.clone(), Function { helper, domain });
    }
    definitions.extend(generated);
    Ok(functions)
}

pub(crate) fn rewrite(value: &mut Expression, functions: &BTreeMap<String, Function>) -> bool {
    if let E::FunctionApplication { function, args } = &value.node
        && let E::Identifier { name } = &function.node
        && let Some(function) = functions.get(name)
    {
        let key = if args.len() == 1 {
            args[0].clone()
        } else {
            Expression {
                location: value.location,
                node: E::Tuple {
                    elements: args.clone(),
                },
            }
        };
        value.node = E::Apply {
            operator: function.helper.clone(),
            args: vec![key],
        };
        return true;
    }
    if let E::Apply { operator, args } = &value.node
        && operator == "DOMAIN"
        && args.len() == 1
        && let E::Identifier { name } = &args[0].node
        && let Some(function) = functions.get(name)
    {
        *value = function.domain.clone();
        return true;
    }
    false
}

pub(crate) fn rewrite_tree(
    value: &mut Expression,
    functions: &BTreeMap<String, Function>,
    depth: usize,
) -> Result<()> {
    if depth >= 64 {
        return crate::unsupported("recursive function normalization depth exceeds 64");
    }
    rewrite(value, functions);
    for child in crate::source::children_mut(value) {
        rewrite_tree(child, functions, depth + 1)?;
    }
    Ok(())
}
