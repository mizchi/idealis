use crate::analysis::{Node, analysis_children};
use crate::{Result, unsupported};
use rtla::ast::{Declaration, Definition, DefinitionKind, ExpressionKind as E, Module};

fn recursive(declarations: &[Declaration], definitions: &[Definition]) -> Result<()> {
    let mut names = std::collections::BTreeSet::new();
    for declaration in declarations {
        if !names.insert(&declaration.name) {
            return unsupported("duplicate RECURSIVE declaration");
        }
        let Some(definition) = definitions
            .iter()
            .find(|definition| definition.name == declaration.name)
        else {
            return unsupported(format!(
                "RECURSIVE operator {} has no definition in the same scope",
                declaration.name
            ));
        };
        let arity = match &definition.head {
            DefinitionKind::Operator { params } => params.len(),
            DefinitionKind::Function { .. } => 0,
        };
        if arity != declaration.arity || declaration.location.start >= definition.location.start {
            return unsupported(format!(
                "RECURSIVE declaration for {} must precede its definition and have matching arity",
                declaration.name
            ));
        }
    }
    Ok(())
}

pub(crate) fn standard_arity(name: &str) -> Option<usize> {
    Some(match name {
        "+" | "-" | "*" | "/" | "\\div" | "%" | "^" | "=" | "#" | "<" | ">" | "<=" | ">="
        | ".." | "\\in" | "\\notin" | "\\subseteq" | "\\cup" | "\\cap" | "\\" | "\\o" | "=>"
        | "<=>" | "/\\" | "\\/" | "Append" | "SelectSeq" | "SortSeq" | "Assert" => 2,
        "~" | "-." | "DOMAIN" | "SUBSET" | "UNION" | "Len" | "Head" | "Tail" | "Cardinality"
        | "ENABLED" | "UNCHANGED" | "[]" | "<>" => 1,
        "SubSeq" | "ApaFoldSet" | "ApaFoldSeqLeft" => 3,
        _ => return None,
    })
}

pub(crate) fn validate(module: &Module) -> Result<()> {
    let mut pending = vec![(module, 0)];
    while let Some((module, depth)) = pending.pop() {
        if depth > 40 {
            return unsupported("module nesting exceeds 40");
        }
        validate_module(module)?;
        pending.extend(module.modules.iter().map(|module| (module, depth + 1)));
    }
    Ok(())
}

fn validate_module(module: &Module) -> Result<()> {
    recursive(&module.recursive, &module.definitions)?;
    // These names are declared by the Sequences module in tla2tools v1.7.4.
    if module.extends.iter().any(|name| name == "Sequences") {
        for (name, at) in module
            .definitions
            .iter()
            .map(|d| (&d.name, d.location))
            .chain(
                module
                    .variables
                    .iter()
                    .chain(&module.constants)
                    .map(|d| (&d.name, d.location)),
            )
        {
            if matches!(
                name.as_str(),
                "Seq" | "Len" | "Append" | "Head" | "Tail" | "SubSeq" | "SelectSeq"
            ) {
                return unsupported(format!(
                    "Multiply-defined symbol '{name}' in module {}, line {}, column {}: conflicts with module Sequences",
                    module.name, at.line, at.column
                ));
            }
        }
    }
    let conflict = |name: &str| {
        let builtin = matches!(
            name,
            "STRING"
                | "TRUE"
                | "FALSE"
                | "BOOLEAN"
                | "="
                | "#"
                | "'"
                | "~"
                | "/\\"
                | "\\/"
                | "<=>"
                | "=>"
                | "SUBSET"
                | "UNION"
                | "DOMAIN"
                | "\\subseteq"
                | "\\in"
                | "\\notin"
                | "\\"
                | "\\cap"
                | "\\cup"
                | "\\X"
                | "~>"
                | "[]"
                | "<>"
                | "ENABLED"
                | "UNCHANGED"
                | "\\cdot"
                | "-+->"
        );
        let imported = module.extends.iter().any(|module| match module.as_str() {
            "Naturals" | "Integers" => {
                matches!(
                    name,
                    "+" | "-" | "*" | "^" | "<" | ">" | "<=" | ">=" | "%" | "\\div" | ".."
                ) || module == "Integers" && name == "-."
            }
            "Sequences" => name == "\\o",
            "TLC" => matches!(name, ":>" | "@@"),
            _ => false,
        });
        if builtin || imported {
            unsupported(format!("builtin or imported symbol {name}"))
        } else {
            Ok(())
        }
    };
    for declaration in module.constants.iter().chain(&module.variables) {
        conflict(&declaration.name)?;
    }
    for instance in &module.instances {
        for param in &instance.params {
            conflict(&param.name)?;
        }
    }
    let mut has_labels = false;
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
        .chain(
            module
                .theorems
                .iter()
                .map(|theorem| Node::Source(&theorem.statement)),
        )
        .chain(module.instances.iter().flat_map(|instance| {
            instance
                .substitutions
                .iter()
                .map(|substitution| Node::Source(&substitution.expression))
        }))
        .collect();
    while let Some(node) = pending.pop() {
        if let Node::SourceDefinition(definition) = node {
            conflict(&definition.name)?;
            if let DefinitionKind::Operator { params } = &definition.head {
                for parameter in params {
                    conflict(&parameter.name)?;
                }
            }
        }
        if let Node::Source(value) = node {
            match &value.node {
                E::Lambda { params, .. } => {
                    for param in params {
                        conflict(&param.name)?;
                    }
                }
                E::LetModule {
                    recursive: declarations,
                    definitions,
                    instances,
                    ..
                } => {
                    recursive(declarations, definitions)?;
                    for instance in instances {
                        for param in &instance.params {
                            conflict(&param.name)?;
                        }
                    }
                }
                E::Label { .. } => has_labels = true,
                _ => {}
            }
        }
        pending.extend(analysis_children(node).into_iter().map(|edge| edge.node));
    }
    if has_labels {
        crate::labels::validate(module)?;
    }
    Ok(())
}
