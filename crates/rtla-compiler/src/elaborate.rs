use crate::analysis::{Node, analysis_children};
use crate::rewrite::{Rewriter, identifier};
use crate::{Result, functions, select, source, unsupported};
use rtla::ast::{Definition, DefinitionKind, Expression, ExpressionKind as E, Module};
use std::collections::{BTreeMap, BTreeSet};

fn higher(definition: &Definition) -> bool {
    matches!(&definition.head, DefinitionKind::Operator { params } if params.iter().any(|param| param.arity != 0))
}

pub(crate) fn needed(module: &Module) -> bool {
    if !module.modules.is_empty() || !module.recursive.is_empty() {
        return true;
    }
    let mut pending: Vec<_> = module
        .definitions
        .iter()
        .map(Node::SourceDefinition)
        .chain(
            module
                .assumptions
                .iter()
                .map(|assumption| Node::Source(&assumption.expression)),
        )
        .collect();
    while let Some(node) = pending.pop() {
        if matches!(node, Node::SourceDefinition(definition) if higher(definition) || functions::recursive(definition))
            || matches!(node, Node::Source(value) if matches!(value.node, E::Lambda { .. } | E::Selection { .. } | E::LetModule { .. }) || matches!(&value.node, E::Identifier { name } | E::Apply { operator: name, .. } if name.contains('!')))
        {
            return true;
        }
        if let Node::Source(value) = node
            && matches!(&value.node, E::Let { definitions, .. } if definitions.iter().any(|definition| crate::free_old(&crate::definition_body(definition))))
            && crate::old::needed(
                value,
                &module
                    .definitions
                    .iter()
                    .cloned()
                    .map(|definition| (definition.name.clone(), definition))
                    .collect(),
            )
        {
            return true;
        }
        pending.extend(analysis_children(node).into_iter().map(|edge| edge.node));
    }
    false
}

struct Elaborator {
    rewrite: Rewriter,
    active: BTreeMap<(String, Vec<String>), String>,
    depth: usize,
    generated: usize,
    names: BTreeMap<String, String>,
    actions: BTreeMap<String, String>,
    functions: BTreeMap<String, functions::Function>,
}

impl Elaborator {
    fn fresh(&mut self) -> Result<String> {
        self.generated += 1;
        if self.generated > 4096 {
            return unsupported("operator specialization exceeds 4096 definitions");
        }
        Ok(format!("$special{}", self.generated))
    }

    fn arity(
        &self,
        expression: &Expression,
        definitions: &BTreeMap<String, Definition>,
    ) -> Result<usize> {
        match &expression.node {
            E::Lambda { params, .. } => Ok(params.len()),
            E::Identifier { name } => {
                if let Some(definition) = definitions.get(name) {
                    return Ok(match &definition.head {
                        DefinitionKind::Operator { params } => params.len(),
                        DefinitionKind::Function { .. } => 0,
                    });
                }
                crate::symbols::standard_arity(name).ok_or_else(|| crate::CompileError {
                    message: format!("unknown operator argument {name}"),
                })
            }
            _ => unsupported("operator argument must be a named operator or LAMBDA"),
        }
    }

    fn expression(
        &mut self,
        value: &mut Expression,
        definitions: &BTreeMap<String, Definition>,
    ) -> Result<()> {
        if self.depth >= 64 {
            return unsupported("operator elaboration depth exceeds 64");
        }
        self.depth += 1;
        let result = self.expression_inner(value, definitions);
        self.depth -= 1;
        result
    }

