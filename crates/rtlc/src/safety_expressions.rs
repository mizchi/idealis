use crate::CoreError;
use crate::ir::{
    Binding, Branch, DefinitionBody, Domain, Expression, FoldOrder, Scalar, Step, TemporalFormula,
};
use crate::safety_chains::Junction;
use crate::safety_values::Arena;
#[cfg(not(target_arch = "wasm32"))]
use crate::safety_values::ValueRemap;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[path = "safety_lazy.rs"]
mod lazy;

const RECURSIVE_DEPTH_LIMIT: u16 = 128;
const DEPTH_LIMIT: u16 = 256;

#[derive(Clone, Copy)]
pub(crate) struct EvalContext<'a> {
    pub current: &'a [Scalar],
    pub next: &'a [Option<Scalar>],
    pub locals: &'a [Scalar],
    pub old: Option<Scalar>,
    pub prime: bool,
    pub(crate) frame: Option<&'a CallFrame>,
    pub(crate) evaluation_depth: u16,
    pub(crate) recursive: bool,
}

#[derive(Clone)]
pub(crate) struct CallFrame {
    arguments: Vec<Argument>,
    captures: Vec<Argument>,
}

#[derive(Clone)]
struct Argument {
    expression: Arc<Expression>,
    locals: Vec<Scalar>,
    frame: Option<Arc<CallFrame>>,
    old: Option<Scalar>,
}

impl Argument {
    fn new(
        expression: &Expression,
        context: EvalContext<'_>,
        frame: Option<Arc<CallFrame>>,
    ) -> Self {
        Self {
            expression: Arc::new(expression.clone()),
            locals: context.locals.to_vec(),
            frame,
            old: context.old,
        }
    }

