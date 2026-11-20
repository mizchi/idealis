use crate::analysis::{Node, analysis_children};
use rtla::ast::{Declaration, Definition, DefinitionKind, Expression, ExpressionKind as E};
use std::collections::{BTreeMap, BTreeSet};

fn zero_arity(definition: &Definition) -> bool {
    !matches!(&definition.head, DefinitionKind::Operator { params } if !params.is_empty())
}

fn contains_except(value: &Expression, definitions: &BTreeMap<String, Definition>) -> bool {
    let mut pending = vec![Node::Source(value)];
    let mut visited = BTreeSet::new();
    while let Some(node) = pending.pop() {
        if matches!(
            node,
            Node::Source(Expression {
                node: E::Except { .. },
                ..
            })
        ) {
            return true;
        }
        if let Node::Source(Expression {
            node: E::Identifier { name } | E::Apply { operator: name, .. },
            ..
        }) = node
            && let Some(definition) = definitions.get(name)
            && visited.insert(name)
        {
            pending.push(Node::SourceDefinition(definition));
        }
        pending.extend(analysis_children(node).into_iter().map(|edge| edge.node));
    }
    false
}

pub(crate) fn needed(value: &Expression, definitions: &BTreeMap<String, Definition>) -> bool {
    let E::Let {
        definitions: local,
        body,
    } = &value.node
    else {
        return false;
    };
    if !local.iter().any(|definition| {
        zero_arity(definition) && crate::free_old(&crate::definition_body(definition))
    }) {
        return false;
    }
    let mut environment = definitions.clone();
    environment.extend(
        local
            .iter()
            .cloned()
            .map(|definition| (definition.name.clone(), definition)),
    );
    contains_except(body, &environment)
}

fn replace(value: &mut Expression, name: &str) {
    match &mut value.node {
        E::ExceptAt => value.node = E::Identifier { name: name.into() },
        E::Except { function, updates } => {
            replace(function, name);
            for update in updates {
                for path in &mut update.path {
                    if let rtla::ast::ExceptPath::Index { args } = path {
                        for argument in args {
                            replace(argument, name);
                        }
                    }
                }
            }
        }
        E::Let { definitions, body } => {
            for definition in definitions
                .iter_mut()
                .filter(|definition| zero_arity(definition))
            {
                replace_definition(definition, name);
            }
            replace(body, name);
        }
        _ => {
            for child in crate::source::children_mut(value) {
                replace(child, name);
            }
        }
    }
}

fn replace_definition(definition: &mut Definition, name: &str) {
    if let DefinitionKind::Function { bounds } = &mut definition.head {
        for bound in bounds {
            replace(&mut bound.set, name);
        }
    }
    replace(&mut definition.body, name);
}

pub(crate) fn capture(value: &mut Expression, helper: String, parameter: String) {
    let location = value.location;
    let mut body = value.clone();
    let E::Let { definitions, .. } = &mut body.node else {
        unreachable!()
    };
    for definition in definitions
        .iter_mut()
        .filter(|definition| zero_arity(definition))
    {
        replace_definition(definition, &parameter);
    }
    value.node = E::Let {
        definitions: vec![Definition {
            name: helper.clone(),
            head: DefinitionKind::Operator {
                params: vec![Declaration {
                    name: parameter,
                    arity: 0,
                    location,
                }],
            },
            body,
            is_local: false,
            is_recursive: false,
            location,
        }],
        body: Box::new(Expression {
            node: E::Apply {
                operator: helper,
                args: vec![Expression {
                    node: E::ExceptAt,
                    location,
                }],
            },
            location,
        }),
    };
}
