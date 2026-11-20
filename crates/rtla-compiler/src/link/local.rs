use super::{Kind, Linked, Linker, Symbol};
use crate::rewrite::identifier;
use crate::{Result, source, unsupported};
use rtla::ast::{self, Definition, DefinitionKind, Expression, ExpressionKind as E, Pattern};
use std::collections::{BTreeMap, BTreeSet};

fn bind(pattern: &Pattern, names: &mut BTreeMap<String, usize>) {
    match pattern {
        Pattern::Name { name } => {
            names.insert(name.clone(), 0);
        }
        Pattern::Tuple { names: entries } => {
            for name in entries {
                names.insert(name.clone(), 0);
            }
        }
    }
}

impl Linker<'_> {
    pub(super) fn expand_local_definition(
        &mut self,
        definition: &mut Definition,
        outer: &Linked,
        visible: &BTreeMap<String, usize>,
        imports: &BTreeSet<String>,
        extra: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<()> {
        let mut names = visible.clone();
        match &mut definition.head {
            DefinitionKind::Operator { params } => {
                for param in params {
                    names.insert(param.name.clone(), param.arity);
                }
            }
            DefinitionKind::Function { bounds } => {
                for bound in bounds {
                    self.expand_local_expression(
                        &mut bound.set,
                        outer,
                        visible,
                        imports,
                        extra,
                        depth + 1,
                    )?;
                    bind(&bound.pattern, &mut names);
                }
            }
        }
        self.expand_local_expression(
            &mut definition.body,
            outer,
            &names,
            imports,
            extra,
            depth + 1,
        )
    }

    pub(super) fn expand_local_expression(
        &mut self,
        value: &mut Expression,
        outer: &Linked,
        visible: &BTreeMap<String, usize>,
        imports: &BTreeSet<String>,
        extra: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<()> {
        if depth > 64 {
            return unsupported("module AST depth exceeds 64");
        }
        match &mut value.node {
            E::Let { definitions, .. } | E::LetModule { definitions, .. } => {
                let mut names = visible.clone();
                for definition in definitions.iter() {
                    names.insert(
                        definition.name.clone(),
                        match &definition.head {
                            DefinitionKind::Operator { params } => params.len(),
                            DefinitionKind::Function { .. } => 0,
                        },
                    );
                }
                let count = definitions.len();
                if let E::LetModule {
                    definitions,
                    instances,
                    ..
                } = &mut value.node
                {
                    let mut sandbox = outer.clone();
                    for (name, arity) in &names {
                        sandbox.symbols.insert(
                            name.clone(),
                            Symbol {
                                captured: false,
                                id: name.clone(),
                                kind: Kind::Lexical,
                                arity: *arity,
                                local: true,
                            },
                        );
                    }
                    let previous: BTreeSet<_> = sandbox.definitions.keys().cloned().collect();
                    let symbols: BTreeSet<_> = sandbox.symbols.keys().cloned().collect();
                    let namespaces: BTreeSet<_> = sandbox.namespaces.keys().cloned().collect();
                    let mut local_imports = imports.clone();
                    for instance in instances.iter() {
                        self.instantiate(instance, &mut sandbox, &mut local_imports)?;
                    }
                    for (name, definition) in &sandbox.definitions {
                        if previous.contains(name) {
                            continue;
                        }
                        self.rewrite.translated.insert(name.clone());
                        definitions.push(definition.clone());
                    }
                    for (name, symbol) in &sandbox.symbols {
                        if symbols.contains(name) || symbol.kind != Kind::Definition {
                            continue;
                        }
                        let mut definition = sandbox.definitions[&symbol.id].clone();
                        definition.name = name.clone();
                        definition.body = match &definition.head {
                            DefinitionKind::Operator { params } => Expression {
                                location: value.location,
                                node: E::Apply {
                                    operator: symbol.id.clone(),
                                    args: params
                                        .iter()
                                        .map(|param| identifier(&param.name, param.location))
                                        .collect(),
                                },
                            },
                            DefinitionKind::Function { .. } => {
                                identifier(&symbol.id, value.location)
                            }
                        };
                        if matches!(definition.head, DefinitionKind::Function { .. }) {
                            definition.head = DefinitionKind::Operator { params: vec![] };
                        }
                        definitions.push(definition);
                        names.insert(name.clone(), symbol.arity);
                    }
                    *instances = sandbox
                        .namespaces
                        .iter()
                        .filter(|(name, _)| !namespaces.contains(*name))
                        .map(|(name, namespace)| ast::Instance {
                            name: Some(name.clone()),
                            module: "$linked".into(),
                            params: (0..namespace.arity)
                                .map(|index| ast::Declaration {
                                    name: format!("$argument{index}"),
                                    arity: 0,
                                    location: value.location,
                                })
                                .collect(),
                            substitutions: vec![],
                            is_local: namespace.local,
                            location: value.location,
                        })
                        .collect();
                    extra.extend(sandbox.extends);
                }
                let (definitions, body) = match &mut value.node {
                    E::Let { definitions, body }
                    | E::LetModule {
                        definitions, body, ..
                    } => (definitions, body),
                    _ => unreachable!(),
                };
                for definition in &mut definitions[..count] {
                    self.expand_local_definition(
                        definition,
                        outer,
                        &names,
                        imports,
                        extra,
                        depth + 1,
                    )?;
                }
                self.expand_local_expression(body, outer, &names, imports, extra, depth + 1)?;
            }
            E::Forall { bounds, body }
            | E::Exists { bounds, body }
            | E::FunctionConstruction { bounds, body }
            | E::SetMap {
                bounds,
                expression: body,
            } => {
                let mut names = visible.clone();
                for bound in bounds {
                    self.expand_local_expression(
                        &mut bound.set,
                        outer,
                        visible,
                        imports,
                        extra,
                        depth + 1,
                    )?;
                    bind(&bound.pattern, &mut names);
                }
                self.expand_local_expression(body, outer, &names, imports, extra, depth + 1)?;
            }
            E::SetFilter { bound, predicate } => {
                self.expand_local_expression(
                    &mut bound.set,
                    outer,
                    visible,
                    imports,
                    extra,
                    depth + 1,
                )?;
                let mut names = visible.clone();
                bind(&bound.pattern, &mut names);
                self.expand_local_expression(predicate, outer, &names, imports, extra, depth + 1)?;
            }
            E::Choose { pattern, set, body } => {
                if let Some(set) = set {
                    self.expand_local_expression(set, outer, visible, imports, extra, depth + 1)?;
                }
                let mut names = visible.clone();
                bind(pattern, &mut names);
                self.expand_local_expression(body, outer, &names, imports, extra, depth + 1)?;
            }
            E::Lambda { params, body } => {
                let mut names = visible.clone();
                for param in params {
                    names.insert(param.name.clone(), param.arity);
                }
                self.expand_local_expression(body, outer, &names, imports, extra, depth + 1)?;
            }
            _ => {
                for child in source::children_mut(value) {
                    self.expand_local_expression(child, outer, visible, imports, extra, depth + 1)?;
                }
            }
        }
        Ok(())
    }
}