    fn expression_inner(
        &mut self,
        value: &mut Expression,
        definitions: &BTreeMap<String, Definition>,
    ) -> Result<()> {
        if crate::old::needed(value, definitions) {
            let helper = self.fresh()?;
            let parameter = self.fresh()?;
            crate::old::capture(value, helper, parameter);
            return self.expression(value, definitions);
        }
        if functions::rewrite(value, &self.functions) {
            return self.expression(value, definitions);
        }
        if let E::Selection {
            expression,
            selectors,
        } = &value.node
        {
            let helper = self.fresh()?;
            if let E::Identifier { name } | E::Apply { operator: name, .. } = &expression.node {
                self.actions.insert(
                    helper.clone(),
                    self.actions
                        .get(name)
                        .or_else(|| self.names.get(name))
                        .unwrap_or(name)
                        .clone(),
                );
            }
            *value = select::resolve(expression, selectors, definitions, &self.names, helper)?;
            return self.expression(value, definitions);
        }
        if let E::Apply { operator, args } = &mut value.node {
            let callback = match operator.as_str() {
                "SelectSeq" | "SortSeq" => Some(1),
                "ApaFoldSet" | "ApaFoldSeqLeft" => Some(0),
                _ => None,
            };
            if !definitions.contains_key(operator)
                && let Some(actual) = callback.and_then(|index| args.get_mut(index))
            {
                self.expression(actual, definitions)?;
                if let E::Lambda { params, body } = &actual.node {
                    let name = self.fresh()?;
                    let lambda = Definition {
                        name: name.clone(),
                        head: DefinitionKind::Operator {
                            params: params.clone(),
                        },
                        body: *body.clone(),
                        is_local: false,
                        is_recursive: false,
                        location: actual.location,
                    };
                    *actual = identifier(&name, actual.location);
                    let call = value.clone();
                    value.node = E::Let {
                        definitions: vec![lambda],
                        body: Box::new(call),
                    };
                    return self.expression(value, definitions);
                }
            }
        }
        if let E::Let {
            definitions: local,
            body,
        } = &mut value.node
        {
            let functions = functions::expand(local, &mut || self.fresh())?;
            self.functions.extend(functions);
            let mut scope = definitions.clone();
            scope.extend(
                local
                    .iter()
                    .cloned()
                    .map(|definition| (definition.name.clone(), definition)),
            );
            for definition in local.iter_mut().filter(|definition| !higher(definition)) {
                self.expression(&mut definition.body, &scope)?;
            }
            return self.expression(body, &scope);
        }
        if let E::Apply { operator, args } = &mut value.node
            && let Some(definition) = definitions
                .get(operator)
                .filter(|definition| higher(definition))
        {
            let DefinitionKind::Operator { params } = &definition.head else {
                unreachable!()
            };
            if params.len() != args.len() {
                return unsupported(format!(
                    "operator {operator} expects {} arguments",
                    params.len()
                ));
            }
            let mut mapping = BTreeMap::new();
            let mut data_params = vec![];
            let mut data_args = vec![];
            let mut generated = vec![];
            let mut scope = definitions.clone();
            let mut targets = vec![];
            for (param, actual) in params.iter().zip(args) {
                self.expression(actual, definitions)?;
                if param.arity == 0 {
                    data_params.push(param.clone());
                    data_args.push(actual.clone());
                    continue;
                }
                if self.arity(actual, definitions)? != param.arity {
                    return unsupported(format!(
                        "operator parameter {} expects arity {}",
                        param.name, param.arity
                    ));
                }
                let target = if let E::Lambda { params, body } = &actual.node {
                    let name = self.fresh()?;
                    let lambda = Definition {
                        name: name.clone(),
                        head: DefinitionKind::Operator {
                            params: params.clone(),
                        },
                        body: *body.clone(),
                        is_local: false,
                        is_recursive: false,
                        location: actual.location,
                    };
                    scope.insert(name.clone(), lambda.clone());
                    generated.push(lambda);
                    name
                } else if let E::Identifier { name } = &actual.node {
                    name.clone()
                } else {
                    return unsupported("unresolved operator argument");
                };
                mapping.insert(param.name.clone(), identifier(&target, actual.location));
                targets.push(target);
            }
            let key = (operator.clone(), targets);
            if let Some(name) = self.active.get(&key) {
                value.node = E::Apply {
                    operator: name.clone(),
                    args: data_args,
                };
                return Ok(());
            }
            let name = self.fresh()?;
            self.actions.insert(
                name.clone(),
                self.actions
                    .get(operator)
                    .or_else(|| self.names.get(operator))
                    .unwrap_or(operator)
                    .clone(),
            );
            self.active.insert(key.clone(), name.clone());
            let mut special = definition.clone();
            special.name = name.clone();
            special.head = DefinitionKind::Operator {
                params: data_params,
            };
            self.rewrite.expression(&mut special.body, &mapping)?;
            self.expression(&mut special.body, &scope)?;
            self.active.remove(&key);
            generated.push(special);
            value.node = E::Let {
                definitions: generated,
                body: Box::new(Expression {
                    node: E::Apply {
                        operator: name,
                        args: data_args,
                    },
                    location: value.location,
                }),
            };
            return Ok(());
        }
        for child in source::children_mut(value) {
            self.expression(child, definitions)?;
        }
        if let E::Apply { operator, args } = &value.node {
            let node = match (operator.as_str(), args.as_slice()) {
                ("/\\", [_, _]) => Some(E::And {
                    operands: args.clone(),
                    is_bulleted: false,
                }),
                ("\\/", [_, _]) => Some(E::Or {
                    operands: args.clone(),
                    is_bulleted: false,
                }),
                ("UNCHANGED", [expression]) => Some(E::Unchanged {
                    expression: Box::new(expression.clone()),
                }),
                ("ENABLED", [expression]) => Some(E::Enabled {
                    expression: Box::new(expression.clone()),
                }),
                _ => None,
            };
            if let Some(node) = node {
                value.node = node;
            }
        }
        Ok(())
    }
}

pub(crate) fn module(input: &Module, names: &mut BTreeMap<String, String>) -> Result<Module> {
    let mut module = input.clone();
    let mut elaborator = Elaborator {
        rewrite: Rewriter {
            next: 1_000_000,
            strict: false,
            imports: BTreeSet::new(),
            labels: BTreeMap::new(),
            depth: 0,
            namespaces: BTreeMap::new(),
            translated: BTreeSet::new(),
        },
        active: BTreeMap::new(),
        depth: 0,
        generated: 0,
        names: names.clone(),
        actions: BTreeMap::new(),
        functions: BTreeMap::new(),
    };
    elaborator.functions = functions::expand(&mut module.definitions, &mut || elaborator.fresh())?;
    let definitions = module
        .definitions
        .iter()
        .cloned()
        .map(|definition| (definition.name.clone(), definition))
        .collect();
    for definition in module
        .definitions
        .iter_mut()
        .filter(|definition| !higher(definition))
    {
        elaborator.expression(&mut definition.body, &definitions)?;
    }
    for assumption in &mut module.assumptions {
        elaborator.expression(&mut assumption.expression, &definitions)?;
    }
    names.extend(elaborator.actions);
    Ok(module)
}
