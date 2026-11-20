use crate::{Result, source, unsupported};
use rtla::ast::{DefinitionKind, Expression, ExpressionKind as E, Instance, Module};
use std::collections::{BTreeMap, BTreeSet};

type Modules = BTreeMap<String, (String, usize)>;

#[derive(Clone, Default)]
pub(super) struct Scope {
    pub parent: Option<String>,
    pub declarations: BTreeMap<String, (String, String)>,
    pub instances: Vec<(String, usize)>,
}

#[derive(Default)]
pub(super) struct Sources {
    pub modules: Vec<Module>,
    pub scopes: BTreeMap<String, Scope>,
}

fn reference(name: &mut String, position: usize, modules: &Modules) -> Result<()> {
    if let Some((target, declared)) = modules.get(name) {
        if *declared >= position {
            return unsupported(format!("nested module {name} is not yet in scope"));
        }
        *name = target.clone();
    }
    Ok(())
}

fn instance(instance: &mut Instance, modules: &Modules) -> Result<()> {
    reference(&mut instance.module, instance.location.start, modules)
}

fn expression(value: &mut Expression, modules: &Modules, depth: usize) -> Result<()> {
    if depth > 64 {
        return unsupported("nested module AST depth exceeds 64");
    }
    if let E::LetModule { instances, .. } = &mut value.node {
        for entry in instances {
            instance(entry, modules)?;
        }
    }
    for child in source::children_mut(value) {
        expression(child, modules, depth + 1)?;
    }
    Ok(())
}

fn visit(
    input: &Module,
    name: String,
    inherited: &Modules,
    scope: Scope,
    inherited_extends: &[String],
    result: &mut Sources,
    depth: usize,
) -> Result<()> {
    if depth > 40 || result.modules.len() >= 257 {
        return unsupported("nested module expansion exceeds 40 levels or 257 modules");
    }
    let mut module = input.clone();
    module.name = name.clone();
    module.extends.extend(inherited_extends.iter().cloned());
    module.extends.sort();
    module.extends.dedup();
    let mut modules = inherited.clone();
    let mut declared = BTreeSet::new();
    for child in &input.modules {
        if !declared.insert(&child.name) {
            return unsupported(format!("duplicate nested module {}", child.name));
        }
        modules.insert(
            child.name.clone(),
            (format!("{name}!{}", child.name), child.location.start),
        );
    }
    for dependency in &mut module.extends {
        reference(dependency, module.location.start, &modules)?;
    }
    for entry in &mut module.instances {
        instance(entry, &modules)?;
    }
    for definition in &mut module.definitions {
        if let DefinitionKind::Function { bounds } = &mut definition.head {
            for bound in bounds {
                expression(&mut bound.set, &modules, 0)?;
            }
        }
        expression(&mut definition.body, &modules, 0)?;
    }
    for assumption in &mut module.assumptions {
        expression(&mut assumption.expression, &modules, 0)?;
    }
    for child in &input.modules {
        let mut child_scope = Scope {
            parent: Some(name.clone()),
            ..scope.clone()
        };
        for (symbol, location) in module
            .constants
            .iter()
            .chain(&module.variables)
            .map(|entry| (&entry.name, entry.location))
            .chain(
                module
                    .definitions
                    .iter()
                    .map(|entry| (&entry.name, entry.location)),
            )
        {
            if location.start < child.location.start {
                child_scope
                    .declarations
                    .insert(symbol.clone(), (name.clone(), symbol.clone()));
            }
        }
        child_scope.instances.extend(
            module
                .instances
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.location.start < child.location.start)
                .map(|(index, _)| (name.clone(), index)),
        );
        visit(
            child,
            format!("{name}!{}", child.name),
            &modules,
            child_scope,
            &module.extends,
            result,
            depth + 1,
        )?;
    }
    module.modules.clear();
    if result.modules.len() >= 257 {
        return unsupported("nested module expansion exceeds 257 modules");
    }
    result.scopes.insert(name, scope);
    result.modules.push(module);
    Ok(())
}

pub(super) fn flatten(root: &Module, dependencies: &[Module]) -> Result<Sources> {
    let mut sources = Sources::default();
    for module in std::iter::once(root).chain(dependencies) {
        visit(
            module,
            module.name.clone(),
            &BTreeMap::new(),
            Scope::default(),
            &[],
            &mut sources,
            0,
        )?;
    }
    Ok(sources)
}
