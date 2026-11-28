use crate::analysis::{Node, analysis_children};
use crate::unsupported;
use crate::{CompileError, Compiler, Context, E, Expression, Phase, Result, Target, typed};
use rtla::ast::{self, ConfigValue, DefinitionKind, Pattern};
use rtlc::ir::{self, Domain, Expression as I, Scalar};
use std::collections::BTreeMap;
fn boolean(value: bool) -> I {
    I::Literal {
        value: Scalar::Bool(value),
    }
}
fn integer(value: i64) -> Result<I> {
    Ok(I::Literal {
        value: Scalar::Int(i32::try_from(value).map_err(|_| CompileError {
            message: "integer outside signed 32 bits".into(),
        })?),
    })
}
fn string(value: &str) -> I {
    I::Data {
        value: rtlc::Value::String(value.into()),
    }
}
fn literal(value: &ConfigValue) -> Result<I> {
    Ok(match value {
        ConfigValue::Number { value } => integer(*value)?,
        ConfigValue::Boolean { value } => boolean(*value),
        ConfigValue::String { value } => string(value),
        ConfigValue::Set { elements } => I::Set {
            values: elements.iter().map(literal).collect::<Result<_>>()?,
        },
        ConfigValue::ModelValue { name } => I::Data {
            value: rtlc::Value::ModelValue(name.clone()),
        },
    })
}
fn not(argument: I) -> I {
    I::Not {
        argument: Box::new(argument),
    }
}
fn unmatched(value: &Expression) -> I {
    I::UnmatchedCase {
        location: format!(
            "line {}, column {}",
            value.location.line, value.location.column
        ),
    }
}
pub(crate) fn same_kind(value: &Expression, and: bool) -> Option<&[Expression]> {
    match &value.node {
        E::And { operands, .. } if and => Some(operands),
        E::Or { operands, .. } if !and => Some(operands),
        _ => None,
    }
}
impl Compiler<'_> {
    pub(super) fn expression_ir(&mut self, value: &Expression, context: &Context) -> Result<I> {
        let child = context.nested()?;
        Ok(match &value.node {
            E::Number { value } => integer(i64::try_from(*value).map_err(|_| CompileError {
                message: "integer outside signed 32 bits".into(),
            })?)?,
            E::String { value } => {
                if value.chars().any(|character| character as u32 > 255) {
                    return unsupported("TLC 1.7.4 string literal outside byte range");
                }
                string(value)
            }
            E::Identifier { name } => {
                if let Some(local) = context.locals.get(name) {
                    return Ok(local.clone());
                }
                if let Some(constant) = self.constants.get(name) {
                    return literal(constant);
                }
                if name == "TRUE" || name == "FALSE" {
                    return Ok(boolean(name == "TRUE"));
                }
                if name == "BOOLEAN" {
                    return Ok(I::Set {
                        values: vec![boolean(false), boolean(true)],
                    });
                }
                if let Some(slot) = self.slots.get(name) {
                    if context.phase == Phase::Constant {
                        return unsupported("variable in constant expression");
                    }
                    return Ok(I::Read {
                        slot: *slot,
                        next: context.phase == Phase::Initial,
                    });
                }
                self.shared_purpose_ir(name, &[], context, false)?
            }
            E::Let { definitions, body } => {
                let expanded = self.let_context(definitions, context)?;
                self.expression_ir(body, &expanded)?
            }
            E::Label { body, .. } => self.expression_ir(body, &child)?,
            E::Prime { expression } if context.phase == Phase::Next => {
                let inner = self.expression_ir(expression, &child)?;
                if let I::Read { slot, next: false } = inner {
                    I::Read { slot, next: true }
                } else {
                    self.prime_ir(inner)?
                }
            }
            E::Unchanged { expression } if context.phase == Phase::Next => {
                let inner = self.expression_ir(expression, &child)?;
                I::Eq {
                    left: Box::new(inner.clone()),
                    right: Box::new(self.prime_ir(inner)?),
                }
            }
            E::Enabled { expression } => I::Enabled {
                branches: self.action_ir(
                    expression,
                    &Context {
                        phase: Phase::Next,
                        ..child
                    },
                    "ENABLED",
                )?,
                subscript: None,
            },
            E::ExceptAt => context.locals.get("@").cloned().unwrap_or(I::Old),
            E::SetEnumeration { elements } | E::Tuple { elements } => {
                let values = self.expressions_ir(elements, &child)?;
                if matches!(value.node, E::Tuple { .. }) {
                    I::Tuple { values }
                } else {
                    I::Set { values }
                }
            }
            E::Record { fields } | E::RecordSet { fields } => {
                let fields = fields
                    .iter()
                    .map(|field| {
                        Ok((
                            field.name.clone(),
                            self.expression_ir(&field.value, &child)?,
                        ))
                    })
                    .collect::<Result<_>>()?;
                if matches!(value.node, E::Record { .. }) {
                    I::Record { fields }
                } else {
                    I::RecordSet { fields }
                }
            }
            E::FunctionSet { domain, range } => I::FunctionSet {
                left: Box::new(self.expression_ir(domain, &child)?),
                right: Box::new(self.expression_ir(range, &child)?),
            },
            E::FunctionApplication { function, args } => {
                let key = self.key_ir(args, &child)?;
                I::At {
                    target: Box::new(self.expression_ir(function, &child)?),
                    key: Box::new(key),
                }
            }
            E::FieldAccess { record, field } => I::At {
                target: Box::new(self.expression_ir(record, &child)?),
                key: Box::new(string(field)),
            },
            E::Except { function, updates } => {
                let mut lowered = vec![];
                for update in updates {
                    let mut path = vec![];
                    for part in &update.path {
                        path.push(match part {
                            ast::ExceptPath::Field { name } => string(name),
                            ast::ExceptPath::Index { args } => self.key_ir(args, &child)?,
                        });
                    }
                    let mut old = child.clone();
                    old.locals.insert("@".into(), I::Old);
                    old.order.retain(|name| name != "@");
                    old.order.push("@".into());
                    let binding =
                        self.binding_at(context.origin, update.value.location.start, 2, 0);
                    old.binding_ids.insert("@".into(), binding);
                    old.projection.insert(binding, I::Old);
                    lowered.push(ir::Update {
                        path,
                        value: self.expression_ir(&update.value, &old)?,
                    });
                }
                I::Update {
                    target: Box::new(self.expression_ir(function, &child)?),
                    updates: lowered,
                }
            }
            E::Forall { bounds, body }
            | E::Exists { bounds, body }
            | E::FunctionConstruction { bounds, body } => {
                let (bounds, bound) = self.bind_ir(bounds, &child)?;
                let body = Box::new(self.expression_ir(body, &bound)?);
                match value.node {
                    E::Forall { .. } => I::Forall { bounds, body },
                    E::Exists { .. } => I::Exists { bounds, body },
                    _ => I::Function { bounds, body },
                }
            }
            E::Product { factors } if factors.len() >= 2 => {
                self.expression_ir(&crate::product::expand(factors, value.location), context)?
            }
            E::SetMap { bounds, expression } => {
                let (bounds, bound) = self.bind_ir(bounds, &child)?;
                I::MapSet {
                    bounds,
                    body: Box::new(self.expression_ir(expression, &bound)?),
                }
            }
            E::SetFilter { bound, predicate } => {
                let (bounds, bound) = self.bind_ir(std::slice::from_ref(bound), &child)?;
                I::Filter {
                    bounds,
                    body: Box::new(self.expression_ir(predicate, &bound)?),
                }
            }
            E::Choose {
                pattern,
                set: Some(set),
                body,
            } => {
                let (mut bounds, bound) = self.bind_ir(
                    &[ast::Bound {
                        pattern: pattern.clone(),
                        set: *set.clone(),
                    }],
                    &child,
                )?;
                I::Choose {
                    binding: bounds.remove(0),
                    body: Box::new(self.expression_ir(body, &bound)?),
                }
            }
            E::If { .. } => self.else_if_ir(value, &child)?,
            E::Case { arms, other } => {
                let mut result = if let Some(other) = other {
                    self.expression_ir(other, &child)?
                } else if !arms.is_empty() {
                    unmatched(value)
                } else {
                    return unsupported("empty CASE");
                };
                for arm in arms.iter().rev() {
                    result = I::If {
                        condition: Box::new(self.expression_ir(&arm.condition, &child)?),
                        consequent: Box::new(self.expression_ir(&arm.body, &child)?),
                        alternative: Box::new(result),
                    };
                }
                result
            }
            E::And { operands, .. } | E::Or { operands, .. } => {
                self.left_chain_ir(matches!(value.node, E::And { .. }), operands, &child)?
            }
            E::BoxAction { action, subscript } | E::AngleAction { action, subscript } => {
                let boxed = matches!(value.node, E::BoxAction { .. });
                let subscript = self.expression_ir(subscript, &child)?;
                let unchanged = I::Eq {
                    left: Box::new(subscript.clone()),
                    right: Box::new(self.prime_ir(subscript)?),
                };
                let left = Box::new(self.expression_ir(action, &child)?);
                if boxed {
                    I::Or {
                        left,
                        right: Box::new(unchanged),
                    }
                } else {
                    I::And {
                        left,
                        right: Box::new(not(unchanged)),
                    }
                }
            }
            E::Apply { operator, args } => self.apply_ir(operator, args, context, &child)?,
            _ => {
                return unsupported(format!(
                    "expression at {}:{}",
                    value.location.line, value.location.column
                ));
            }
        })
    }
    fn else_if_ir(&mut self, value: &Expression, child: &Context) -> Result<I> {
        let mut arms = vec![];
        let mut tail = value;
        while let E::If {
            condition,
            consequent,
            alternative,
        } = &tail.node
        {
            arms.push((
                self.expression_ir(condition, child)?,
                self.expression_ir(consequent, child)?,
            ));
            tail = alternative;
        }
        let mut result = self.expression_ir(tail, child)?;
        for (condition, consequent) in arms.into_iter().rev() {
            result = I::If {
                condition: Box::new(condition),
                consequent: Box::new(consequent),
                alternative: Box::new(result),
            };
        }
        Ok(result)
    }
    fn left_chain_ir(&mut self, and: bool, operands: &[Expression], child: &Context) -> Result<I> {
        let mut layers = vec![];
        let mut current = operands;
        loop {
            layers.push(current);
            match current.first().and_then(|first| same_kind(first, and)) {
                Some(inner) => current = inner,
                None => break,
            }
        }
        let mut result = if let Some(first) = current.first() {
            self.expression_ir(first, child)?
        } else {
            boolean(and)
        };
        for layer in layers.into_iter().rev() {
            for operand in layer.iter().skip(1) {
                let left = Box::new(result);
                let right = Box::new(self.expression_ir(operand, child)?);
                result = if and {
                    I::And { left, right }
                } else {
                    I::Or { left, right }
                };
            }
        }
        Ok(result)
    }
    fn expressions_ir(&mut self, values: &[Expression], context: &Context) -> Result<Vec<I>> {
        values
            .iter()
            .map(|value| self.expression_ir(value, context))
            .collect()
    }
    fn key_ir(&mut self, args: &[Expression], context: &Context) -> Result<I> {
        if args.len() == 1 {
            self.expression_ir(&args[0], context)
        } else {
            Ok(I::Tuple {
                values: self.expressions_ir(args, context)?,
            })
        }
    }
    pub(super) fn prime_ir(&mut self, value: I) -> Result<I> {
        self.lower_pending()?;
        if typed::level_summary(Node::Expression(&value), &self.level_cache.borrow()).base > 1 {
            return unsupported("prime of an action expression");
        }
        Ok(I::Prime {
            argument: Box::new(value),
        })
    }
    pub(super) fn shared_ir(
        &mut self,
        name: &str,
        args: &[Expression],
        context: &Context,
    ) -> Result<I> {
        self.shared_purpose_ir(name, args, context, false)
    }
    fn shared_purpose_ir(
        &mut self,
        name: &str,
        args: &[Expression],
        context: &Context,
        membership: bool,
    ) -> Result<I> {
        let child = context.nested()?;
        let demanded = self.membership_demands(name, context, membership)?;
        let position = self.scopes[context.scope][name].id;
        let arguments = args
            .iter()
            .enumerate()
            .map(|(slot, argument)| {
                if membership || demanded.contains(&self.binding_id(position, 0, slot)) {
                    self.membership_ir(argument, &child)
                } else {
                    self.expression_ir(argument, &child)
                }
            })
            .collect::<Result<_>>()?;
        let targets = self.call_targets(name, args, context)?;
        self.shared_arguments_ir(name, arguments, context, membership, targets)
    }
    pub(super) fn bind_ir(
        &mut self,
        bounds: &[ast::Bound],
        context: &Context,
    ) -> Result<(Vec<ir::Binding>, Context)> {
        let mut child = context.clone();
        let mut domain_context = context.clone();
        let mut bindings = vec![];
        for bound in bounds {
            if child.bound >= 40 {
                return unsupported("more than 40 bound variables");
            }
            let slot = child.bound;
            // Domains keep the outer lexical names but run after earlier slots are occupied.
            domain_context.bound = slot;
            let mut binding = ir::Binding {
                slot,
                tuple_size: None,
                domain: self.domain_ir(&bound.set, &domain_context, true)?,
            };
            let names = match &bound.pattern {
                Pattern::Name { name } => vec![name],
                Pattern::Tuple { names } => {
                    binding.tuple_size = Some(names.len());
                    names.iter().collect()
                }
            };
            for (index, name) in names.iter().enumerate() {
                child.order.retain(|item| item != *name);
                child.order.push((*name).clone());
                let binding_id =
                    self.named_binding_at(context.origin, bound.set.location.start, 1, index, name);
                child.binding_ids.insert((*name).clone(), binding_id);
                let value = if matches!(bound.pattern, Pattern::Tuple { .. }) {
                    I::At {
                        target: Box::new(I::Bound { slot }),
                        key: Box::new(integer(index as i64 + 1)?),
                    }
                } else {
                    I::Bound { slot }
                };
                child.projection.insert(binding_id, value.clone());
                child.locals.insert((*name).clone(), value);
            }
            child = child.nested()?;
            child.bound = slot + 1;
            bindings.push(binding);
        }
        Ok((bindings, child))
    }
    pub(super) fn domain_ir(
        &mut self,
        value: &Expression,
        context: &Context,
        choice: bool,
    ) -> Result<Domain> {
        Ok(match &value.node {
            E::SetEnumeration { elements } => Domain::Set {
                values: self.expressions_ir(elements, &context.nested()?)?,
            },
            E::Apply { operator, args } if operator == ".." && args.len() == 2 => {
                if !self.arithmetic() {
                    return unsupported("range requires Naturals");
                }
                let child = context.nested()?;
                Domain::Range {
                    lower: Box::new(self.expression_ir(&args[0], &child)?),
                    upper: Box::new(self.expression_ir(&args[1], &child)?),
                }
            }
            E::Identifier { name } if name == "BOOLEAN" => Domain::Set {
                values: vec![boolean(false), boolean(true)],
            },
            E::Identifier { name } if name == "Nat" || name == "Int" => {
                if choice || !self.arithmetic() || name == "Int" && !self.extends("Integers") {
                    return unsupported("infinite enumeration");
                }
                Domain::Range {
                    lower: Box::new(integer(if name == "Nat" { 0 } else { i32::MIN as i64 })?),
                    upper: Box::new(integer(i32::MAX as i64)?),
                }
            }
            E::Identifier { name }
                if self.has(context, name) && !context.locals.contains_key(name) =>
            {
                Domain::Value {
                    value: Box::new(self.shared_ir(name, &[], context)?),
                }
            }
            _ => Domain::Value {
                value: Box::new(self.expression_ir(value, context)?),
            },
        })
    }
    pub(super) fn membership_ir(&mut self, value: &Expression, context: &Context) -> Result<I> {
        let child = context.nested()?;
        Ok(match &value.node {
            E::Let { definitions, body } => {
                let inner = self.let_context(definitions, context)?;
                self.membership_ir(body, &inner)?
            }
            E::Label { body, .. } => self.membership_ir(body, &child)?,
            E::If { .. } => {
                let mut arms = vec![];
                let mut tail = value;
                while let E::If {
                    condition,
                    consequent,
                    alternative,
                } = &tail.node
                {
                    arms.push((
                        self.expression_ir(condition, &child)?,
                        self.membership_ir(consequent, &child)?,
                    ));
                    tail = alternative;
                }
                let mut result = self.membership_ir(tail, &child)?;
                for (condition, consequent) in arms.into_iter().rev() {
                    result = I::If {
                        condition: Box::new(condition),
                        consequent: Box::new(consequent),
                        alternative: Box::new(result),
                    };
                }
                result
            }
            E::Case { arms, other } if !arms.is_empty() => {
                let mut result = if let Some(other) = other {
                    self.membership_ir(other, &child)?
                } else {
                    unmatched(value)
                };
                for arm in arms.iter().rev() {
                    result = I::If {
                        condition: Box::new(self.expression_ir(&arm.condition, &child)?),
                        consequent: Box::new(self.membership_ir(&arm.body, &child)?),
                        alternative: Box::new(result),
                    };
                }
                result
            }
            E::Identifier { name } if name == "Nat" || name == "Int" => {
                if !self.arithmetic() || name == "Int" && !self.extends("Integers") {
                    return unsupported("infinite enumeration");
                }
                I::MapSet {
                    bounds: vec![ir::Binding {
                        slot: context.bound,
                        tuple_size: None,
                        domain: Domain::Range {
                            lower: Box::new(integer(if name == "Nat" {
                                0
                            } else {
                                i32::MIN as i64
                            })?),
                            upper: Box::new(integer(i32::MAX as i64)?),
                        },
                    }],
                    body: Box::new(I::Bound {
                        slot: context.bound,
                    }),
                }
            }
            E::Identifier { name }
                if self.has(context, name) && !context.locals.contains_key(name) =>
            {
                self.shared_purpose_ir(name, &[], context, true)?
            }
            E::Apply { operator, args } if self.has(context, operator) => {
                self.shared_purpose_ir(operator, args, context, true)?
            }
            E::Apply { operator, args }
                if operator == "Seq" && args.len() == 1 && self.extends("Sequences") =>
            {
                I::SequenceSet {
                    argument: Box::new(self.membership_ir(&args[0], &child)?),
                }
            }
            E::FunctionSet { domain, range } => I::FunctionSet {
                left: Box::new(self.expression_ir(domain, &child)?),
                right: Box::new(self.membership_ir(range, &child)?),
            },
            E::RecordSet { fields } => I::RecordSet {
                fields: fields
                    .iter()
                    .map(|field| {
                        Ok((
                            field.name.clone(),
                            self.membership_ir(&field.value, &child)?,
                        ))
                    })
                    .collect::<Result<_>>()?,
            },
            _ => self.expression_ir(value, context)?,
        })
    }
    fn membership_test_ir(&mut self, left: I, value: &Expression, context: &Context) -> Result<I> {
        if match &value.node {
            E::Identifier { name } => self.has(context, name) || context.locals.contains_key(name),
            E::Apply { operator, .. } => self.has(context, operator),
            _ => false,
        } {
            return Ok(I::FunctionIn {
                left: Box::new(left),
                right: Box::new(self.membership_ir(value, context)?),
            });
        }
        Ok(match &value.node {
            E::Apply { operator, args }
                if operator == "Seq" && args.len() == 1 && self.extends("Sequences") =>
            {
                I::SequenceIn {
                    left: Box::new(left),
                    right: Box::new(self.membership_ir(&args[0], &context.nested()?)?),
                }
            }
            E::RecordSet { .. } | E::FunctionSet { .. } => I::FunctionIn {
                left: Box::new(left),
                right: Box::new(self.membership_ir(value, &context.nested()?)?),
            },
            E::Apply { operator, args } if operator == "SUBSET" && args.len() == 1 => {
                I::FunctionIn {
                    left: Box::new(left),
                    right: Box::new(I::Powerset {
                        argument: Box::new(self.expression_ir(&args[0], context)?),
                    }),
                }
            }
            _ => I::In {
                value: Box::new(left),
                domain: self.domain_ir(value, context, false)?,
            },
        })
    }
    fn fold_ir(
        &mut self,
        sequence: bool,
        args: &[Expression],
        context: &Context,
        child: &Context,
    ) -> Result<I> {
        let E::Identifier { name } = &args[0].node else {
            return unsupported("fold operator argument");
        };
        if context.locals.contains_key(name) {
            return unsupported(format!("fold operator {name}"));
        }
        let entry = self.scopes[context.scope]
            .get(name)
            .cloned()
            .ok_or_else(|| CompileError {
                message: format!("operator {name}"),
            })?;
        if !entry.definition.is_recursive && context.call_ids.contains(&entry.id) {
            return unsupported(format!("fold operator {name}"));
        }
        let DefinitionKind::Operator { params } = &entry.definition.head else {
            return unsupported(format!("fold operator {name}"));
        };
        if params.len() != 2 || params.iter().any(|param| param.arity != 0) {
            return unsupported(format!("fold operator {name}"));
        }
        let accumulator = context.bound;
        if accumulator + 2 > 40 {
            return unsupported("more than 40 bound variables");
        }
        let initial = Box::new(self.expression_ir(&args[1], child)?);
        let collection = Box::new(self.expression_ir(&args[2], child)?);
        let body = Box::new(self.shared_arguments_ir(
            name,
            vec![
                I::Bound { slot: accumulator },
                I::Bound {
                    slot: accumulator + 1,
                },
            ],
            context,
            false,
            BTreeMap::new(),
        )?);
        Ok(I::Fold {
            accumulator,
            initial,
            collection,
            body,
            order: if sequence {
                ir::FoldOrder::SequenceLeft
            } else {
                ir::FoldOrder::SetMembers
            },
        })
    }
    fn apply_ir(
        &mut self,
        operator: &str,
        args: &[Expression],
        context: &Context,
        child: &Context,
    ) -> Result<I> {
        if self.has(context, operator) {
            return self.shared_ir(operator, args, context);
        }
        if (operator == "SelectSeq" && self.extends("Sequences"))
            || (operator == "SortSeq" && self.extends("TLC"))
        {
            if args.len() != 2 {
                return unsupported(format!("operator arity {operator}"));
            }
            let arity = if operator == "SortSeq" { 2 } else { 1 };
            let E::Identifier { name } = &args[1].node else {
                return unsupported("callback operator argument");
            };
            if context.locals.contains_key(name) || context.bound + arity > 40 {
                return unsupported(format!("callback operator {name}"));
            }
            let body = Box::new(
                self.shared_arguments_ir(
                    name,
                    (0..arity)
                        .map(|index| I::Bound {
                            slot: context.bound + index,
                        })
                        .collect(),
                    context,
                    false,
                    BTreeMap::new(),
                )?,
            );
            let sequence = Box::new(self.expression_ir(&args[0], child)?);
            return Ok(if arity == 1 {
                I::SelectSeq {
                    slot: context.bound,
                    sequence,
                    body,
                }
            } else {
                I::SortSeq {
                    slot: context.bound,
                    sequence,
                    body,
                }
            });
        }
        if operator == "Assert" && self.extends("TLC") {
            if args.len() != 2 {
                return unsupported("operator arity Assert");
            }
            return Ok(I::Assert {
                condition: Box::new(self.expression_ir(&args[0], child)?),
                message: Box::new(self.expression_ir(&args[1], child)?),
            });
        }
        if self.extends("Apalache") && ["ApaFoldSet", "ApaFoldSeqLeft"].contains(&operator) {
            if args.len() != 3 {
                return unsupported("fold arity");
            }
            return self.fold_ir(operator == "ApaFoldSeqLeft", args, context, child);
        }
        if self.extends("Apalache") && operator == "SetAsFun" {
            let [set] = args else {
                return unsupported("operator arity SetAsFun");
            };
            let expansion = crate::set_as_fun::expand(set, set.location);
            return Ok(I::If {
                condition: Box::new(self.expression_ir(&expansion.single_valued, child)?),
                consequent: Box::new(self.expression_ir(&expansion.function, child)?),
                alternative: Box::new(I::Assert {
                    condition: Box::new(boolean(false)),
                    message: Box::new(string(crate::set_as_fun::DUPLICATE_KEY)),
                }),
            });
        }
        if self.extends("Apalache") && operator == "MkSeq" {
            return self.expression_ir(&crate::mk_seq::expand(args)?, child);
        }
        if self.extends("Variants") {
            match operator {
                "Variant" if args.len() == 2 => {
                    if context.bound >= 256 {
                        return unsupported("too many bound variables");
                    }
                    let tag = self.expression_ir(&args[0], child)?;
                    let payload = self.expression_ir(
                        &args[1],
                        &Context {
                            bound: context.bound + 1,
                            ..child.clone()
                        },
                    )?;
                    return Ok(I::Function {
                        bounds: vec![ir::Binding {
                            slot: context.bound,
                            tuple_size: None,
                            domain: Domain::Set { values: vec![tag] },
                        }],
                        body: Box::new(payload),
                    });
                }
                "VariantTag" if args.len() == 1 => {
                    return Ok(I::VariantTag {
                        argument: Box::new(self.expression_ir(&args[0], child)?),
                    });
                }
                "VariantGetUnsafe" if args.len() == 2 => {
                    return Ok(I::At {
                        target: Box::new(self.expression_ir(&args[1], child)?),
                        key: Box::new(self.expression_ir(&args[0], child)?),
                    });
                }
                _ => {}
            }
        }
        if args.len() == 1 && ["-.", "-"].contains(&operator) {
            if !self.extends("Integers") {
                return unsupported("unary minus requires Integers");
            }
            if matches!(args[0].node, E::Number { value: 2147483648 }) {
                return integer(i32::MIN as i64);
            }
        }
        if args.len() == 1 && ["~", "\\lnot", "-.", "-"].contains(&operator) {
            let argument = Box::new(self.expression_ir(&args[0], child)?);
            return Ok(if ["~", "\\lnot"].contains(&operator) {
                I::Not { argument }
            } else {
                I::Neg { argument }
            });
        }
        if args.len() == 1
            && [
                "DOMAIN",
                "Cardinality",
                "Len",
                "Head",
                "Tail",
                "SUBSET",
                "UNION",
            ]
            .contains(&operator)
        {
            if ["Len", "Head", "Tail"].contains(&operator) && !self.extends("Sequences") {
                return unsupported("sequence operator requires Sequences");
            }
            if operator == "Cardinality" && !self.extends("FiniteSets") {
                return unsupported("Cardinality requires FiniteSets");
            }
            let argument = Box::new(self.expression_ir(&args[0], child)?);
            return Ok(match operator {
                "DOMAIN" => I::Domain { argument },
                "Cardinality" => I::Cardinality { argument },
                "Len" => I::Len { argument },
                "Head" => I::Head { argument },
                "Tail" => I::Tail { argument },
                "SUBSET" => I::Powerset { argument },
                "UNION" => I::Union { argument },
                _ => unreachable!(),
            });
        }
        if args.len() == 2 && ["\\in", "\\notin"].contains(&operator) {
            let left = self.expression_ir(&args[0], child)?;
            let membership = self.membership_test_ir(left, &args[1], child)?;
            return Ok(if operator == "\\notin" {
                not(membership)
            } else {
                membership
            });
        }
        if operator == "<=>" && args.len() == 2 {
            return Ok(I::Eq {
                left: Box::new(not(self.expression_ir(&args[0], child)?)),
                right: Box::new(not(self.expression_ir(&args[1], child)?)),
            });
        }
        if operator == ".." && args.len() == 2 {
            let value = Expression {
                node: E::Apply {
                    operator: operator.into(),
                    args: args.to_vec(),
                },
                location: args[0].location,
            };
            return Ok(I::MapSet {
                bounds: vec![ir::Binding {
                    slot: context.bound,
                    tuple_size: None,
                    domain: self.domain_ir(&value, child, true)?,
                }],
                body: Box::new(I::Bound {
                    slot: context.bound,
                }),
            });
        }
        if operator == "Append" && args.len() == 2 && self.extends("Sequences") {
            return Ok(I::Append {
                left: Box::new(self.expression_ir(&args[0], child)?),
                right: Box::new(self.expression_ir(&args[1], child)?),
            });
        }
        if operator == "SubSeq" && args.len() == 3 && self.extends("Sequences") {
            return Ok(I::Subseq {
                sequence: Box::new(self.expression_ir(&args[0], child)?),
                from: Box::new(self.expression_ir(&args[1], child)?),
                to: Box::new(self.expression_ir(&args[2], child)?),
            });
        }
        let op = match operator {
            "+" => "add",
            "-" => "sub",
            "*" => "mul",
            "\\div" => "div",
            "%" => "mod",
            "^" => "pow",
            "=" => "eq",
            ":=" if self.extends("Apalache") => "eq",
            "#" | "/=" => "ne",
            "<" => "lt",
            "<=" | "\\leq" => "le",
            ">" => "gt",
            ">=" | "\\geq" => "ge",
            "=>" => "implies",
            "\\cup" | "\\union" => "cup",
            "\\cap" | "\\intersect" => "cap",
            "\\" => "difference",
            "\\subseteq" => "subset-eq",
            "\\o" => "concat",
            _ => return unsupported(format!("operator {operator}")),
        };
        if args.len() != 2 {
            return unsupported(format!("operator arity {operator}"));
        }
        if [
            "add", "sub", "mul", "div", "mod", "pow", "lt", "le", "gt", "ge",
        ]
        .contains(&op)
            && !self.arithmetic()
        {
            return unsupported("arithmetic requires Naturals");
        }
        if op == "concat" && !self.extends("Sequences") {
            return unsupported("concatenation requires Sequences");
        }
        let left = Box::new(self.expression_ir(&args[0], child)?);
        let right = Box::new(self.expression_ir(&args[1], child)?);
        Ok(match op {
            "add" => I::Add { left, right },
            "sub" => I::Sub { left, right },
            "mul" => I::Mul { left, right },
            "div" => I::Div { left, right },
            "mod" => I::Mod { left, right },
            "pow" => I::Pow { left, right },
            "eq" => I::Eq { left, right },
            "ne" => I::Ne { left, right },
            "lt" => I::Lt { left, right },
            "le" => I::Le { left, right },
            "gt" => I::Gt { left, right },
            "ge" => I::Ge { left, right },
            "implies" => I::Implies { left, right },
            "cup" => I::Cup { left, right },
            "cap" => I::Cap { left, right },
            "difference" => I::Difference { left, right },
            "subset-eq" => I::SubsetEq { left, right },
            "concat" => I::Concat { left, right },
            _ => unreachable!(),
        })
    }
    pub(super) fn target_ir(target: &Target) -> Result<I> {
        Ok(match target {
            Target::Slot(slot) => I::Read {
                slot: *slot,
                next: false,
            },
            Target::Tuple(values) => I::Tuple {
                values: values.iter().map(Self::target_ir).collect::<Result<_>>()?,
            },
            Target::Binding(_) => {
                return unsupported("generative UNCHANGED requires state variables");
            }
        })
    }
    pub(super) fn variable_reads_ir(
        &mut self,
        value: I,
        message: &str,
        depth: usize,
    ) -> Result<Vec<I>> {
        if self.target_nesting >= 40 {
            return unsupported("expression or operator nesting exceeds 40");
        }
        self.target_nesting += 1;
        self.lower_pending()?;
        struct Frame {
            definition: Option<usize>,
            arguments: Vec<I>,
            captures: Vec<I>,
            parent: Option<usize>,
            depth: usize,
        }
        let mut frames: Vec<Frame> = vec![];
        let mut pending: Vec<(I, Option<usize>, usize)> = vec![(value, None, depth)];
        let mut reads = vec![];
        let mut visited = 0;
        while let Some((value, frame, depth)) = pending.pop() {
            if visited == crate::TARGET_EXPANSION_LIMIT {
                return unsupported(format!(
                    "target expansion exceeds {} nodes",
                    crate::TARGET_EXPANSION_LIMIT
                ));
            }
            visited += 1;
            if depth >= 40 {
                return unsupported("expression or operator nesting exceeds 40");
            }
            let children = analysis_children(Node::Expression(&value));
            match &value {
                I::Tuple { .. } => pending.extend(children.into_iter().rev().filter_map(|edge| {
                    if let Node::Expression(value) = edge.node {
                        Some((value.clone(), frame, depth + 1))
                    } else {
                        None
                    }
                })),
                I::Read { next: false, .. } => reads.push(value),
                I::Call { definition, .. } => {
                    let mut ancestor = frame;
                    while let Some(index) = ancestor {
                        if frames[index].definition == Some(*definition) {
                            return unsupported("recursive operator definitions");
                        }
                        ancestor = frames[index].parent;
                    }
                    self.lower_definition(*definition)?;
                    let next = frames.len();
                    frames.push(Frame {
                        definition: Some(*definition),
                        arguments: children
                            .iter()
                            .filter(|edge| edge.descriptor.role == "call-actual")
                            .filter_map(|edge| {
                                if let Node::Expression(value) = edge.node {
                                    Some(value.clone())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        captures: children
                            .iter()
                            .filter(|edge| edge.descriptor.role == "call-capture")
                            .filter_map(|edge| {
                                if let Node::Expression(value) = edge.node {
                                    Some(value.clone())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        parent: frame,
                        depth,
                    });
                    let ir::DefinitionBody::Expression(body) =
                        &self.shared[*definition].definition.as_ref().unwrap().body
                    else {
                        return unsupported("generative UNCHANGED requires state variables");
                    };
                    pending.push((body.clone(), Some(next), depth + 1));
                }
                I::Frame { .. } => {
                    let next = frames.len();
                    frames.push(Frame {
                        definition: None,
                        arguments: children
                            .iter()
                            .filter(|edge| edge.descriptor.role == "frame-actual")
                            .filter_map(|edge| {
                                if let Node::Expression(value) = edge.node {
                                    Some(value.clone())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        captures: children
                            .iter()
                            .filter(|edge| edge.descriptor.role == "frame-capture")
                            .filter_map(|edge| {
                                if let Node::Expression(value) = edge.node {
                                    Some(value.clone())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        parent: frame,
                        depth,
                    });
                    for edge in children
                        .into_iter()
                        .filter(|edge| edge.descriptor.role == "frame-body")
                    {
                        if let Node::Expression(value) = edge.node {
                            pending.push((value.clone(), Some(next), depth + 1));
                        }
                    }
                }
                I::Parameter { slot } | I::Capture { slot } => {
                    let frame =
                        frame
                            .and_then(|id| frames.get(id))
                            .ok_or_else(|| CompileError {
                                message: message.into(),
                            })?;
                    let actuals = if matches!(value, I::Parameter { .. }) {
                        &frame.arguments
                    } else {
                        &frame.captures
                    };
                    let actual = actuals.get(*slot).ok_or_else(|| CompileError {
                        message: message.into(),
                    })?;
                    pending.push((actual.clone(), frame.parent, frame.depth + 1));
                }
                _ => return unsupported(message),
            }
        }
        self.target_nesting -= 1;
        Ok(reads)
    }
}
