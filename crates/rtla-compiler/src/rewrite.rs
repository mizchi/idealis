use crate::{Result, unsupported};
use rtla::ast::{
    Bound, Definition, DefinitionKind, ExceptPath, Expression, ExpressionKind as E, Pattern,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type Names = BTreeMap<String, Expression>;

pub(crate) fn identifier(name: &str, location: rtla::ast::Location) -> Expression {
    Expression {
        node: E::Identifier { name: name.into() },
        location,
    }
}

pub(crate) struct Rewriter {
    pub next: usize,
    pub strict: bool,
    pub imports: BTreeSet<String>,
    pub labels: BTreeMap<String, String>,
    pub depth: usize,
    pub namespaces: BTreeMap<String, usize>,
    pub translated: BTreeSet<String>,
}

impl Rewriter {
    fn qualifier_arguments(&self, name: &str) -> Result<()> {
        for (index, _) in name.match_indices('!') {
            if self
                .namespaces
                .get(&name[..index])
                .is_some_and(|arity| *arity != 0)
            {
                return unsupported(format!(
                    "missing arguments for module namespace {}",
                    &name[..index]
                ));
            }
        }
        Ok(())
    }

    fn namespace_selection(
        &mut self,
        value: &Expression,
        names: &Names,
    ) -> Result<Option<Expression>> {
        let E::Selection {
            expression,
            selectors,
        } = &value.node
        else {
            return Ok(None);
        };
        let (path, args) = match &expression.node {
            E::Identifier { name } => (name, vec![]),
            E::Apply { operator, args } => (operator, args.clone()),
            _ => return Ok(None),
        };
        let Some(arity) = self.namespaces.get(path) else {
            return Ok(None);
        };
        self.qualifier_arguments(path)?;
        if args.len() != *arity {
            return unsupported(format!("module namespace {path} expects {arity} arguments"));
        }
        let mut path = path.clone();
        let mut arguments = args;
        for (index, selector) in selectors.iter().enumerate() {
            let rtla::ast::Selector::Name { name, args } = selector else {
                return unsupported("expected a member name after module namespace");
            };
            path.push('!');
            path.push_str(name);
            arguments.extend(args.clone());
            if let Some(arity) = self.namespaces.get(&path) {
                if args.len() != *arity {
                    return unsupported(format!(
                        "module namespace {path} expects {arity} arguments"
                    ));
                }
                continue;
            }
            let Some(Expression {
                node: E::Identifier { name },
                ..
            }) = names.get(&path)
            else {
                return unsupported(format!("unknown module member {path}"));
            };
            let mut result = Expression {
                location: value.location,
                node: E::Apply {
                    operator: name.clone(),
                    args: arguments,
                },
            };
            if index + 1 < selectors.len() {
                result = Expression {
                    location: value.location,
                    node: E::Selection {
                        expression: Box::new(result),
                        selectors: selectors[index + 1..].to_vec(),
                    },
                };
            }
            self.expression(&mut result, names)?;
            return Ok(Some(result));
        }
        unsupported("module namespace is not an executable expression")
    }
    fn selection(
        &self,
        name: &str,
        args: Vec<Expression>,
        names: &Names,
        location: rtla::ast::Location,
    ) -> Option<Expression> {
        for (index, _) in name.match_indices('!').rev() {
            if let Some(expression) = names.get(&name[..index]) {
                let mut selectors: Vec<_> = name[index + 1..]
                    .split('!')
                    .map(|name| rtla::ast::Selector::Name {
                        name: name.into(),
                        args: vec![],
                    })
                    .collect();
                if let Some(rtla::ast::Selector::Name { args: actuals, .. }) = selectors.last_mut()
                {
                    *actuals = args;
                }
                return Some(Expression {
                    location,
                    node: E::Selection {
                        expression: Box::new(expression.clone()),
                        selectors,
                    },
                });
            }
        }
        None
    }
    fn imported(&self, operator: &str) -> Result<()> {
        if !self.strict {
            return Ok(());
        }
        let permitted = match operator {
            "+" | "-" | "*" | "\\div" | "%" | "^" | "<" | "<=" | "\\leq" | ">" | ">=" | "\\geq"
            | ".." | "Nat" => ["Naturals", "Integers", "Sequences", "FiniteSets"]
                .iter()
                .any(|name| self.imports.contains(*name)),
            "Int" | "-." => self.imports.contains("Integers"),
            "Seq" | "Len" | "Head" | "Tail" | "Append" | "SubSeq" | "SelectSeq" | "\\o" => {
                self.imports.contains("Sequences")
            }
            "Cardinality" => self.imports.contains("FiniteSets"),
            "SortSeq" | "Assert" => self.imports.contains("TLC"),
            "ApaFoldSet" | "ApaFoldSeqLeft" | "SetAsFun" | ":=" => {
                self.imports.contains("Apalache")
            }
            "Variant" | "VariantTag" | "VariantGetUnsafe" => self.imports.contains("Variants"),
            _ => true,
        };
        if !permitted {
            return unsupported(format!(
                "operator {operator} is not imported in its defining module"
            ));
        }
        Ok(())
    }
    pub fn fresh(&mut self) -> String {
        let name = format!("$linked{}", self.next);
        self.next += 1;
        name
    }

    fn pattern(
        &mut self,
        pattern: &mut Pattern,
        names: &mut Names,
        location: rtla::ast::Location,
    ) -> Result<()> {
        let entries = match pattern {
            Pattern::Name { name } => std::slice::from_mut(name),
            Pattern::Tuple { names } => names.as_mut_slice(),
        };
        for name in entries {
            if self.strict && names.contains_key(name) {
                return unsupported(format!("duplicate bound name {name}"));
            }
            let fresh = self.fresh();
            names.insert(name.clone(), identifier(&fresh, location));
            *name = fresh;
        }
        Ok(())
    }

    fn bounds(&mut self, bounds: &mut [Bound], names: &mut Names) -> Result<()> {
        for bound in bounds {
            self.expression(&mut bound.set, names)?;
            self.pattern(&mut bound.pattern, names, bound.set.location)?;
        }
        Ok(())
    }

    pub fn definition(&mut self, definition: &mut Definition, names: &Names) -> Result<()> {
        let mut names = names.clone();
        match &mut definition.head {
            DefinitionKind::Operator { params } => {
                for param in params {
                    if self.strict && names.contains_key(&param.name) {
                        return unsupported(format!("duplicate lexical parameter {}", param.name));
                    }
                    let fresh = self.fresh();
                    names.insert(param.name.clone(), identifier(&fresh, param.location));
                    param.name = fresh;
                }
            }
            DefinitionKind::Function { bounds } => self.bounds(bounds, &mut names)?,
        }
        self.expression(&mut definition.body, &names)
    }

    pub fn expression(&mut self, value: &mut Expression, names: &Names) -> Result<()> {
        if self.depth >= 64 {
            return unsupported("module AST depth exceeds 64");
        }
        self.depth += 1;
        let result = self.expression_inner(value, names);
        self.depth -= 1;
        result
    }

    fn let_definitions(
        &mut self,
        definitions: &mut [Definition],
        body: &mut Expression,
        names: &Names,
    ) -> Result<()> {
        let mut names = names.clone();
        for definition in definitions.iter_mut() {
            let standard = match definition.name.as_str() {
                "SelectSeq" => self.imports.contains("Sequences"),
                "SortSeq" | "Assert" => self.imports.contains("TLC"),
                _ => false,
            };
            if self.strict && (names.contains_key(&definition.name) || standard) {
                return unsupported(format!("duplicate LET definition {}", definition.name));
            }
            let fresh = self.fresh();
            if self.translated.contains(&definition.name) {
                self.translated.insert(fresh.clone());
            }
            let label = self
                .labels
                .get(&definition.name)
                .unwrap_or(&definition.name)
                .clone();
            self.labels.insert(fresh.clone(), label);
            names.insert(
                definition.name.clone(),
                identifier(&fresh, definition.location),
            );
            definition.name = fresh;
        }
        for definition in definitions {
            let strict = self.strict;
            self.strict &= !self.translated.contains(&definition.name);
            let result = self.definition(definition, &names);
            self.strict = strict;
            result?;
        }
        self.expression(body, &names)?;
        Ok(())
    }

    fn expression_inner(&mut self, value: &mut Expression, names: &Names) -> Result<()> {
        if let Some(result) = self.namespace_selection(value, names)? {
            *value = result;
            return Ok(());
        }
        match &mut value.node {
            E::Identifier { name } => {
                if let Some(replacement) = names.get(name) {
                    *value = replacement.clone();
                } else if let Some(selection) = self.selection(name, vec![], names, value.location)
                {
                    *value = selection;
                } else if self.strict
                    && !["TRUE", "FALSE", "BOOLEAN", "STRING", "Nat", "Int"]
                        .contains(&name.as_str())
                    && !matches!(
                        name.as_str(),
                        "DOMAIN"
                            | "SUBSET"
                            | "UNION"
                            | "ENABLED"
                            | "UNCHANGED"
                            | "Len"
                            | "Head"
                            | "Tail"
                            | "Append"
                            | "Cardinality"
                            | "SetAsFun"
                    )
                    && name.starts_with(char::is_alphabetic)
                {
                    return unsupported(format!("unresolved module identifier {name}"));
                } else {
                    self.imported(name)?;
                }
            }
            E::Apply { operator, args } => {
                self.qualifier_arguments(operator)?;
                for argument in args.iter_mut() {
                    self.expression(argument, names)?;
                }
                if let Some(replacement) = names.get(operator) {
                    if let E::Identifier { name } = &replacement.node {
                        *operator = name.clone();
                    } else if let E::Apply {
                        operator: target,
                        args: leading,
                    } = &replacement.node
                    {
                        let mut arguments = leading.clone();
                        arguments.append(args);
                        *operator = target.clone();
                        *args = arguments;
                    } else if let E::Lambda { params, body } = &replacement.node {
                        if params.len() != args.len() {
                            return unsupported(
                                "INSTANCE operator replacement has the wrong arity",
                            );
                        }
                        let name = self.fresh();
                        let call = Expression {
                            location: value.location,
                            node: E::Apply {
                                operator: name.clone(),
                                args: args.clone(),
                            },
                        };
                        let definition = Definition {
                            name,
                            head: DefinitionKind::Operator {
                                params: params.clone(),
                            },
                            body: *body.clone(),
                            is_local: false,
                            is_recursive: false,
                            location: value.location,
                        };
                        value.node = E::Let {
                            definitions: vec![definition],
                            body: Box::new(call),
                        };
                    } else if args.is_empty() {
                        *value = replacement.clone();
                    } else {
                        return unsupported("INSTANCE replacement is not an operator");
                    }
                } else if let Some(selection) =
                    self.selection(operator, args.clone(), names, value.location)
                {
                    *value = selection;
                } else if self.strict
                    && operator.starts_with(char::is_alphabetic)
                    && ![
                        "Cardinality",
                        "DOMAIN",
                        "SUBSET",
                        "UNION",
                        "Len",
                        "Head",
                        "Tail",
                        "Append",
                        "SubSeq",
                        "SelectSeq",
                        "SortSeq",
                        "Assert",
                        "Seq",
                        "ApaFoldSet",
                        "ApaFoldSeqLeft",
                        "SetAsFun",
                        "Variant",
                        "VariantTag",
                        "VariantGetUnsafe",
                    ]
                    .contains(&operator.as_str())
                {
                    return unsupported(format!("unresolved module operator {operator}"));
                } else {
                    self.imported(operator)?;
                }
            }
            E::Lambda { params, body } => {
                let mut names = names.clone();
                for param in params {
                    if self.strict && names.contains_key(&param.name) {
                        return unsupported(format!("duplicate lexical parameter {}", param.name));
                    }
                    let fresh = self.fresh();
                    names.insert(param.name.clone(), identifier(&fresh, param.location));
                    param.name = fresh;
                }
                self.expression(body, &names)?;
            }
            E::LetModule {
                definitions,
                instances,
                body,
                ..
            } => {
                if instances
                    .iter()
                    .any(|instance| instance.module != "$linked")
                {
                    return unsupported("unresolved LET module definitions");
                }
                let previous = self.namespaces.clone();
                for instance in instances {
                    if let Some(name) = &instance.name {
                        self.namespaces.insert(name.clone(), instance.params.len());
                    }
                }
                let result = self.let_definitions(definitions, body, names);
                self.namespaces = previous;
                result?;
                value.node = E::Let {
                    definitions: definitions.clone(),
                    body: body.clone(),
                };
            }
            E::AssumeProve { .. } => {
                return unsupported("ASSUME/PROVE is a proof statement, not an executable value");
            }
            E::Selection {
                expression,
                selectors,
            } => {
                self.expression(expression, names)?;
                for selector in selectors {
                    if let rtla::ast::Selector::Name { args, .. }
                    | rtla::ast::Selector::Arguments { args } = selector
                    {
                        for argument in args {
                            self.expression(argument, names)?;
                        }
                    }
                }
            }
            E::And { operands, .. } | E::Or { operands, .. } => {
                for operand in operands {
                    self.expression(operand, names)?;
                }
            }
            E::Prime { expression }
            | E::Unchanged { expression }
            | E::Enabled { expression }
            | E::Always { expression }
            | E::Eventually { expression } => self.expression(expression, names)?,
            E::If {
                condition,
                consequent,
                alternative,
            } => {
                self.expression(condition, names)?;
                self.expression(consequent, names)?;
                self.expression(alternative, names)?;
            }
            E::Case { arms, other } => {
                for arm in arms {
                    self.expression(&mut arm.condition, names)?;
                    self.expression(&mut arm.body, names)?;
                }
                if let Some(other) = other {
                    self.expression(other, names)?;
                }
            }
            E::Let { definitions, body } => self.let_definitions(definitions, body, names)?,
            E::Forall { bounds, body }
            | E::Exists { bounds, body }
            | E::FunctionConstruction { bounds, body } => {
                let mut names = names.clone();
                self.bounds(bounds, &mut names)?;
                self.expression(body, &names)?;
            }
            E::UnboundedQuantifier { .. } => return unsupported("unbounded module quantification"),
            E::Choose { pattern, set, body } => {
                if let Some(set) = set {
                    self.expression(set, names)?;
                }
                let mut names = names.clone();
                self.pattern(pattern, &mut names, value.location)?;
                self.expression(body, &names)?;
            }
            E::SetEnumeration { elements } | E::Tuple { elements } => {
                for element in elements {
                    self.expression(element, names)?;
                }
            }
            E::SetFilter { bound, predicate } => {
                let mut names = names.clone();
                self.bounds(std::slice::from_mut(bound), &mut names)?;
                self.expression(predicate, &names)?;
            }
            E::SetMap { expression, bounds } => {
                let mut names = names.clone();
                self.bounds(bounds, &mut names)?;
                self.expression(expression, &names)?;
            }
            E::FunctionSet { domain, range } => {
                self.expression(domain, names)?;
                self.expression(range, names)?;
            }
            E::Record { fields } | E::RecordSet { fields } => {
                for field in fields {
                    self.expression(&mut field.value, names)?;
                }
            }
            E::Except { function, updates } => {
                self.expression(function, names)?;
                for update in updates {
                    for path in &mut update.path {
                        if let ExceptPath::Index { args } = path {
                            for argument in args {
                                self.expression(argument, names)?;
                            }
                        }
                    }
                    self.expression(&mut update.value, names)?;
                }
            }
            E::FunctionApplication { function, args } => {
                self.expression(function, names)?;
                for argument in args {
                    self.expression(argument, names)?;
                }
            }
            E::Bracket { args } => {
                for argument in args {
                    self.expression(argument, names)?;
                }
            }
            E::FieldAccess { record, .. } => self.expression(record, names)?,
            E::Product { factors } => {
                for factor in factors {
                    self.expression(factor, names)?;
                }
            }
            E::BoxAction { action, subscript }
            | E::AngleAction { action, subscript }
            | E::WeakFairness { action, subscript }
            | E::StrongFairness { action, subscript } => {
                self.expression(action, names)?;
                self.expression(subscript, names)?;
            }
            E::Label { body, params, .. } => {
                for param in params {
                    if let Some(replacement) = names.get(param) {
                        let E::Identifier { name } = &replacement.node else {
                            return unsupported("label parameter is not a lexical name");
                        };
                        *param = name.clone();
                    }
                }
                self.expression(body, names)?;
            }
            E::Number { .. }
            | E::Integer { .. }
            | E::Decimal { .. }
            | E::String { .. }
            | E::ExceptAt => {}
        }
        Ok(())
    }
}