    fn evaluate<T>(
        &self,
        context: EvalContext<'_>,
        arena: &mut Arena,
        evaluate: fn(&Expression, EvalContext<'_>, &mut Arena) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        arena.consume(1)?;
        evaluate(
            &self.expression,
            EvalContext {
                locals: &self.locals,
                frame: self.frame.as_deref(),
                old: self.old,
                ..context
            },
            arena,
        )
    }
}
impl CallFrame {
    pub(crate) fn new(
        arguments: &[Expression],
        captures: &[Expression],
        context: EvalContext<'_>,
    ) -> Self {
        let parent = context
            .frame
            .filter(|_| !arguments.is_empty() || !captures.is_empty())
            .cloned()
            .map(Arc::new);
        Self {
            arguments: arguments
                .iter()
                .map(|value| Argument::new(value, context, parent.clone()))
                .collect(),
            captures: captures
                .iter()
                .map(|value| Argument::new(value, context, parent.clone()))
                .collect(),
        }
    }
}

impl<'a> EvalContext<'a> {
    pub fn new(current: &'a [Scalar], next: &'a [Option<Scalar>]) -> Self {
        Self {
            current,
            next,
            locals: &[],
            old: None,
            prime: false,
            frame: None,
            evaluation_depth: 0,
            recursive: false,
        }
    }
    #[inline]
    pub(crate) fn descend(&mut self, arena: &Arena) -> Result<(), CoreError> {
        if arena.metadata.is_none() && arena.definitions.is_empty() && self.frame.is_none() {
            return Ok(());
        }
        if self.evaluation_depth >= RECURSIVE_DEPTH_LIMIT {
            if self.recursive {
                return Err(CoreError {
                    code: "evaluation-budget",
                    message: "Recursive safety evaluation nesting exceeds 128".into(),
                });
            }
            if self.evaluation_depth >= DEPTH_LIMIT {
                return Err(CoreError::protocol("Safety evaluation nesting exceeds 256"));
            }
        }
        self.evaluation_depth += 1;
        Ok(())
    }
    fn read(self, slot: usize, next: bool) -> Result<Scalar, CoreError> {
        if next || self.prime {
            self.next
                .get(slot)
                .copied()
                .flatten()
                .ok_or_else(|| CoreError::protocol("Read of an unassigned next variable"))
        } else {
            self.current
                .get(slot)
                .copied()
                .ok_or_else(|| CoreError::protocol("Read of a missing current variable"))
        }
    }
}

pub(crate) fn register_memberships<'a>(
    expression: &'a Expression,
    visitor: &mut impl FnMut(&'a Expression),
) {
    fn register_type<'a>(expression: &'a Expression, visitor: &mut impl FnMut(&'a Expression)) {
        visitor(expression);
        for child in expression.children() {
            register_type(child, visitor);
        }
    }
    if let Expression::FunctionIn { right, .. } | Expression::SequenceIn { right, .. } = expression
    {
        register_type(right, visitor);
    }
    for child in expression.children() {
        register_memberships(child, visitor);
    }
}

pub(crate) fn register_branch_memberships<'a>(
    branches: &'a [Branch],
    visitor: &mut impl FnMut(&'a Expression),
) {
    let mut pending = vec![branches];
    while let Some(branches) = pending.pop() {
        for branch in branches {
            for step in &branch.steps {
                match step {
                    Step::Frame {
                        arguments,
                        captures,
                        branches,
                        ..
                    } => {
                        for expression in arguments.iter().chain(captures) {
                            visitor(expression);
                        }
                        pending.push(branches);
                    }
                    Step::Call {
                        arguments,
                        captures,
                        ..
                    } => {
                        for expression in arguments.iter().chain(captures) {
                            visitor(expression);
                        }
                    }
                    Step::Guard { expression }
                    | Step::Assign {
                        value: expression, ..
                    } => visitor(expression),
                    Step::Bind { domain, .. } | Step::Choose { domain, .. } => {
                        for expression in domain.children() {
                            visitor(expression);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn register_formula_memberships<'a>(
    formula: &'a TemporalFormula,
    visitor: &mut impl FnMut(&'a Expression),
) {
    let mut pending = vec![formula];
    while let Some(formula) = pending.pop() {
        match formula {
            TemporalFormula::Call {
                arguments,
                captures,
                ..
            } => {
                for expression in arguments.iter().chain(captures) {
                    visitor(expression);
                }
            }
            TemporalFormula::Frame {
                arguments,
                captures,
                formula,
                ..
            } => {
                for expression in arguments.iter().chain(captures) {
                    visitor(expression);
                }
                pending.push(formula);
            }
            TemporalFormula::Atom { expression } => visitor(expression),
            TemporalFormula::Not { formula }
            | TemporalFormula::Always { formula }
            | TemporalFormula::Eventually { formula } => pending.push(formula),
            TemporalFormula::And { left, right } | TemporalFormula::Or { left, right } => {
                pending.extend([left.as_ref(), right.as_ref()]);
            }
            TemporalFormula::Forall { bounds, formula }
            | TemporalFormula::Exists { bounds, formula } => {
                for binding in bounds {
                    for expression in binding.domain.children() {
                        visitor(expression);
                    }
                }
                pending.push(formula);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Validation {
    variables: usize,
    phase: u8,
    depth: usize,
    locals: usize,
    old: bool,
}
impl Validation {
    fn child(self) -> Self {
        Self {
            depth: self.depth + 1,
            ..self
        }
    }
}

#[derive(Clone)]
pub(crate) enum Values {
    Range(std::ops::RangeInclusive<i32>),
    Items(std::vec::IntoIter<Scalar>),
}
impl Iterator for Values {
    type Item = Scalar;
    fn next(&mut self) -> Option<Scalar> {
        match self {
            Self::Range(values) => values.next().map(Scalar::Int),
            Self::Items(values) => values.next(),
        }
    }
}

impl Domain {
    pub(crate) fn children(&self) -> Vec<&Expression> {
        match self {
            Self::Range { lower, upper } => vec![lower, upper],
            Self::Set { values } => values.iter().collect(),
            Self::Value { value } => vec![value],
        }
    }
    fn validate(&self, context: Validation) -> Result<(), CoreError> {
        for value in self.children() {
            value.validate_with(context)?;
        }
        Ok(())
    }
    pub(crate) fn values(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Values, CoreError> {
        Ok(match self {
            Self::Range { lower, upper } => Values::Range(
                lower.eval(ctx, arena)?.integer()?..=upper.eval(ctx, arena)?.integer()?,
            ),
            Self::Set { values } => {
                let values = values
                    .iter()
                    .map(|value| value.eval(ctx, arena))
                    .collect::<Result<Vec<_>, _>>()?;
                Values::Items(arena.ordered(values)?.into_iter())
            }
            Self::Value { value } => {
                let value = value.eval(ctx, arena)?;
                Values::Items(arena.members(value)?.into_iter())
            }
        })
    }
    pub(crate) fn contains(
        &self,
        value: Scalar,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<bool, CoreError> {
        match self {
            Self::Range { lower, upper } => {
                let lower = lower.eval(ctx, arena)?.integer()?;
                let upper = upper.eval(ctx, arena)?.integer()?;
                if lower > upper {
                    return Ok(false);
                }
                if arena.model_membership(value)? {
                    return Ok(false);
                }
                let value = value.integer()?;
                Ok(lower <= value && value <= upper)
            }
            Self::Set { .. } => {
                let values = self.values(ctx, arena)?;
                for candidate in values {
                    if arena.equal(value, candidate)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Value { value: set } => {
                let set = set.eval(ctx, arena)?;
                arena.contains(value, set)
            }
        }
    }
}

pub(crate) fn validate_branches(
    branches: &[Branch],
    variables: usize,
    phase: u8,
    locals: usize,
) -> Result<(), CoreError> {
    for branch in branches {
        if branch.steps.len() > 256 {
            return Err(CoreError::protocol("Safety branch exceeds 256 steps"));
        }
        let mut context = Validation {
            variables,
            phase,
            depth: 0,
            locals,
            old: false,
        };
        for step in &branch.steps {
            match step {
                Step::Frame { .. } | Step::Call { .. } => {
                    return Err(CoreError::protocol(
                        "Operator references require safety IR v2",
                    ));
                }
                Step::Guard { expression } => expression.validate_with(context)?,
                Step::Assign { slot, value } => {
                    if *slot >= variables {
                        return Err(CoreError::protocol("Invalid safety assignment slot"));
                    }
                    value.validate_with(context)?;
                }
                Step::Choose { slot, domain } => {
                    if *slot >= variables {
                        return Err(CoreError::protocol("Invalid safety assignment slot"));
                    }
                    domain.validate(context)?;
                }
                Step::Bind {
                    slot,
                    domain,
                    tuple_size,
                } => {
                    if *slot != context.locals || tuple_size.is_some_and(|size| size > 256) {
                        return Err(CoreError::protocol("Invalid bound variable slot"));
                    }
                    domain.validate(context)?;
                    context.locals += 1;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_constant_bindings(
    bindings: &[Binding],
    variables: usize,
    locals: usize,
) -> Result<usize, CoreError> {
    if bindings.is_empty() || locals + bindings.len() > 40 {
        return Err(CoreError::protocol("Invalid temporal binding count"));
    }
    for (index, binding) in bindings.iter().enumerate() {
        if binding.slot != locals + index || binding.tuple_size.is_some_and(|size| size > 256) {
            return Err(CoreError::protocol("Invalid temporal binding slot"));
        }
        binding.domain.validate(Validation {
            variables,
            phase: 3,
            depth: 0,
            locals,
            old: false,
        })?;
    }
    Ok(locals + bindings.len())
}

impl Expression {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn is_scalar_cost(&self) -> bool {
        let simple = match self {
            Self::Literal { value } => !matches!(value, Scalar::Ref(_)),
            Self::Read { .. }
            | Self::Bound { .. }
            | Self::Old
            | Self::Not { .. }
            | Self::Neg { .. }
            | Self::Add { .. }
            | Self::Sub { .. }
            | Self::Mul { .. }
            | Self::Div { .. }
            | Self::Mod { .. }
            | Self::Pow { .. }
            | Self::Eq { .. }
            | Self::Ne { .. }
            | Self::Lt { .. }
            | Self::Le { .. }
            | Self::Gt { .. }
            | Self::Ge { .. }
            | Self::And { .. }
            | Self::Or { .. }
            | Self::Implies { .. }
            | Self::If { .. } => true,
            Self::In { domain, .. } => match domain {
                Domain::Range { .. } => true,
                Domain::Set { values } => values.len() <= 16,
                Domain::Value { .. } => false,
            },
            _ => false,
        };
        simple && self.children().into_iter().all(Self::is_scalar_cost)
    }
    pub(crate) fn children(&self) -> Vec<&Self> {
        match self {
            Self::Frame {
                arguments,
                captures,
                body,
                ..
            } => arguments
                .iter()
                .chain(captures)
                .chain(std::iter::once(body.as_ref()))
                .collect(),
            Self::Call {
                arguments,
                captures,
                ..
            } => arguments.iter().chain(captures).collect(),
            Self::Literal { .. }
            | Self::Data { .. }
            | Self::Read { .. }
            | Self::Bound { .. }
            | Self::Parameter { .. }
            | Self::Capture { .. }
            | Self::Old => vec![],
            Self::Prime { argument }
            | Self::Not { argument }
            | Self::Neg { argument }
            | Self::Domain { argument }
            | Self::VariantTag { argument }
            | Self::Cardinality { argument }
            | Self::SequenceSet { argument }
            | Self::Len { argument }
            | Self::Head { argument }
            | Self::Tail { argument }
            | Self::Powerset { argument }
            | Self::Union { argument } => vec![argument],
            Self::Add { left, right }
            | Self::Sub { left, right }
            | Self::Mul { left, right }
            | Self::Div { left, right }
            | Self::Mod { left, right }
            | Self::Pow { left, right }
            | Self::Eq { left, right }
            | Self::Ne { left, right }
            | Self::Lt { left, right }
            | Self::Le { left, right }
            | Self::Gt { left, right }
            | Self::Ge { left, right }
            | Self::And { left, right }
            | Self::Or { left, right }
            | Self::Implies { left, right }
            | Self::Cup { left, right }
            | Self::Cap { left, right }
            | Self::Difference { left, right }
            | Self::SubsetEq { left, right }
            | Self::Append { left, right }
            | Self::Concat { left, right }
            | Self::FunctionSet { left, right }
            | Self::FunctionIn { left, right }
            | Self::SequenceIn { left, right } => vec![left, right],
            Self::If {
                condition,
                consequent,
                alternative,
            } => vec![condition, consequent, alternative],
            Self::Assert { condition, message } => vec![condition, message],
            Self::SelectSeq { sequence, body, .. } | Self::SortSeq { sequence, body, .. } => {
                vec![sequence, body]
            }
            Self::In { value, domain } => {
                let mut children = vec![value.as_ref()];
                children.extend(domain.children());
                children
            }
            Self::Set { values } | Self::Tuple { values } => values.iter().collect(),
            Self::Record { fields } | Self::RecordSet { fields } => {
                fields.iter().map(|entry| &entry.1).collect()
            }
            Self::At { target, key } => vec![target, key],
            Self::Update { target, updates } => {
                let mut children = vec![target.as_ref()];
                for update in updates {
                    children.extend(update.path.iter());
                    children.push(&update.value);
                }
                children
            }
            Self::Forall { bounds, body }
            | Self::Exists { bounds, body }
            | Self::Function { bounds, body }
            | Self::Filter { bounds, body }
            | Self::MapSet { bounds, body } => {
                let mut children = vec![];
                for bound in bounds {
                    children.extend(bound.domain.children());
                }
                children.push(body);
                children
            }
            Self::Fold {
                initial,
                collection,
                body,
                ..
            } => vec![initial, collection, body],
            Self::Choose { binding, body } => {
                let mut children = binding.domain.children();
                children.push(body);
                children
            }
            Self::Subseq { sequence, from, to } => vec![sequence, from, to],
            Self::Enabled { .. } | Self::UnmatchedCase { .. } => vec![],
        }
    }
    fn is_context_free(&self) -> bool {
        !matches!(
            self,
            Self::Read { .. }
                | Self::Frame { .. }
                | Self::Call { .. }
                | Self::Parameter { .. }
                | Self::Capture { .. }
                | Self::Enabled { .. }
                | Self::UnmatchedCase { .. }
                | Self::Bound { .. }
                | Self::Old
                | Self::Prime { .. }
                | Self::Literal {
                    value: Scalar::Ref(_)
                }
        ) && self.children().iter().all(|child| child.is_context_free())
    }
    pub(crate) fn is_constant(&self) -> bool {
        !matches!(
            self,
            Self::Read { .. }
                | Self::Enabled { .. }
                | Self::UnmatchedCase { .. }
                | Self::Frame { .. }
                | Self::Call { .. }
                | Self::Parameter { .. }
                | Self::Capture { .. }
        ) && self.children().iter().all(|value| value.is_constant())
    }
    pub(crate) fn validate(
        &self,
        variables: usize,
        phase: u8,
        depth: usize,
    ) -> Result<(), CoreError> {
        self.validate_with(Validation {
            variables,
            phase,
            depth,
            locals: 0,
            old: false,
        })
    }
    pub(crate) fn validate_locals(
        &self,
        variables: usize,
        phase: u8,
        locals: usize,
    ) -> Result<(), CoreError> {
        self.validate_with(Validation {
            variables,
            phase,
            locals,
            depth: 0,
            old: false,
        })
    }
    fn validate_with(&self, ctx: Validation) -> Result<(), CoreError> {
        if ctx.depth > 48 {
            return Err(CoreError::protocol("Safety expression nesting exceeds 48"));
        }
        match self {
            Self::Frame { .. }
            | Self::Call { .. }
            | Self::Parameter { .. }
            | Self::Capture { .. } => {
                return Err(CoreError::protocol(
                    "Operator references require safety IR v2",
                ));
            }
            Self::UnmatchedCase { location } => {
                if location.is_empty() {
                    return Err(CoreError::protocol("Invalid unmatched CASE location"));
                }
            }
            Self::Prime { argument } => {
                if ctx.phase != 1 {
                    return Err(CoreError::protocol("Prime requires an action context"));
                }
                argument.validate_with(Validation {
                    phase: 2,
                    ..ctx.child()
                })?;
                return Ok(());
            }
            Self::Read { slot, next } => {
                if *slot >= ctx.variables
                    || (ctx.phase == 0 && !*next)
                    || (ctx.phase == 2 && *next)
                    || ctx.phase == 3
                {
                    return Err(CoreError::protocol("Invalid safety variable read"));
                }
            }
            Self::Bound { slot } => {
                if *slot >= ctx.locals {
                    return Err(CoreError::protocol("Unbound quantified variable"));
                }
            }
            Self::Old => {
                if !ctx.old {
                    return Err(CoreError::protocol("Unbound EXCEPT @"));
                }
            }
            Self::Choose { binding, body } => {
                if binding.slot != ctx.locals
                    || ctx.locals >= 40
                    || binding.tuple_size.is_some_and(|size| size > 256)
                {
                    return Err(CoreError::protocol("Invalid CHOOSE variable slot"));
                }
                binding.domain.validate(ctx.child())?;
                body.validate_with(Validation {
                    locals: ctx.locals + 1,
                    ..ctx.child()
                })?;
                return Ok(());
            }
            Self::Forall { bounds, body }
            | Self::Exists { bounds, body }
            | Self::Function { bounds, body }
            | Self::Filter { bounds, body }
            | Self::MapSet { bounds, body } => {
                if bounds.is_empty() || bounds.len() > 40 {
                    return Err(CoreError::protocol("Empty binding list"));
                }
                let mut child = ctx.child();
                for bound in bounds {
                    if bound.slot != child.locals || bound.tuple_size.is_some_and(|size| size > 256)
                    {
                        return Err(CoreError::protocol("Invalid bound variable slot"));
                    }
                    bound.domain.validate(child)?;
                    child.locals += 1;
                }
                body.validate_with(child)?;
                return Ok(());
            }
            Self::Fold {
                accumulator,
                initial,
                collection,
                body,
                ..
            } => {
                if *accumulator != ctx.locals || ctx.locals + 2 > 40 {
                    return Err(CoreError::protocol("Invalid fold variable slot"));
                }
                initial.validate_with(ctx.child())?;
                collection.validate_with(ctx.child())?;
                let mut child = ctx.child();
                child.locals += 2;
                body.validate_with(child)?;
                return Ok(());
            }
            Self::SelectSeq {
                slot,
                sequence,
                body,
            }
            | Self::SortSeq {
                slot,
                sequence,
                body,
            } => {
                let arity = if matches!(self, Self::SortSeq { .. }) {
                    2
                } else {
                    1
                };
                if *slot != ctx.locals || ctx.locals + arity > 40 {
                    return Err(CoreError::protocol(
                        "Invalid sequence callback variable slot",
                    ));
                }
                sequence.validate_with(ctx.child())?;
                body.validate_with(Validation {
                    locals: ctx.locals + arity,
                    ..ctx.child()
                })?;
                return Ok(());
            }
            Self::Update { target, updates } => {
                target.validate_with(ctx.child())?;
                for update in updates {
                    if update.path.is_empty() || update.path.len() > 48 {
                        return Err(CoreError::protocol("Empty EXCEPT path"));
                    }
                    for part in &update.path {
                        part.validate_with(ctx.child())?;
                    }
                    update.value.validate_with(Validation {
                        old: true,
                        ..ctx.child()
                    })?;
                }
                return Ok(());
            }
            Self::Enabled {
                branches,
                subscript,
            } => {
                if ctx.phase == 3 {
                    return Err(CoreError::protocol("ENABLED is not a constant"));
                }
                validate_branches(branches, ctx.variables, 1, ctx.locals)?;
                if let Some(subscript) = subscript {
                    subscript.validate_with(Validation {
                        phase: 2,
                        ..ctx.child()
                    })?;
                }
                return Ok(());
            }
            _ => {}
        }
        for child in self.children() {
            child.validate_with(ctx.child())?;
        }
        Ok(())
    }
    #[inline(always)]
    pub(crate) fn eval(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        if ctx.evaluation_depth >= RECURSIVE_DEPTH_LIMIT {
            let mut checked = ctx;
            checked.descend(arena)?;
        }
        match self {
            Self::Literal { value } => Ok(*value),
            Self::Read { slot, next } => ctx.read(*slot, *next),
            Self::Bound { slot } => ctx
                .locals
                .get(*slot)
                .copied()
                .ok_or_else(|| CoreError::protocol("Unbound quantified variable")),
            Self::Old => ctx
                .old
                .ok_or_else(|| CoreError::protocol("Unbound EXCEPT @")),
            _ => self.eval_nonleaf(ctx, arena),
        }
    }
    #[inline(never)]
    fn eval_nonleaf(
        &self,
        mut ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        ctx.descend(arena)?;
        match self {
            Self::Prime { .. }
            | Self::Not { .. }
            | Self::Neg { .. }
            | Self::Add { .. }
            | Self::Sub { .. }
            | Self::Mul { .. }
            | Self::Div { .. }
            | Self::Mod { .. }
            | Self::Pow { .. }
            | Self::Eq { .. }
            | Self::Ne { .. }
            | Self::Lt { .. }
            | Self::Le { .. }
            | Self::Gt { .. }
            | Self::Ge { .. }
            | Self::And { .. }
            | Self::Or { .. }
            | Self::Implies { .. }
            | Self::If { .. }
            | Self::In { .. } => self.eval_scalar_operator(ctx, arena),
            Self::Frame { .. }
            | Self::Call { .. }
            | Self::Parameter { .. }
            | Self::Capture { .. } => self.eval_reference(ctx, arena, Self::eval),
            Self::Enabled { .. } => self.eval_enabled(ctx, arena),
            _ => self.eval_operator(ctx, arena),
        }
    }

    fn eval_reference<T>(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
        evaluate: fn(&Expression, EvalContext<'_>, &mut Arena) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        match self {
            Self::Frame {
                arguments,
                captures,
                body,
                ..
            } => {
                arena.consume(1)?;
                let frame = CallFrame::new(arguments, captures, ctx);
                evaluate(
                    body,
                    EvalContext {
                        locals: &[],
                        frame: Some(&frame),
                        old: None,
                        ..ctx
                    },
                    arena,
                )
            }
            Self::Call {
                definition,
                arguments,
                captures,
            } => {
                arena.consume(1)?;
                let definition = arena
                    .definitions
                    .get(*definition)
                    .ok_or_else(|| CoreError::protocol("Unknown safety definition"))?;
                let frame = CallFrame::new(arguments, captures, ctx);
                let DefinitionBody::Expression(body) = &definition.body else {
                    return Err(CoreError::protocol("Invalid safety call body kind"));
                };
                evaluate(
                    body,
                    EvalContext {
                        recursive: ctx.recursive || definition.recursive,
                        locals: &[],
                        frame: Some(&frame),
                        old: None,
                        ..ctx
                    },
                    arena,
                )
            }
            Self::Parameter { slot } | Self::Capture { slot } => {
                let frame = ctx
                    .frame
                    .ok_or_else(|| CoreError::protocol("Missing lexical call frame"))?;
                let bindings = if matches!(self, Self::Parameter { .. }) {
                    &frame.arguments
                } else {
                    &frame.captures
                };
                bindings
                    .get(*slot)
                    .ok_or_else(|| CoreError::protocol("Invalid lexical reference"))?
                    .evaluate(ctx, arena, evaluate)
            }
            _ => Err(CoreError::protocol("Expected a lexical reference")),
        }
    }

    fn eval_scalar_operator(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        Ok(match self {
            Self::Prime { argument } => argument.eval(EvalContext { prime: true, ..ctx }, arena)?,
            Self::Not { argument } => Scalar::Bool(!argument.eval(ctx, arena)?.boolean()?),
            Self::Neg { .. } => return self.eval_neg(ctx, arena),
            Self::Add { .. } | Self::Sub { .. } | Self::Mul { .. } => {
                return self.eval_arithmetic(ctx, arena);
            }
            Self::Div { .. } => return self.eval_div(ctx, arena),
            Self::Mod { .. } => return self.eval_mod(ctx, arena),
            Self::Pow { .. } => return self.eval_pow(ctx, arena),
            Self::Eq { .. } | Self::Ne { .. } => return self.eval_equality(ctx, arena),
            Self::Lt { .. } | Self::Le { .. } | Self::Gt { .. } | Self::Ge { .. } => {
                return self.eval_comparison(ctx, arena);
            }
            Self::And { left, .. } if matches!(**left, Self::And { .. }) => {
                return self.eval_junction(Junction::And, ctx, arena);
            }
            Self::Or { left, .. } if matches!(**left, Self::Or { .. }) => {
                return self.eval_junction(Junction::Or, ctx, arena);
            }
            Self::And { .. } => return self.eval_and(ctx, arena),
            Self::Or { .. } => return self.eval_or(ctx, arena),
            Self::Implies { .. } => return self.eval_implies(ctx, arena),
            Self::If { .. } => return self.eval_if(ctx, arena),
            Self::In { .. } => return self.eval_in(ctx, arena),
            _ => return Err(CoreError::protocol("Expected a scalar operator")),
        })
    }
    #[inline]
    fn eval_neg(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Neg { argument } = self else {
            return Err(CoreError::protocol("Expected neg"));
        };
        Ok(Scalar::Int(
            argument
                .eval(ctx, arena)?
                .integer()?
                .checked_neg()
                .ok_or_else(|| CoreError::protocol("Safety integer overflow"))?,
        ))
    }
    #[inline]
    fn eval_arithmetic(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Add { left, right } | Self::Sub { left, right } | Self::Mul { left, right }) =
            self
        else {
            return Err(CoreError::protocol("Expected an arithmetic operator"));
        };
        Ok({
            let left = left.eval(ctx, arena)?.integer()?;
            let right = right.eval(ctx, arena)?.integer()?;
            Scalar::Int(
                match self {
                    Self::Add { .. } => left.checked_add(right),
                    Self::Sub { .. } => left.checked_sub(right),
                    _ => left.checked_mul(right),
                }
                .ok_or_else(|| CoreError::protocol("Safety integer overflow"))?,
            )
        })
    }
    #[inline]
    fn eval_div(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Div { left, right } = self else {
            return Err(CoreError::protocol("Expected div"));
        };
        Ok({
            let left = left.eval(ctx, arena)?.integer()?;
            let right = right.eval(ctx, arena)?.integer()?;
            if right == 0 {
                return Err(CoreError::protocol("Integer division by zero"));
            }
            let quotient = left
                .checked_div(right)
                .ok_or_else(|| CoreError::protocol("Safety integer overflow"))?;
            Scalar::Int(if left % right != 0 && (left < 0) != (right < 0) {
                quotient - 1
            } else {
                quotient
            })
        })
    }
    #[inline]
    fn eval_mod(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Mod { left, right } = self else {
            return Err(CoreError::protocol("Expected mod"));
        };
        Ok({
            let left = left.eval(ctx, arena)?.integer()?;
            let right = right.eval(ctx, arena)?.integer()?;
            if right <= 0 {
                return Err(CoreError::protocol("Modulo requires a positive divisor"));
            }
            Scalar::Int(left.rem_euclid(right))
        })
    }
    #[inline]
    fn eval_pow(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Pow { left, right } = self else {
            return Err(CoreError::protocol("Expected pow"));
        };
        Ok({
            let left = left.eval(ctx, arena)?.integer()?;
            let right = right.eval(ctx, arena)?.integer()?;
            if right < 0 {
                return Err(CoreError::protocol("Power requires a natural exponent"));
            }
            if left == 0 && right == 0 {
                return Err(CoreError::protocol("Undefined 0^0"));
            }
            Scalar::Int(
                left.checked_pow(right as u32)
                    .ok_or_else(|| CoreError::protocol("Safety integer overflow"))?,
            )
        })
    }
    #[inline]
    fn eval_equality(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let (Self::Eq { left, right } | Self::Ne { left, right }) = self else {
            return Err(CoreError::protocol("Expected an equality"));
        };
        Ok({
            let left = left.eval(ctx, arena)?;
            let right = right.eval(ctx, arena)?;
            Scalar::Bool(arena.equal(left, right)? == matches!(self, Self::Eq { .. }))
        })
    }
    #[inline]
    fn eval_comparison(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Lt { left, right }
        | Self::Le { left, right }
        | Self::Gt { left, right }
        | Self::Ge { left, right }) = self
        else {
            return Err(CoreError::protocol("Expected a comparison"));
        };
        Ok({
            let left = left.eval(ctx, arena)?.integer()?;
            let right = right.eval(ctx, arena)?.integer()?;
            Scalar::Bool(match self {
                Self::Lt { .. } => left < right,
                Self::Le { .. } => left <= right,
                Self::Gt { .. } => left > right,
                _ => left >= right,
            })
        })
    }
    #[inline]
    fn eval_and(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::And { left, right } = self else {
            return Err(CoreError::protocol("Expected and"));
        };
        Ok(Scalar::Bool(
            left.eval(ctx, arena)?.boolean()? && right.eval(ctx, arena)?.boolean()?,
        ))
    }
    #[inline]
    fn eval_or(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Or { left, right } = self else {
            return Err(CoreError::protocol("Expected or"));
        };
        Ok(Scalar::Bool(
            left.eval(ctx, arena)?.boolean()? || right.eval(ctx, arena)?.boolean()?,
        ))
    }
    #[inline]
    fn eval_implies(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Implies { left, right } = self else {
            return Err(CoreError::protocol("Expected implies"));
        };
        Ok(Scalar::Bool(
            !left.eval(ctx, arena)?.boolean()? || right.eval(ctx, arena)?.boolean()?,
        ))
    }
    #[inline]
    fn eval_if(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let mut node = self;
        while let Self::If {
            condition,
            consequent,
            alternative,
        } = node
        {
            if condition.eval(ctx, arena)?.boolean()? {
                return consequent.eval(ctx, arena);
            }
            node = alternative;
        }
        node.eval(ctx, arena)
    }
    #[inline]
    fn eval_in(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::In { value, domain } = self else {
            return Err(CoreError::protocol("Expected in"));
        };
        Ok({
            let value = value.eval(ctx, arena)?;
            Scalar::Bool(domain.contains(value, ctx, arena)?)
        })
    }
    #[inline(never)]
    fn eval_junction(
        &self,
        junction: Junction,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let decisive = junction == Junction::Or;
        let mut result = !decisive;
        junction.visit_terms(self, |term, _| {
            if term.eval(ctx, arena)?.boolean()? == decisive {
                result = decisive;
                Ok(false)
            } else {
                Ok(true)
            }
        })?;
        Ok(Scalar::Bool(result))
    }
    fn eval_enabled(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let old = if arena.metadata.is_some() || !arena.definitions.is_empty() {
            ctx.old
        } else {
            None
        };
        Ok(match self {
            Self::Enabled {
                branches,
                subscript,
            } => {
                let current = if ctx.current.is_empty() || ctx.prime {
                    ctx.next
                        .iter()
                        .map(|value| {
                            value.ok_or_else(|| {
                                CoreError::protocol("ENABLED requires assigned initial variables")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    ctx.current.to_vec()
                };
                let mut enabled = false;
                for branch in branches {
                    let mut next = vec![None; current.len()];
                    visit_branch(
                        branch,
                        0,
                        EvalContext {
                            current: &current,
                            next: &[],
                            locals: ctx.locals,
                            old,
                            prime: false,
                            frame: ctx.frame,
                            evaluation_depth: ctx.evaluation_depth,
                            recursive: ctx.recursive,
                        },
                        &mut next,
                        arena,
                        &mut |next, arena| {
                            if let Some(subscript) = subscript {
                                let context = EvalContext {
                                    locals: ctx.locals,
                                    frame: ctx.frame,
                                    evaluation_depth: ctx.evaluation_depth.saturating_add(
                                        u16::try_from(branch.steps.len() + 1).unwrap_or(u16::MAX),
                                    ),
                                    old,
                                    ..EvalContext::new(&current, next)
                                };
                                let before = subscript.eval(context, arena)?;
                                let after = subscript.eval(
                                    EvalContext {
                                        prime: true,
                                        ..context
                                    },
                                    arena,
                                )?;
                                if arena.equal(before, after)? {
                                    return Ok(false);
                                }
                            }
                            enabled = true;
                            Ok(true)
                        },
                    )?;
                    if enabled {
                        break;
                    }
                }
                Scalar::Bool(enabled)
            }
            _ => return Err(CoreError::protocol("Expected ENABLED")),
        })
    }
    #[inline(never)]
    fn eval_operator(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        Ok(match self {
            Self::Literal { .. } | Self::Read { .. } | Self::Bound { .. } | Self::Old => {
                return self.eval(ctx, arena);
            }
            Self::Frame { .. }
            | Self::Call { .. }
            | Self::Parameter { .. }
            | Self::Capture { .. } => {
                return self.eval_reference(ctx, arena, Self::eval);
            }
            Self::Data { value } => arena.data_expression(self, value)?,
            Self::Assert { .. } => return self.eval_assert(ctx, arena),
            Self::UnmatchedCase { .. } => return self.eval_unmatched_case(ctx, arena),
            Self::Prime { .. }
            | Self::Not { .. }
            | Self::Neg { .. }
            | Self::Add { .. }
            | Self::Sub { .. }
            | Self::Mul { .. }
            | Self::Div { .. }
            | Self::Mod { .. }
            | Self::Pow { .. }
            | Self::Eq { .. }
            | Self::Ne { .. }
            | Self::Lt { .. }
            | Self::Le { .. }
            | Self::Gt { .. }
            | Self::Ge { .. }
            | Self::And { .. }
            | Self::Or { .. }
            | Self::Implies { .. }
            | Self::If { .. }
            | Self::In { .. } => self.eval_scalar_operator(ctx, arena)?,
            Self::Set { .. } | Self::Tuple { .. } => return self.eval_set_or_tuple(ctx, arena),
            Self::Record { .. } => return self.eval_record(ctx, arena),
            Self::At { .. } => return self.eval_at(ctx, arena),
            Self::Update { .. } => return self.eval_update(ctx, arena),
            Self::Forall { .. } | Self::Exists { .. } => return self.eval_quantifier(ctx, arena),
            Self::Choose { .. } => return self.eval_choose(ctx, arena),
            Self::Function { .. } | Self::Filter { .. } | Self::MapSet { .. } => {
                return self.eval_comprehension(ctx, arena);
            }
            Self::Fold { .. } => return self.eval_fold(ctx, arena),
            Self::SelectSeq { .. } => return self.eval_select_seq(ctx, arena),
            Self::SortSeq { .. } => return self.eval_sort_seq(ctx, arena),
            Self::Domain { .. } => return self.eval_domain(ctx, arena),
            Self::VariantTag { .. } => return self.eval_variant_tag(ctx, arena),
            Self::Cardinality { .. } => return self.eval_cardinality(ctx, arena),
            Self::Len { .. } | Self::Head { .. } | Self::Tail { .. } => {
                return self.eval_len_head_tail(ctx, arena);
            }
            Self::Append { .. } | Self::Concat { .. } => {
                return self.eval_append_or_concat(ctx, arena);
            }
            Self::Subseq { .. } => return self.eval_subseq(ctx, arena),
            Self::Cup { .. }
            | Self::Cap { .. }
            | Self::Difference { .. }
            | Self::SubsetEq { .. } => return self.eval_set_operator(ctx, arena),
            Self::Union { .. } => return self.eval_union(ctx, arena),
            Self::Powerset { .. } => return self.eval_powerset(ctx, arena),
            Self::FunctionSet { .. } => return self.eval_function_set(ctx, arena),
            Self::RecordSet { .. } => return self.eval_record_set(ctx, arena),
            Self::FunctionIn { .. } => return self.eval_function_in(ctx, arena),
            Self::SequenceIn { .. } => return self.eval_sequence_in(ctx, arena),
            Self::SequenceSet { .. } => {
                return Err(CoreError::protocol("Sequence sets cannot be enumerated"));
            }
            Self::Enabled { .. } => self.eval_enabled(ctx, arena)?,
        })
    }
    #[inline]
    fn eval_assert(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Assert { condition, message } = self else {
            return Err(CoreError::protocol("Expected assert"));
        };
        Ok({
            let condition = condition.eval(ctx, arena)?;
            let message = message.eval(ctx, arena)?;
            if condition != Scalar::Bool(true) {
                return Err(CoreError::protocol(&format!(
                    "TLC assertion failed: {:?}",
                    arena.portable(message)?
                )));
            }
            Scalar::Bool(true)
        })
    }
    #[inline]
    fn eval_unmatched_case(
        &self,
        _ctx: EvalContext<'_>,
        _arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::UnmatchedCase { location } = self else {
            return Err(CoreError::protocol("Expected unmatched case"));
        };
        Err(CoreError::protocol(&format!(
            "Attempted to evaluate a CASE with no conditions true at {location}"
        )))
    }
    #[inline]
    fn eval_set_or_tuple(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Set { values } | Self::Tuple { values }) = self else {
            return Err(CoreError::protocol("Expected a set or tuple"));
        };
        Ok({
            let mut result = vec![];
            for value in values {
                arena.tick()?;
                result.push(value.eval(ctx, arena)?);
            }
            if matches!(self, Self::Set { .. }) {
                arena.set(result)?
            } else {
                arena.tuple(result)?
            }
        })
    }
    #[inline]
    fn eval_record(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Record { fields } = self else {
            return Err(CoreError::protocol("Expected record"));
        };
        Ok({
            let mut result = vec![];
            for (name, value) in fields {
                arena.tick()?;
                let name = arena.string(name.clone())?;
                let value = value.eval(ctx, arena)?;
                result.push((name, value));
            }
            arena.function(result)?
        })
    }
    #[inline]
    fn eval_at(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::At { target, key } = self else {
            return Err(CoreError::protocol("Expected at"));
        };
        Ok({
            if arena.metadata.is_some() {
                let target = target.eval_function(ctx, arena)?;
                let key = key.eval(ctx, arena)?;
                return target.at(key, arena);
            }
            let target = target.eval(ctx, arena)?;
            let key = key.eval(ctx, arena)?;
            arena.at(target, key)?
        })
    }
    #[inline]
    fn eval_update(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Update { target, updates } = self else {
            return Err(CoreError::protocol("Expected update"));
        };
        Ok({
            let mut result = target.eval(ctx, arena)?;
            for update in updates {
                arena.tick()?;
                let path = update
                    .path
                    .iter()
                    .map(|part| part.eval(ctx, arena))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(old) = arena.path(result, &path)? {
                    let value = update.value.eval(
                        EvalContext {
                            old: Some(old),
                            ..ctx
                        },
                        arena,
                    )?;
                    result = arena.replace(result, &path, value)?;
                }
            }
            result
        })
    }
    #[inline]
    fn eval_quantifier(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Forall { bounds, body } | Self::Exists { bounds, body }) = self else {
            return Err(CoreError::protocol("Expected a quantifier"));
        };
        Ok({
            let forall = matches!(self, Self::Forall { .. });
            let mut result = forall;
            visit_bindings(bounds, 0, ctx, arena, &mut |ctx, arena| {
                let value = body.eval(ctx, arena)?.boolean()?;
                if value != forall {
                    result = value;
                    Ok(false)
                } else {
                    Ok(true)
                }
            })?;
            Scalar::Bool(result)
        })
    }
    #[inline]
    fn eval_choose(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Choose { binding, body } = self else {
            return Err(CoreError::protocol("Expected choose"));
        };
        Ok({
            let mut selected = None;
            visit_bindings(
                std::slice::from_ref(binding),
                0,
                ctx,
                arena,
                &mut |bound, arena| {
                    if body.eval(bound, arena)?.boolean()? {
                        selected = bound.locals.last().copied();
                        Ok(false)
                    } else {
                        Ok(true)
                    }
                },
            )?;
            selected.ok_or_else(|| {
                CoreError::protocol("CHOOSE: no element of the domain satisfied the predicate")
            })?
        })
    }
    #[inline]
    fn eval_comprehension(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Function { bounds, body }
        | Self::Filter { bounds, body }
        | Self::MapSet { bounds, body }) = self
        else {
            return Err(CoreError::protocol("Expected a comprehension"));
        };
        Ok({
            let mut values = vec![];
            let mut entries = vec![];
            visit_bindings(bounds, 0, ctx, arena, &mut |inner, arena| {
                let value = body.eval(inner, arena)?;
                if matches!(self, Self::Function { .. }) {
                    let keys = inner.locals[ctx.locals.len()..].to_vec();
                    let key = if keys.len() == 1 {
                        keys[0]
                    } else {
                        arena.tuple(keys)?
                    };
                    entries.push((key, value));
                } else if matches!(self, Self::Filter { .. }) {
                    if value.boolean()? {
                        if bounds.len() != 1 {
                            return Err(CoreError::protocol("Filter requires one bound"));
                        }
                        values.push(inner.locals[ctx.locals.len()]);
                    }
                } else {
                    values.push(value);
                }
                Ok(true)
            })?;
            if matches!(self, Self::Function { .. }) {
                arena.function(entries)?
            } else {
                arena.set(values)?
            }
        })
    }
    #[inline]
    fn eval_fold(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Fold {
            accumulator,
            initial,
            collection,
            order,
            body,
        } = self
        else {
            return Err(CoreError::protocol("Expected fold"));
        };
        Ok({
            if *accumulator != ctx.locals.len() {
                return Err(CoreError::protocol("Invalid fold variable slot"));
            }
            let mut result = initial.eval(ctx, arena)?;
            let source = collection.eval(ctx, arena)?;
            let (elements, replacement) = match order {
                FoldOrder::SetMembers => {
                    let found = arena.fold_members(source, self)?;
                    (found.members, found.replacement)
                }
                FoldOrder::SequenceLeft => (arena.sequence(source)?, None),
            };
            let mut locals = ctx.locals.to_vec();
            for element in elements {
                arena.tick()?;
                locals.push(result);
                locals.push(element);
                result = body
                    .eval(
                        EvalContext {
                            locals: &locals,
                            ..ctx
                        },
                        arena,
                    )
                    .map_err(|error| match replacement {
                        Some(message)
                            if error.code != "budget"
                                && error.code != "evaluation-budget"
                                && !error.message.starts_with("Unsupported TLA input") =>
                        {
                            CoreError::protocol(message)
                        }
                        _ => error,
                    })?;
                locals.truncate(ctx.locals.len());
            }
            if let Some(message) = replacement {
                arena.require_comparable_members(result, message)?;
            }
            result
        })
    }
    #[inline]
    fn eval_select_seq(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::SelectSeq {
            slot,
            sequence,
            body,
        } = self
        else {
            return Err(CoreError::protocol("Expected select seq"));
        };
        Ok({
            if *slot != ctx.locals.len() {
                return Err(CoreError::protocol(
                    "Invalid sequence callback variable slot",
                ));
            }
            let source = sequence.eval(ctx, arena)?;
            let elements = arena.sequence(source)?;
            arena.consume(elements.len())?;
            let mut locals = ctx.locals.to_vec();
            locals.push(Scalar::Bool(false));
            let mut selected = vec![];
            for element in elements {
                arena.tick()?;
                locals[*slot] = element;
                if body
                    .eval(
                        EvalContext {
                            locals: &locals,
                            ..ctx
                        },
                        arena,
                    )?
                    .boolean()?
                {
                    selected.push(element);
                }
            }
            arena.tuple(selected)?
        })
    }
    #[inline]
    fn eval_sort_seq(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::SortSeq {
            slot,
            sequence,
            body,
        } = self
        else {
            return Err(CoreError::protocol("Expected sort seq"));
        };
        Ok({
            if *slot != ctx.locals.len() {
                return Err(CoreError::protocol(
                    "Invalid sequence callback variable slot",
                ));
            }
            let source = sequence.eval(ctx, arena)?;
            let mut elements = arena.sequence(source)?;
            arena.consume(elements.len())?;
            let mut locals = ctx.locals.to_vec();
            locals.extend([Scalar::Bool(false); 2]);
            for index in 1..elements.len() {
                arena.tick()?;
                let element = elements[index];
                let mut position = index;
                while position > 0 {
                    arena.tick()?;
                    locals[*slot] = element;
                    locals[*slot + 1] = elements[position - 1];
                    if !body
                        .eval(
                            EvalContext {
                                locals: &locals,
                                ..ctx
                            },
                            arena,
                        )?
                        .boolean()?
                    {
                        break;
                    }
                    elements[position] = elements[position - 1];
                    position -= 1;
                }
                elements[position] = element;
            }
            arena.tuple(elements)?
        })
    }
    #[inline]
    fn eval_domain(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Domain { argument } = self else {
            return Err(CoreError::protocol("Expected domain"));
        };
        Ok({
            if arena.metadata.is_some() {
                return argument.eval_function(ctx, arena)?.domain(arena);
            }
            let value = argument.eval(ctx, arena)?;
            arena.domain(value)?
        })
    }
    #[inline]
    fn eval_variant_tag(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::VariantTag { argument } = self else {
            return Err(CoreError::protocol("Expected variant tag"));
        };
        Ok({
            let value = argument.eval(ctx, arena)?;
            let entries = arena.entries(value)?;
            if entries.len() != 1 || arena.text(entries[0].0).is_none() {
                return Err(CoreError::protocol(
                    "Unsupported TLA input: VariantTag requires a singleton string domain",
                ));
            }
            entries[0].0
        })
    }
    #[inline]
    fn eval_cardinality(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::Cardinality { argument } = self else {
            return Err(CoreError::protocol("Expected cardinality"));
        };
        Ok({
            let value = argument.eval(ctx, arena)?;
            Scalar::Int(
                i32::try_from(arena.members(value)?.len())
                    .map_err(|_| CoreError::protocol("Set cardinality overflow"))?,
            )
        })
    }
    #[inline]
    fn eval_len_head_tail(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Len { argument } | Self::Head { argument } | Self::Tail { argument }) = self
        else {
            return Err(CoreError::protocol("Expected Len, Head or Tail"));
        };
        Ok({
            let value = argument.eval(ctx, arena)?;
            if let Some(text) = arena.text(value) {
                let units = text.encode_utf16().collect::<Vec<_>>();
                return Ok(match self {
                    Self::Len { .. } => Scalar::Int(
                        i32::try_from(units.len())
                            .map_err(|_| CoreError::protocol("String length overflow"))?,
                    ),
                    Self::Head { .. } => {
                        let first = units
                            .first()
                            .ok_or_else(|| CoreError::protocol("Head of an empty string"))?;
                        arena.string_utf16(&[*first])?
                    }
                    _ => {
                        if units.is_empty() {
                            return Err(CoreError::protocol("Tail of an empty string"));
                        }
                        arena.string_utf16(&units[1..])?
                    }
                });
            }
            let mut values = arena.sequence(value)?;
            match self {
                Self::Len { .. } => Scalar::Int(values.len() as i32),
                Self::Head { .. } => *values
                    .first()
                    .ok_or_else(|| CoreError::protocol("Head of an empty sequence"))?,
                _ => {
                    if values.is_empty() {
                        return Err(CoreError::protocol("Tail of an empty sequence"));
                    }
                    values.remove(0);
                    arena.tuple(values)?
                }
            }
        })
    }
    #[inline]
    fn eval_append_or_concat(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Append { left, right } | Self::Concat { left, right }) = self else {
            return Err(CoreError::protocol("Expected Append or concatenation"));
        };
        Ok({
            let left = left.eval(ctx, arena)?;
            let right = right.eval(ctx, arena)?;
            if let Some(text) = arena.text(left) {
                let suffix = arena
                    .text(right)
                    .ok_or_else(|| CoreError::protocol("String concatenation requires strings"))?;
                let size = text
                    .len()
                    .checked_add(suffix.len())
                    .ok_or_else(|| CoreError::protocol("String length overflow"))?;
                arena.consume(size)?;
                let text = arena
                    .text(left)
                    .ok_or_else(|| CoreError::protocol("Expected a string"))?;
                let suffix = arena
                    .text(right)
                    .ok_or_else(|| CoreError::protocol("Expected a string"))?;
                let value = format!("{text}{suffix}");
                return arena.string(value);
            }
            let mut values = arena.sequence_values(left)?;
            if matches!(self, Self::Append { .. }) {
                values.push(right);
            } else {
                values.extend(arena.sequence_values(right)?);
            }
            arena.tuple(values)?
        })
    }
    #[inline]
    fn eval_subseq(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Subseq { sequence, from, to } = self else {
            return Err(CoreError::protocol("Expected subseq"));
        };
        Ok({
            let sequence = sequence.eval(ctx, arena)?;
            let from = from.eval(ctx, arena)?.integer()?;
            let to = to.eval(ctx, arena)?.integer()?;
            if let Some(text) = arena.text(sequence) {
                let units = text.encode_utf16().collect::<Vec<_>>();
                if from > to {
                    return arena.string(String::new());
                }
                if from < 1 || to as usize > units.len() {
                    return Err(CoreError::protocol("SubSeq bounds are outside the string"));
                }
                return arena.string_utf16(&units[(from - 1) as usize..to as usize]);
            }
            let values = arena.sequence(sequence)?;
            if from > to {
                arena.tuple(vec![])?
            } else {
                if from < 1 || to as usize > values.len() {
                    return Err(CoreError::protocol(
                        "SubSeq bounds are outside the sequence",
                    ));
                }
                arena.tuple(values[(from - 1) as usize..to as usize].to_vec())?
            }
        })
    }
    #[inline]
    fn eval_set_operator(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let (Self::Cup { left, right }
        | Self::Cap { left, right }
        | Self::Difference { left, right }
        | Self::SubsetEq { left, right }) = self
        else {
            return Err(CoreError::protocol("Expected a set operator"));
        };
        Ok({
            let left = left.eval(ctx, arena)?;
            let right = right.eval(ctx, arena)?;
            let mut result = vec![];
            for value in arena.members(left)? {
                arena.tick()?;
                let contains = arena.contains(value, right)?;
                if matches!(self, Self::SubsetEq { .. }) && !contains {
                    return Ok(Scalar::Bool(false));
                }
                if matches!(self, Self::Cup { .. })
                    || (matches!(self, Self::Cap { .. }) && contains)
                    || (matches!(self, Self::Difference { .. }) && !contains)
                {
                    result.push(value);
                }
            }
            if matches!(self, Self::SubsetEq { .. }) {
                Scalar::Bool(true)
            } else {
                if matches!(self, Self::Cup { .. }) {
                    result.extend(arena.members(right)?);
                }
                arena.set(result)?
            }
        })
    }
    #[inline]
    fn eval_union(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Union { argument } = self else {
            return Err(CoreError::protocol("Expected union"));
        };
        Ok({
            let outer = argument.eval(ctx, arena)?;
            let mut values = vec![];
            for set in arena.members(outer)? {
                arena.tick()?;
                values.extend(arena.members(set)?);
            }
            arena.set(values)?
        })
    }
    #[inline]
    fn eval_powerset(&self, ctx: EvalContext<'_>, arena: &mut Arena) -> Result<Scalar, CoreError> {
        let Self::Powerset { argument } = self else {
            return Err(CoreError::protocol("Expected powerset"));
        };
        Ok({
            let set = argument.eval(ctx, arena)?;
            let members = arena.members(set)?;
            let mut sets = vec![arena.set(vec![])?];
            for value in members {
                let mut additions = vec![];
                for set in &sets {
                    arena.tick()?;
                    let mut members = arena.members(*set)?;
                    members.push(value);
                    additions.push(arena.set(members)?);
                }
                sets.extend(additions);
            }
            arena.set(sets)?
        })
    }
    #[inline]
    fn eval_function_set(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::FunctionSet { left, right } = self else {
            return Err(CoreError::protocol("Expected function set"));
        };
        Ok({
            let domain = left.eval(ctx, arena)?;
            let range = right.eval(ctx, arena)?;
            let keys = arena.members(domain)?;
            let values = arena.members(range)?;
            let mut functions = vec![];
            enumerate_functions(
                &keys,
                &vec![values.as_slice(); keys.len()],
                arena,
                &mut functions,
            )?;
            arena.set(functions)?
        })
    }
    #[inline]
    fn eval_record_set(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::RecordSet { fields } = self else {
            return Err(CoreError::protocol("Expected record set"));
        };
        Ok({
            let mut keys = vec![];
            let mut ranges = vec![];
            for (name, set) in fields {
                keys.push(arena.string(name.clone())?);
                let set = set.eval(ctx, arena)?;
                ranges.push(arena.members(set)?);
            }
            let mut functions = vec![];
            enumerate_functions(
                &keys,
                &ranges.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                arena,
                &mut functions,
            )?;
            arena.set(functions)?
        })
    }
    #[inline]
    fn eval_function_in(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::FunctionIn { left, right } = self else {
            return Err(CoreError::protocol("Expected function in"));
        };
        Ok({
            let value = left.eval(ctx, arena)?;
            let set = MembershipSet::eval(right, ctx, arena)?;
            Scalar::Bool(set.contains(value, arena)?)
        })
    }
    #[inline]
    fn eval_sequence_in(
        &self,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Scalar, CoreError> {
        let Self::SequenceIn { left, right } = self else {
            return Err(CoreError::protocol("Expected sequence in"));
        };
        Ok({
            let value = left.eval(ctx, arena)?;
            let set = MembershipSet::new(
                MembershipKind::Sequences(MembershipSet::eval(right, ctx, arena)?),
                false,
            );
            Scalar::Bool(set.contains(value, arena)?)
        })
    }
}

#[derive(PartialEq, Eq)]
pub(crate) struct MembershipSet {
    kind: MembershipKind,
    cacheable: bool,
    hash: u64,
}
#[derive(PartialEq, Eq, Hash)]
enum MembershipKind {
    Finite(Scalar),
    Range {
        lower: i32,
        upper: i32,
    },
    Sequences(Arc<MembershipSet>),
    Functions {
        domain: Scalar,
        range: Arc<MembershipSet>,
    },
    Records(Vec<(Scalar, Arc<MembershipSet>)>),
    Subsets(Scalar),
}
impl Hash for MembershipSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}
impl MembershipSet {
    pub(crate) fn fingerprint(&self) -> u64 {
        self.hash
    }
    fn new(kind: MembershipKind, cacheable: bool) -> Arc<Self> {
        let mut hasher = DefaultHasher::new();
        kind.hash(&mut hasher);
        Arc::new(Self {
            kind,
            cacheable,
            hash: hasher.finish(),
        })
    }
    fn eval(
        expression: &Expression,
        ctx: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<Arc<Self>, CoreError> {
        let mut ctx = ctx;
        ctx.descend(arena)?;
        if matches!(
            expression,
            Expression::Frame { .. }
                | Expression::Call { .. }
                | Expression::Parameter { .. }
                | Expression::Capture { .. }
        ) {
            return expression.eval_reference(ctx, arena, Self::eval);
        }
        if arena.metadata.is_some() && matches!(expression, Expression::If { .. }) {
            let mut node = expression;
            while let Expression::If {
                condition,
                consequent,
                alternative,
            } = node
            {
                if condition.eval(ctx, arena)?.boolean()? {
                    return Self::eval(consequent, ctx, arena);
                }
                node = alternative;
            }
            return Self::eval(node, ctx, arena);
        }
        let cached = arena.prepared_membership(expression);
        if let Some(Some(prepared)) = &cached {
            return Ok(Arc::clone(prepared));
        }
        let cacheable = if cached.is_some() {
            false
        } else {
            expression.is_context_free()
                && arena
                    .metadata
                    .as_ref()
                    .is_none_or(|metadata| metadata.is_context_free(expression))
        };
        let kind = match expression {
            Expression::MapSet { bounds, body }
                if arena.metadata.is_some()
                    && matches!(
                        (bounds.as_slice(), body.as_ref()),
                        (
                            [Binding {
                                slot,
                                tuple_size: None,
                                domain: Domain::Range { .. },
                            }],
                            Expression::Bound { slot: body_slot }
                        ) if slot == body_slot
                    ) =>
            {
                let Domain::Range { lower, upper } = &bounds[0].domain else {
                    unreachable!()
                };
                MembershipKind::Range {
                    lower: lower.eval(ctx, arena)?.integer()?,
                    upper: upper.eval(ctx, arena)?.integer()?,
                }
            }
            Expression::SequenceSet { argument } => {
                MembershipKind::Sequences(Self::eval(argument, ctx, arena)?)
            }
            Expression::FunctionSet { left, right } => MembershipKind::Functions {
                domain: left.eval(ctx, arena)?,
                range: Self::eval(right, ctx, arena)?,
            },
            Expression::RecordSet { fields } => {
                let mut values = vec![];
                for (name, set) in fields {
                    let name = arena.string(name.clone())?;
                    values.push((name, Self::eval(set, ctx, arena)?));
                }
                values.sort_by(|left, right| arena.total(left.0, right.0));
                MembershipKind::Records(values)
            }
            Expression::Powerset { argument } => {
                MembershipKind::Subsets(argument.eval(ctx, arena)?)
            }
            _ => MembershipKind::Finite(expression.eval(ctx, arena)?),
        };
        let prepared = Self::new(kind, cacheable);
        if cached.is_none() {
            arena.cache_membership_type(expression, cacheable.then(|| Arc::clone(&prepared)));
        }
        Ok(prepared)
    }
    fn contains(self: &Arc<Self>, value: Scalar, arena: &mut Arena) -> Result<bool, CoreError> {
        let cacheable = self.cacheable && matches!(self.kind, MembershipKind::Records(_));
        if cacheable && let Some(result) = arena.membership_result(self, value) {
            return Ok(result);
        }
        let result = self.contains_uncached(value, arena)?;
        if cacheable {
            arena.cache_membership_result(Arc::clone(self), value, result);
        }
        Ok(result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn remap(self: &Arc<Self>, mapping: &ValueRemap) -> Arc<Self> {
        let changed = match &self.kind {
            MembershipKind::Finite(value) | MembershipKind::Subsets(value) => {
                mapping.get(*value) != *value
            }
            MembershipKind::Range { .. } => false,
            MembershipKind::Sequences(set) => !Arc::ptr_eq(set, &set.remap(mapping)),
            MembershipKind::Functions { domain, range } => {
                mapping.get(*domain) != *domain || !Arc::ptr_eq(range, &range.remap(mapping))
            }
            MembershipKind::Records(fields) => fields.iter().any(|(key, set)| {
                mapping.get(*key) != *key || !Arc::ptr_eq(set, &set.remap(mapping))
            }),
        };
        if !changed {
            return Arc::clone(self);
        }
        let kind = match &self.kind {
            MembershipKind::Finite(value) => MembershipKind::Finite(mapping.get(*value)),
            MembershipKind::Subsets(value) => MembershipKind::Subsets(mapping.get(*value)),
            MembershipKind::Range { lower, upper } => MembershipKind::Range {
                lower: *lower,
                upper: *upper,
            },
            MembershipKind::Sequences(set) => MembershipKind::Sequences(set.remap(mapping)),
            MembershipKind::Functions { domain, range } => MembershipKind::Functions {
                domain: mapping.get(*domain),
                range: range.remap(mapping),
            },
            MembershipKind::Records(fields) => MembershipKind::Records(
                fields
                    .iter()
                    .map(|(key, set)| (mapping.get(*key), set.remap(mapping)))
                    .collect(),
            ),
        };
        Self::new(kind, self.cacheable)
    }
    fn contains_uncached(&self, value: Scalar, arena: &mut Arena) -> Result<bool, CoreError> {
        if !matches!(
            self.kind,
            MembershipKind::Finite(_) | MembershipKind::Subsets(_) | MembershipKind::Range { .. }
        ) && arena.model_membership(value)?
        {
            return Ok(false);
        }
        match &self.kind {
            MembershipKind::Finite(set) => arena.contains(value, *set),
            MembershipKind::Range { lower, upper } => {
                if lower > upper {
                    return Ok(false);
                }
                if arena.model_membership(value)? {
                    return Ok(false);
                }
                let value = value.integer()?;
                Ok(*lower <= value && value <= *upper)
            }
            MembershipKind::Subsets(set) => {
                for element in arena.members(value)? {
                    arena.tick()?;
                    if !arena.contains(element, *set)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            MembershipKind::Sequences(set) => {
                for element in arena.sequence_values(value)? {
                    arena.tick()?;
                    if !set.contains(element, arena)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            MembershipKind::Functions { domain, range } => {
                if !arena.has_domain(value, *domain)? {
                    return Ok(false);
                }
                if let MembershipKind::Finite(set) = range.kind
                    && let Some(count) = arena.binary_function_membership(value, set)
                {
                    arena.consume(count)?;
                    return Ok(true);
                }
                for element in arena.function_values(value)? {
                    arena.tick()?;
                    if !range.contains(element, arena)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            MembershipKind::Records(fields) => {
                let entries = arena.entries(value)?;
                if entries.iter().any(|(key, _)| arena.text(*key).is_none()) {
                    return Err(CoreError::protocol(
                        "Record membership requires string keys",
                    ));
                }
                if entries.len() != fields.len() {
                    return Ok(false);
                }
                for (key, set) in fields {
                    let Some(element) = arena.select(value, *key)? else {
                        return Ok(false);
                    };
                    arena.tick()?;
                    if !set.contains(element, arena)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }
}

pub(crate) fn visit_bindings(
    bindings: &[Binding],
    position: usize,
    ctx: EvalContext<'_>,
    arena: &mut Arena,
    visitor: &mut impl FnMut(EvalContext<'_>, &mut Arena) -> Result<bool, CoreError>,
) -> Result<bool, CoreError> {
    let mut ctx = ctx;
    ctx.descend(arena)?;
    let Some(binding) = bindings.get(position) else {
        return visitor(ctx, arena);
    };
    let values = binding.domain.values(ctx, arena)?;
    let mut locals = ctx.locals.to_vec();
    if binding.slot != locals.len() {
        return Err(CoreError::protocol("Unbound variable slot"));
    }
    for value in values {
        arena.tick()?;
        arena.check_tuple(value, binding.tuple_size)?;
        locals.push(value);
        let keep = visit_bindings(
            bindings,
            position + 1,
            EvalContext {
                locals: &locals,
                ..ctx
            },
            arena,
            visitor,
        )?;
        locals.pop();
        if !keep {
            return Ok(false);
        }
    }
    Ok(true)
}

fn enumerate_functions(
    keys: &[Scalar],
    ranges: &[&[Scalar]],
    arena: &mut Arena,
    output: &mut Vec<Scalar>,
) -> Result<(), CoreError> {
    if ranges.iter().any(|range| range.is_empty()) {
        return Ok(());
    }
    let mut positions = vec![0; keys.len()];
    loop {
        arena.tick()?;
        let mut entries = Vec::with_capacity(keys.len());
        for (index, key) in keys.iter().enumerate() {
            arena.tick()?;
            entries.push((*key, ranges[index][positions[index]]));
        }
        output.push(arena.function(entries)?);
        let mut index = keys.len();
        loop {
            if index == 0 {
                return Ok(());
            }
            index -= 1;
            positions[index] += 1;
            if positions[index] < ranges[index].len() {
                break;
            }
            positions[index] = 0;
        }
    }
}

type BranchVisitor<'a> =
    dyn FnMut(&mut [Option<Scalar>], &mut Arena) -> Result<bool, CoreError> + 'a;

pub(crate) fn visit_branch(
    branch: &Branch,
    position: usize,
    ctx: EvalContext<'_>,
    next: &mut [Option<Scalar>],
    arena: &mut Arena,
    visitor: &mut BranchVisitor<'_>,
) -> Result<bool, CoreError> {
    let mut ctx = ctx;
    ctx.descend(arena)?;
    arena.tick()?;
    let Some(step) = branch.steps.get(position) else {
        return visitor(next, arena);
    };
    let context = EvalContext { next, ..ctx };
    match step {
        Step::Call {
            definition,
            arguments,
            captures,
        } => {
            arena.consume(1)?;
            let definition = arena
                .definitions
                .get(*definition)
                .ok_or_else(|| CoreError::protocol("Unknown safety definition"))?;
            let DefinitionBody::Action(branches) = &definition.body else {
                return Err(CoreError::protocol("Invalid safety call body kind"));
            };
            let frame = CallFrame::new(arguments, captures, context);
            let inner = EvalContext {
                recursive: ctx.recursive || definition.recursive,
                locals: &[],
                frame: Some(&frame),
                old: None,
                ..ctx
            };
            for inner_branch in branches {
                if visit_branch(inner_branch, 0, inner, next, arena, &mut |next, arena| {
                    visit_branch(branch, position + 1, ctx, next, arena, visitor)
                })? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Step::Frame {
            arguments,
            captures,
            branches,
            ..
        } => {
            arena.consume(1)?;
            let frame = CallFrame::new(arguments, captures, context);
            let inner = EvalContext {
                locals: &[],
                frame: Some(&frame),
                old: None,
                ..ctx
            };
            for inner_branch in branches {
                if visit_branch(inner_branch, 0, inner, next, arena, &mut |next, arena| {
                    visit_branch(branch, position + 1, ctx, next, arena, visitor)
                })? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Step::Guard { expression } => {
            if expression.eval(context, arena)?.boolean()? {
                visit_branch(branch, position + 1, ctx, next, arena, visitor)
            } else {
                Ok(false)
            }
        }
        Step::Assign { slot, value } => {
            let value = value.eval(context, arena)?;
            let previous = next[*slot];
            if let Some(previous) = previous
                && !arena.equal(previous, value)?
            {
                return Ok(false);
            }
            next[*slot] = Some(value);
            let result = visit_branch(branch, position + 1, ctx, next, arena, visitor);
            next[*slot] = previous;
            result
        }
        Step::Choose { slot, domain } => {
            if let Some(value) = next[*slot] {
                return if domain.contains(value, context, arena)? {
                    visit_branch(branch, position + 1, ctx, next, arena, visitor)
                } else {
                    Ok(false)
                };
            }
            for value in domain.values(context, arena)? {
                next[*slot] = Some(value);
                let found = visit_branch(branch, position + 1, ctx, next, arena, visitor)?;
                next[*slot] = None;
                if found {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Step::Bind {
            slot,
            domain,
            tuple_size,
        } => {
            if let Some(run) = arena.bind_run(branch, position) {
                let mut locals = ctx.locals.to_vec();
                return visit_bind_run(
                    branch,
                    position,
                    run,
                    ctx,
                    &mut locals,
                    next,
                    arena,
                    visitor,
                    &mut Continuations::default(),
                );
            }
            let values = domain.values(context, arena)?;
            let mut locals = ctx.locals.to_vec();
            if *slot != locals.len() {
                return Err(CoreError::protocol("Unbound action variable"));
            }
            for value in values {
                arena.check_tuple(value, *tuple_size)?;
                locals.push(value);
                let found = visit_branch(
                    branch,
                    position + 1,
                    EvalContext {
                        locals: &locals,
                        ..ctx
                    },
                    next,
                    arena,
                    visitor,
                )?;
                locals.pop();
                if found {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

pub(crate) struct BindRun {
    end: usize,
    read: Vec<usize>,
}

const RECORDED_SUCCESSORS_PER_RUN: usize = 4096;
const RECORDED_SUCCESSORS_PER_CONTINUATION: usize = 64;
const RECORDED_CONTINUATIONS_PER_RUN: usize = 4096;
const UNREPLAYED_CONTINUATIONS_BEFORE_GIVING_UP: usize = 256;

type Successors = Vec<Vec<Option<Scalar>>>;

#[derive(Default)]
struct Continuations {
    entries: HashMap<Vec<Scalar>, Option<Successors>>,
    recorded: usize,
    replayed: usize,
}

enum ReadWork<'a> {
    Expression(&'a Expression),
    Expressions(std::slice::Iter<'a, Expression>),
    Children(std::vec::IntoIter<&'a Expression>),
    Steps(std::slice::Iter<'a, Step>),
    Branches(std::slice::Iter<'a, Branch>),
}

fn step_reads(steps: &[Step], reads: &mut Vec<usize>) -> bool {
    let mut work = vec![ReadWork::Steps(steps.iter())];
    while let Some(item) = work.pop() {
        match item {
            ReadWork::Expression(expression) => match expression {
                Expression::Bound { slot } => reads.push(*slot),
                Expression::Enabled { .. } => return false,
                _ => work.push(ReadWork::Children(expression.children().into_iter())),
            },
            ReadWork::Expressions(mut remaining) => {
                if let Some(expression) = remaining.next() {
                    work.push(ReadWork::Expressions(remaining));
                    work.push(ReadWork::Expression(expression));
                }
            }
            ReadWork::Children(mut remaining) => {
                if let Some(expression) = remaining.next() {
                    work.push(ReadWork::Children(remaining));
                    work.push(ReadWork::Expression(expression));
                }
            }
            ReadWork::Steps(mut remaining) => {
                let Some(step) = remaining.next() else {
                    continue;
                };
                work.push(ReadWork::Steps(remaining));
                match step {
                    Step::Guard { expression } => work.push(ReadWork::Expression(expression)),
                    Step::Assign { value, .. } => work.push(ReadWork::Expression(value)),
                    Step::Choose { domain, .. } | Step::Bind { domain, .. } => {
                        work.push(ReadWork::Children(domain.children().into_iter()));
                    }
                    Step::Call {
                        arguments,
                        captures,
                        ..
                    } => {
                        work.push(ReadWork::Expressions(captures.iter()));
                        work.push(ReadWork::Expressions(arguments.iter()));
                    }
                    Step::Frame {
                        arguments,
                        captures,
                        branches,
                        ..
                    } => {
                        work.push(ReadWork::Branches(branches.iter()));
                        work.push(ReadWork::Expressions(captures.iter()));
                        work.push(ReadWork::Expressions(arguments.iter()));
                    }
                }
            }
            ReadWork::Branches(mut remaining) => {
                if let Some(branch) = remaining.next() {
                    work.push(ReadWork::Branches(remaining));
                    work.push(ReadWork::Steps(branch.steps.iter()));
                }
            }
        }
    }
    true
}

fn plan_bind_run(branch: &Branch, position: usize) -> Option<BindRun> {
    let Some(Step::Bind { slot: first, .. }) = branch.steps.get(position) else {
        return None;
    };
    let end = position
        + branch.steps[position..]
            .iter()
            .take_while(|step| matches!(step, Step::Bind { .. }))
            .count();
    if end == branch.steps.len() {
        return None;
    }
    let mut reads = vec![];
    if !step_reads(&branch.steps[end..], &mut reads) {
        return None;
    }
    let slots = *first..first.checked_add(end - position)?;
    let read: Vec<usize> = slots.clone().filter(|slot| reads.contains(slot)).collect();
    (read.len() < slots.len()).then_some(BindRun { end, read })
}

pub(crate) fn plan_bind_runs<'a>(
    branches: &'a [Branch],
    register: &mut impl FnMut(&'a Branch, usize, BindRun),
) {
    let mut pending = vec![branches];
    while let Some(branches) = pending.pop() {
        for branch in branches {
            for (position, step) in branch.steps.iter().enumerate() {
                if let Step::Frame { branches, .. } = step {
                    pending.push(branches);
                }
                if let Some(run) = plan_bind_run(branch, position) {
                    register(branch, position, run);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn visit_bind_run(
    branch: &Branch,
    position: usize,
    run: &BindRun,
    ctx: EvalContext<'_>,
    locals: &mut Vec<Scalar>,
    next: &mut [Option<Scalar>],
    arena: &mut Arena,
    visitor: &mut BranchVisitor<'_>,
    continuations: &mut Continuations,
) -> Result<bool, CoreError> {
    if position == run.end {
        return visit_continuation(
            branch,
            run,
            ctx,
            locals,
            next,
            arena,
            visitor,
            continuations,
        );
    }
    let mut ctx = ctx;
    if locals.len() > ctx.locals.len() {
        ctx.descend(arena)?;
        arena.tick()?;
    }
    let Some(Step::Bind {
        slot,
        domain,
        tuple_size,
    }) = branch.steps.get(position)
    else {
        return Err(CoreError::protocol("Invalid bound variable run"));
    };
    let values = domain.values(
        EvalContext {
            next,
            locals,
            ..ctx
        },
        arena,
    )?;
    if *slot != locals.len() {
        return Err(CoreError::protocol("Unbound action variable"));
    }
    for value in values {
        arena.check_tuple(value, *tuple_size)?;
        locals.push(value);
        let found = visit_bind_run(
            branch,
            position + 1,
            run,
            ctx,
            locals,
            next,
            arena,
            visitor,
            continuations,
        )?;
        locals.pop();
        if found {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
fn visit_continuation(
    branch: &Branch,
    run: &BindRun,
    ctx: EvalContext<'_>,
    locals: &[Scalar],
    next: &mut [Option<Scalar>],
    arena: &mut Arena,
    visitor: &mut BranchVisitor<'_>,
    continuations: &mut Continuations,
) -> Result<bool, CoreError> {
    let key: Vec<Scalar> = run.read.iter().map(|slot| locals[*slot]).collect();
    if let Some(Some(successors)) = continuations.entries.get(&key) {
        continuations.replayed += 1;
        arena.tick()?;
        for successor in successors.clone() {
            let mut successor = successor;
            arena.tick()?;
            if visitor(&mut successor, arena)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let known = continuations.entries.contains_key(&key);
    let unproductive = continuations.replayed == 0
        && continuations.entries.len() >= UNREPLAYED_CONTINUATIONS_BEFORE_GIVING_UP;
    if !known && (unproductive || continuations.entries.len() >= RECORDED_CONTINUATIONS_PER_RUN) {
        return visit_branch(
            branch,
            run.end,
            EvalContext { locals, ..ctx },
            next,
            arena,
            visitor,
        );
    }
    let mut recording =
        (!known && continuations.recorded < RECORDED_SUCCESSORS_PER_RUN).then(Vec::new);
    let capacity = RECORDED_SUCCESSORS_PER_CONTINUATION
        .min(RECORDED_SUCCESSORS_PER_RUN.saturating_sub(continuations.recorded));
    let found = visit_branch(
        branch,
        run.end,
        EvalContext { locals, ..ctx },
        next,
        arena,
        &mut |values, arena| {
            if let Some(successors) = &mut recording {
                if successors.len() < capacity {
                    successors.push(values.to_vec());
                } else {
                    recording = None;
                }
            }
            visitor(values, arena)
        },
    )?;
    if !known && !found {
        continuations.recorded += recording.as_ref().map_or(0, Vec::len);
        continuations.entries.insert(key, recording);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::EvalContext;
    use crate::Value;
    use crate::ir::{Expression, Scalar};
    use crate::safety_values::Arena;

    #[test]
    fn a_continuation_crossing_the_run_wide_budget_is_not_recorded_and_not_replayed() {
        use super::{BindRun, Continuations, RECORDED_SUCCESSORS_PER_RUN, visit_continuation};
        use crate::ir::{Branch, Domain, Step};
        let literal = |value| Expression::Literal {
            value: Scalar::Int(value),
        };
        let branch = Branch {
            name: "Two".into(),
            steps: vec![
                Step::Bind {
                    slot: 0,
                    domain: Domain::Set {
                        values: vec![literal(0)],
                    },
                    tuple_size: None,
                },
                Step::Choose {
                    slot: 0,
                    domain: Domain::Set {
                        values: vec![literal(1), literal(2)],
                    },
                },
            ],
        };
        let run = BindRun {
            end: 1,
            read: vec![],
        };
        let current = [Scalar::Int(0)];
        let locals = [Scalar::Int(0)];
        let mut continuations = Continuations {
            recorded: RECORDED_SUCCESSORS_PER_RUN - 1,
            ..Continuations::default()
        };
        let visits = |continuations: &mut Continuations| {
            let mut arena = Arena::new(1000);
            let mut seen = vec![];
            let found = visit_continuation(
                &branch,
                &run,
                EvalContext {
                    locals: &locals,
                    ..EvalContext::new(&current, &[])
                },
                &locals,
                &mut [None],
                &mut arena,
                &mut |values, _| {
                    seen.push(values.to_vec());
                    Ok(false)
                },
                continuations,
            )
            .unwrap();
            assert!(!found);
            seen
        };
        let expected = vec![vec![Some(Scalar::Int(1))], vec![Some(Scalar::Int(2))]];
        assert_eq!(visits(&mut continuations), expected);
        assert_eq!(continuations.recorded, RECORDED_SUCCESSORS_PER_RUN - 1);
        assert!(matches!(continuations.entries.get(&vec![]), Some(None)));
        assert_eq!(visits(&mut continuations), expected);
        assert_eq!(continuations.replayed, 0);
    }

    #[test]
    fn v1_enabled_subscript_keeps_the_existing_long_branch_limit() {
        use crate::ir::{Branch, Step};
        let mut steps = vec![Step::Assign {
            slot: 0,
            value: Expression::Read {
                slot: 0,
                next: false,
            },
        }];
        steps.extend((0..129).map(|_| Step::Guard {
            expression: Expression::Literal {
                value: Scalar::Bool(true),
            },
        }));
        let expression = Expression::Enabled {
            branches: vec![Branch {
                name: "Long".into(),
                steps,
            }],
            subscript: Some(Box::new(Expression::Read {
                slot: 0,
                next: false,
            })),
        };
        assert_eq!(
            expression
                .eval(
                    EvalContext::new(&[Scalar::Int(0)], &[]),
                    &mut Arena::new(1000)
                )
                .unwrap(),
            Scalar::Bool(false)
        );
    }

    fn fold(
        order: &str,
        initial: serde_json::Value,
        collection: serde_json::Value,
        body: serde_json::Value,
    ) -> Expression {
        serde_json::from_value(serde_json::json!({"op":"fold","accumulator":0,"order":order,"initial":initial,"collection":collection,"body":body})).unwrap()
    }

    #[test]
    fn fold_charges_each_iteration_and_exhaustion_is_an_error() {
        use serde_json::json;
        for (order, value) in [
            (
                "set-members",
                Value::Set(vec![Value::Int("1".into()), Value::Int("2".into())]),
            ),
            (
                "sequence-left",
                Value::List(vec![Value::Int("1".into()), Value::Int("2".into())]),
            ),
        ] {
            let expression = fold(
                order,
                json!({"op":"literal","value":{"type":"int","value":"0"}}),
                json!({"op":"read","slot":0,"next":false}),
                json!({"op":"bound","slot":0}),
            );
            let mut arena = Arena::new(100);
            let source = arena.data(&value).unwrap();
            let before = arena.work;
            expression
                .eval(EvalContext::new(&[source], &[]), &mut arena)
                .unwrap();
            assert_eq!(arena.work - before, 2);
            let mut limited = Arena::new(before + 1);
            let source = limited.data(&value).unwrap();
            let error = expression
                .eval(EvalContext::new(&[source], &[]), &mut limited)
                .unwrap_err();
            assert!(error.message.contains("work budget"), "{error}");
            assert_eq!(limited.work, before + 1);
        }
    }

    #[test]
    fn fold_reserves_two_slots_without_raising_the_bound_limit() {
        use serde_json::json;
        for (count, accepted) in [(38, true), (39, false)] {
            let bindings: Vec<_> = (0..count)
                .map(|slot| json!({"slot":slot,"domain":{"kind":"set","values":[]}}))
                .collect();
            let expression: Expression = serde_json::from_value(json!({"op":"forall","bounds":bindings,"body":{"op":"fold","accumulator":count,"initial":{"op":"literal","value":{"type":"bool","value":true}},"collection":{"op":"set","values":[]},"order":"set-members","body":{"op":"bound","slot":count+1}}})).unwrap();
            assert_eq!(expression.validate(0, 3, 0).is_ok(), accepted);
        }
    }

    #[test]
    fn fold_slots_are_scoped_and_collection_uses_outer_scope() {
        use serde_json::json;
        let valid = fold(
            "set-members",
            json!({"op":"literal","value":{"type":"int","value":"0"}}),
            json!({"op":"set","values":[]}),
            json!({"op":"bound","slot":1}),
        );
        for (field, value) in [
            ("accumulator", json!(1)),
            ("accumulator", json!(40)),
            ("initial", json!({"op":"bound","slot":0})),
            ("collection", json!({"op":"bound","slot":1})),
            ("body", json!({"op":"bound","slot":2})),
        ] {
            let mut invalid = serde_json::to_value(&valid).unwrap();
            invalid[field] = value;
            let invalid: Expression = serde_json::from_value(invalid).unwrap();
            assert!(invalid.validate(0, 3, 0).is_err());
        }
        for (field, value) in [("order", json!("right")), ("extra", json!(true))] {
            let mut invalid = serde_json::to_value(&valid).unwrap();
            invalid[field] = value;
            assert!(serde_json::from_value::<Expression>(invalid).is_err());
        }
    }

    fn membership(element: Expression) -> Expression {
        Expression::FunctionIn {
            left: Box::new(Expression::Read {
                slot: 0,
                next: false,
            }),
            right: Box::new(Expression::RecordSet {
                fields: vec![(
                    "value".into(),
                    Expression::Set {
                        values: vec![element],
                    },
                )],
            }),
        }
    }

    #[test]
    fn successful_record_type_checks_reuse_their_work() {
        let mut arena = Arena::new(1000);
        let value = arena
            .data(&Value::Record(vec![(
                "value".into(),
                Value::Int("0".into()),
            )]))
            .unwrap();
        let expression = membership(Expression::Literal {
            value: Scalar::Int(0),
        });
        assert!(
            expression
                .eval(EvalContext::new(&[value], &[]), &mut arena)
                .unwrap()
                .boolean()
                .unwrap()
        );
        let work = arena.work;
        for _ in 0..10 {
            assert!(
                expression
                    .eval(EvalContext::new(&[value], &[]), &mut arena)
                    .unwrap()
                    .boolean()
                    .unwrap()
            );
        }
        assert_eq!(arena.work, work);
    }

    #[test]
    fn cached_type_results_compare_descriptors_after_hash_collisions() {
        use super::{MembershipKind, MembershipSet};
        use std::sync::Arc;
        let mut arena = Arena::new(1000);
        let key = arena.string("value".into()).unwrap();
        let value = arena.function(vec![(key, Scalar::Int(0))]).unwrap();
        let zero = arena.set(vec![Scalar::Int(0)]).unwrap();
        let one = arena.set(vec![Scalar::Int(1)]).unwrap();
        let first = Arc::new(MembershipSet {
            hash: 7,
            cacheable: true,
            kind: MembershipKind::Records(vec![(
                key,
                MembershipSet::new(MembershipKind::Finite(zero), true),
            )]),
        });
        let second = Arc::new(MembershipSet {
            hash: 7,
            cacheable: true,
            kind: MembershipKind::Records(vec![(
                key,
                MembershipSet::new(MembershipKind::Finite(one), true),
            )]),
        });
        arena.cache_membership_result(Arc::clone(&first), value, true);
        arena.cache_membership_result(Arc::clone(&second), value, false);
        assert_eq!(arena.membership_result(&first, value), Some(true));
        assert_eq!(arena.membership_result(&second, value), Some(false));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn cached_membership_values_and_descriptors_remap_with_worker_references() {
        use crate::safety_work::WorkBudget;
        let mut base = Arena::new(1000);
        let budget = WorkBudget::new(900, 2);
        let first = membership(Expression::Literal {
            value: Scalar::Int(1),
        });
        let second = membership(Expression::Literal {
            value: Scalar::Int(2),
        });
        let mut left = base.fork(&budget, 0);
        let mut right = base.fork(&budget, 1);
        let left_value = left
            .data(&Value::Record(vec![(
                "value".into(),
                Value::Int("1".into()),
            )]))
            .unwrap();
        let right_value = right
            .data(&Value::Record(vec![(
                "value".into(),
                Value::Int("2".into()),
            )]))
            .unwrap();
        assert_eq!(left_value, right_value);
        assert!(
            first
                .eval(EvalContext::new(&[left_value], &[]), &mut left)
                .unwrap()
                .boolean()
                .unwrap()
        );
        assert!(
            second
                .eval(EvalContext::new(&[right_value], &[]), &mut right)
                .unwrap()
                .boolean()
                .unwrap()
        );
        let left = left.into_delta();
        let right = right.into_delta();
        let left_map = base.merge(left).unwrap();
        let right_map = base.merge(right).unwrap();
        let left_value = left_map.get(left_value);
        let right_value = right_map.get(right_value);
        assert_ne!(left_value, right_value);
        assert!(
            first
                .eval(EvalContext::new(&[left_value], &[]), &mut base)
                .unwrap()
                .boolean()
                .unwrap()
        );
        assert!(
            second
                .eval(EvalContext::new(&[right_value], &[]), &mut base)
                .unwrap()
                .boolean()
                .unwrap()
        );
        assert!(
            !first
                .eval(EvalContext::new(&[right_value], &[]), &mut base)
                .unwrap()
                .boolean()
                .unwrap()
        );
    }

    #[test]
    fn record_types_depending_on_bound_variables_are_not_reused() {
        let mut arena = Arena::new(1000);
        let value = arena
            .data(&Value::Record(vec![(
                "value".into(),
                Value::Int("0".into()),
            )]))
            .unwrap();
        let expression = membership(Expression::Bound { slot: 0 });
        for (number, expected) in [(0, true), (1, false), (0, true)] {
            let current = [value];
            let locals = [Scalar::Int(number)];
            let context = EvalContext {
                locals: &locals,
                ..EvalContext::new(&current, &[])
            };
            assert_eq!(
                expression
                    .eval(context, &mut arena)
                    .unwrap()
                    .boolean()
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn preparing_types_does_not_evaluate_an_unreached_branch_or_cache_an_error() {
        let mut arena = Arena::new(1000);
        let value = arena
            .data(&Value::Record(vec![(
                "value".into(),
                Value::Int("0".into()),
            )]))
            .unwrap();
        let invalid = membership(Expression::Head {
            argument: Box::new(Expression::Tuple { values: vec![] }),
        });
        let expression = Expression::If {
            condition: Box::new(Expression::Literal {
                value: Scalar::Bool(false),
            }),
            consequent: Box::new(invalid.clone()),
            alternative: Box::new(Expression::Literal {
                value: Scalar::Bool(true),
            }),
        };
        assert!(
            expression
                .eval(EvalContext::new(&[value], &[]), &mut arena)
                .unwrap()
                .boolean()
                .unwrap()
        );
        assert!(
            invalid
                .eval(EvalContext::new(&[value], &[]), &mut arena)
                .is_err()
        );
        assert!(
            invalid
                .eval(EvalContext::new(&[value], &[]), &mut arena)
                .is_err()
        );
    }
    #[test]
    fn leaf_evaluation_preserves_context_errors_and_lazy_branches() {
        let current = [Scalar::Int(3)];
        let next = [Some(Scalar::Int(7))];
        let locals = [Scalar::Bool(true)];
        let context = EvalContext {
            current: &current,
            next: &next,
            locals: &locals,
            old: Some(Scalar::Int(-1)),
            prime: false,
            frame: None,
            evaluation_depth: 0,
            recursive: false,
        };
        let mut arena = Arena::new(0);
        for (expression, expected) in [
            (
                Expression::Literal {
                    value: Scalar::Bool(false),
                },
                Scalar::Bool(false),
            ),
            (
                Expression::Read {
                    slot: 0,
                    next: false,
                },
                Scalar::Int(3),
            ),
            (
                Expression::Read {
                    slot: 0,
                    next: true,
                },
                Scalar::Int(7),
            ),
            (Expression::Bound { slot: 0 }, Scalar::Bool(true)),
            (Expression::Old, Scalar::Int(-1)),
            (
                Expression::Prime {
                    argument: Box::new(Expression::Read {
                        slot: 0,
                        next: false,
                    }),
                },
                Scalar::Int(7),
            ),
        ] {
            assert_eq!(expression.eval(context, &mut arena).unwrap(), expected);
        }
        let empty = EvalContext::new(&[], &[]);
        for (expression, expected) in [
            (
                Expression::Read {
                    slot: 0,
                    next: false,
                },
                "Read of a missing current variable",
            ),
            (
                Expression::Read {
                    slot: 0,
                    next: true,
                },
                "Read of an unassigned next variable",
            ),
            (Expression::Bound { slot: 0 }, "Unbound quantified variable"),
            (Expression::Old, "Unbound EXCEPT @"),
        ] {
            assert_eq!(
                expression.eval(empty, &mut arena).unwrap_err().message,
                expected
            );
        }
        let expression = Expression::If {
            condition: Box::new(Expression::Literal {
                value: Scalar::Bool(false),
            }),
            consequent: Box::new(Expression::Old),
            alternative: Box::new(Expression::Literal {
                value: Scalar::Int(42),
            }),
        };
        assert_eq!(expression.eval(empty, &mut arena).unwrap(), Scalar::Int(42));
        assert_eq!(arena.work, 0);
    }
}
