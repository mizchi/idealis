use crate::analysis::{Node, analysis_children};
use crate::{Result, unsupported};
use rtla::ast::{Definition, DefinitionKind, Expression, ExpressionKind as E, Module, Pattern};
use std::collections::BTreeSet;

fn names(pattern: &Pattern) -> &[String] {
    match pattern {
        Pattern::Name { name } => std::slice::from_ref(name),
        Pattern::Tuple { names } => names,
    }
}

#[derive(Default)]
struct Scope {
    parameters: BTreeSet<String>,
    labels: BTreeSet<String>,
    allowed: bool,
    old: bool,
}

fn definition(value: &Definition, outer: &mut Scope, depth: usize) -> Result<()> {
    let mut scope = Scope {
        allowed: true,
        old: outer.old,
        ..Scope::default()
    };
    if let DefinitionKind::Function { bounds } = &value.head {
        for bound in bounds {
            expression(&bound.set, outer, depth + 1)?;
            scope
                .parameters
                .extend(names(&bound.pattern).iter().cloned());
        }
    }
    expression(&value.body, &mut scope, depth + 1)
}

fn expression(value: &Expression, scope: &mut Scope, depth: usize) -> Result<()> {
    if depth > 64 {
        return unsupported("label scope nesting exceeds 64");
    }
    match &value.node {
        E::Label { name, params, body } => {
            if !scope.allowed {
                return unsupported("label outside an operator, assumption or theorem body");
            }
            if scope.old {
                return unsupported("labels inside EXCEPT replacements are not supported by TLC");
            }
            let parameters: BTreeSet<_> = params.iter().cloned().collect();
            if parameters.len() != params.len() || parameters != scope.parameters {
                return unsupported(format!(
                    "label {name} must list exactly the bound names in its labeling scope"
                ));
            }
            if !scope.labels.insert(name.clone()) {
                return unsupported(format!("duplicate label {name}"));
            }
            expression(
                body,
                &mut Scope {
                    allowed: true,
                    ..Scope::default()
                },
                depth + 1,
            )?;
        }
        E::Let { definitions, body }
        | E::LetModule {
            definitions, body, ..
        } => {
            for entry in definitions {
                definition(entry, scope, depth + 1)?;
            }
            if let E::LetModule { instances, .. } = &value.node {
                for instance in instances {
                    for substitution in &instance.substitutions {
                        expression(&substitution.expression, scope, depth + 1)?;
                    }
                }
            }
            expression(body, scope, depth + 1)?;
        }
        E::Forall { bounds, body }
        | E::Exists { bounds, body }
        | E::FunctionConstruction { bounds, body }
        | E::SetMap {
            bounds,
            expression: body,
        } => {
            let previous = scope.parameters.clone();
            for bound in bounds {
                expression(&bound.set, scope, depth + 1)?;
            }
            for bound in bounds {
                scope
                    .parameters
                    .extend(names(&bound.pattern).iter().cloned());
            }
            expression(body, scope, depth + 1)?;
            scope.parameters = previous;
        }
        E::Lambda { params, body } => {
            let previous = scope.parameters.clone();
            scope
                .parameters
                .extend(params.iter().map(|param| param.name.clone()));
            expression(body, scope, depth + 1)?;
            scope.parameters = previous;
        }
        E::Choose { pattern, set, body } => {
            if let Some(set) = set {
                expression(set, scope, depth + 1)?;
            }
            let previous = scope.parameters.clone();
            scope.parameters.extend(names(pattern).iter().cloned());
            expression(body, scope, depth + 1)?;
            scope.parameters = previous;
        }
        E::SetFilter { bound, predicate } => {
            expression(&bound.set, scope, depth + 1)?;
            let previous = scope.parameters.clone();
            scope
                .parameters
                .extend(names(&bound.pattern).iter().cloned());
            expression(predicate, scope, depth + 1)?;
            scope.parameters = previous;
        }
        E::UnboundedQuantifier { names, body, .. } => {
            let previous = scope.parameters.clone();
            scope.parameters.extend(names.iter().cloned());
            expression(body, scope, depth + 1)?;
            scope.parameters = previous;
        }
        E::Except { function, updates } => {
            expression(function, scope, depth + 1)?;
            for update in updates {
                for path in &update.path {
                    if let rtla::ast::ExceptPath::Index { args } = path {
                        for argument in args {
                            expression(argument, scope, depth + 1)?;
                        }
                    }
                }
                let previous = scope.old;
                scope.old = true;
                expression(&update.value, scope, depth + 1)?;
                scope.old = previous;
            }
        }
        _ => {
            for edge in analysis_children(Node::Source(value)) {
                if let Node::Source(child) = edge.node {
                    expression(child, scope, depth + 1)?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn validate(module: &Module) -> Result<()> {
    for entry in &module.definitions {
        definition(entry, &mut Scope::default(), 0)?;
    }
    for assumption in &module.assumptions {
        expression(
            &assumption.expression,
            &mut Scope {
                allowed: true,
                ..Scope::default()
            },
            0,
        )?;
    }
    Ok(())
}
