use crate::CoreError;
use crate::ir::{
    Binding, BodyKind, Branch, Definition, DefinitionBody, Domain, Expression, SafetyProgram, Step,
    TemporalFormula, validate_wire_index,
};
use crate::safety_chains::chain_terms;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Default, PartialEq, Eq)]
struct Summary {
    parameters: BTreeMap<usize, u8>,
    captures: BTreeMap<usize, u8>,
    contextual: bool,
    stateful: bool,
}

impl Summary {
    fn merge(&mut self, other: Self) {
        self.contextual |= other.contextual;
        self.stateful |= other.stateful;
        for (slot, phases) in other.parameters {
            *self.parameters.entry(slot).or_default() |= phases;
        }
        for (slot, phases) in other.captures {
            *self.captures.entry(slot).or_default() |= phases;
        }
    }
}

struct DefinitionSignature {
    kind: BodyKind,
    parameter_count: usize,
    capture_count: usize,
}

pub(crate) struct Metadata {
    definitions: Vec<DefinitionSignature>,
    summaries: Vec<[Option<Summary>; 4]>,
    variables: usize,
}

#[derive(Clone, Copy)]
struct Context {
    phase: u8,
    depth: usize,
    nodes: usize,
    locals: usize,
    capacity: usize,
    parameters: usize,
    captures: usize,
    old: bool,
}

type ContextKey = (u8, usize, usize, usize, usize, usize, bool);

impl Context {
    fn key(self) -> ContextKey {
        (
            self.phase,
            self.depth,
            self.locals,
            self.capacity,
            self.parameters,
            self.captures,
            self.old,
        )
    }
}

impl Context {
    fn root(phase: u8) -> Self {
        Self {
            phase,
            depth: 0,
            nodes: 0,
            locals: 0,
            capacity: 40,
            parameters: 0,
            captures: 0,
            old: false,
        }
    }
    fn child(self) -> Self {
        Self {
            depth: self.depth + 1,
            nodes: self.nodes + 1,
            ..self
        }
    }
    fn term(self, offset: usize) -> Self {
        Self {
            depth: self.depth + 1,
            nodes: self.nodes + offset,
            ..self
        }
    }
    fn mode(self) -> usize {
        usize::from(self.phase)
    }
}

fn invalid(message: &str) -> CoreError {
    CoreError::protocol(message)
}

const IR_NODE_LIMIT: usize = 128;

fn ir_nesting() -> CoreError {
    invalid("Safety IR expression nesting exceeds 128")
}

fn branch_expansion(branches: &[Branch], depth: usize) -> Result<(usize, usize), CoreError> {
    if depth > 48 {
        return Err(invalid("Safety expression nesting exceeds 48"));
    }
    let mut count = 0usize;
    let mut maximum = 0usize;
    for branch in branches {
        let mut alternatives = 1usize;
        let mut steps = branch.steps.len();
        for step in &branch.steps {
            if let Step::Frame { branches, .. } = step {
                let (inner_steps, inner_count) = branch_expansion(branches, depth + 1)?;
                steps = steps.saturating_add(inner_steps);
                alternatives = alternatives.saturating_mul(inner_count);
            }
        }
        maximum = maximum.max(steps);
        count = count.saturating_add(alternatives);
    }
    if maximum > 256 {
        return Err(invalid("Safety branch exceeds 256 steps"));
    }
    if count > 4096 {
        return Err(invalid("Safety expansion exceeds 4096 branches"));
    }
    Ok((maximum, count))
}

fn definition_order(definitions: &[Definition]) -> Result<Vec<(Vec<usize>, bool)>, CoreError> {
    let mut edges = vec![vec![]; definitions.len()];
    for (index, definition) in definitions.iter().enumerate() {
        for count in [
            definition.id,
            definition.parameter_count,
            definition.capture_count,
            definition.local_scope,
        ] {
            validate_wire_index(count)?;
        }
        if definition.id != index || definition.local_scope > 40 {
            return Err(invalid("Invalid safety definition ID or local scope"));
        }
        enum Node<'a> {
            Expression(&'a Expression, usize, usize),
            Branch(&'a Branch, usize, usize),
            Formula(&'a TemporalFormula, usize),
        }
        let mut pending = match &definition.body {
            DefinitionBody::Temporal(body)
                if definition.body_kind == BodyKind::Temporal && !definition.recursive =>
            {
                vec![Node::Formula(body, 0)]
            }
            DefinitionBody::Expression(body) if definition.body_kind == BodyKind::Expression => {
                vec![Node::Expression(body, 0, 0)]
            }
            DefinitionBody::Action(branches) if definition.body_kind == BodyKind::Action => {
                branches
                    .iter()
                    .map(|branch| Node::Branch(branch, 0, 0))
                    .collect()
            }
            _ => return Err(invalid("Invalid safety definition body kind")),
        };
        let mut call = |id: usize,
                        arguments: &[Expression],
                        captures: &[Expression],
                        kind|
         -> Result<(), CoreError> {
            validate_wire_index(id)?;
            let callee = definitions
                .get(id)
                .ok_or_else(|| invalid("Unknown safety definition ID"))?;
            if callee.body_kind != kind {
                return Err(invalid("Invalid safety call body kind"));
            }
            if arguments.len() != callee.parameter_count || captures.len() != callee.capture_count {
                return Err(invalid("Invalid safety call arity or capture count"));
            }
            edges[index].push(id);
            Ok(())
        };
        while let Some(node) = pending.pop() {
            match node {
                Node::Formula(formula, depth) => {
                    if depth > 40 {
                        return Err(invalid("Temporal formula nesting exceeds 40"));
                    }
                    match formula {
                        TemporalFormula::Call {
                            definition,
                            arguments,
                            captures,
                        } => {
                            call(*definition, arguments, captures, BodyKind::Temporal)?;
                            pending.extend(
                                arguments
                                    .iter()
                                    .chain(captures)
                                    .map(|expression| Node::Expression(expression, 0, 0)),
                            );
                        }
                        TemporalFormula::Frame {
                            arguments,
                            captures,
                            formula,
                            ..
                        } => {
                            pending.extend(
                                arguments
                                    .iter()
                                    .chain(captures)
                                    .map(|expression| Node::Expression(expression, 0, 0)),
                            );
                            pending.push(Node::Formula(formula, depth + 1));
                        }
                        TemporalFormula::Atom { expression } => {
                            pending.push(Node::Expression(expression, 0, 0))
                        }
                        TemporalFormula::Not { formula }
                        | TemporalFormula::Always { formula }
                        | TemporalFormula::Eventually { formula } => {
                            pending.push(Node::Formula(formula, depth + 1))
                        }
                        TemporalFormula::And { left, right }
                        | TemporalFormula::Or { left, right } => pending.extend([
                            Node::Formula(left, depth + 1),
                            Node::Formula(right, depth + 1),
                        ]),
                        TemporalFormula::Forall { bounds, formula }
                        | TemporalFormula::Exists { bounds, formula } => {
                            for binding in bounds {
                                pending.extend(
                                    binding
                                        .domain
                                        .children()
                                        .into_iter()
                                        .map(|expression| Node::Expression(expression, 0, 0)),
                                );
                            }
                            pending.push(Node::Formula(formula, depth + 1));
                        }
                    }
                }
                Node::Expression(expression, depth, nodes) => {
                    if depth > 48 {
                        return Err(invalid("Safety expression nesting exceeds 48"));
                    }
                    if nodes > IR_NODE_LIMIT {
                        return Err(ir_nesting());
                    }
                    if let Some(terms) = chain_terms(expression) {
                        pending.extend(terms.into_iter().map(|(term, offset)| {
                            Node::Expression(term, depth + 1, nodes + offset)
                        }));
                        continue;
                    }
                    if let Expression::Call {
                        definition,
                        arguments,
                        captures,
                    } = expression
                    {
                        call(*definition, arguments, captures, BodyKind::Expression)?;
                    }
                    if let Expression::Enabled {
                        branches,
                        subscript,
                    } = expression
                    {
                        pending.extend(
                            branches
                                .iter()
                                .map(|branch| Node::Branch(branch, depth + 1, nodes + 1)),
                        );
                        pending.extend(
                            subscript
                                .iter()
                                .map(|body| Node::Expression(body, depth + 1, nodes + 1)),
                        );
                    } else {
                        pending.extend(
                            expression
                                .children()
                                .into_iter()
                                .map(|body| Node::Expression(body, depth + 1, nodes + 1)),
                        );
                    }
                }
                Node::Branch(branch, depth, nodes) => {
                    if depth > 48 {
                        return Err(invalid("Safety expression nesting exceeds 48"));
                    }
                    for step in &branch.steps {
                        match step {
                            Step::Call {
                                definition,
                                arguments,
                                captures,
                            } => {
                                call(*definition, arguments, captures, BodyKind::Action)?;
                                pending.extend(
                                    arguments
                                        .iter()
                                        .chain(captures)
                                        .map(|body| Node::Expression(body, depth + 1, nodes + 1)),
                                );
                            }
                            Step::Frame {
                                arguments,
                                captures,
                                branches,
                                ..
                            } => {
                                pending.extend(
                                    arguments
                                        .iter()
                                        .chain(captures)
                                        .map(|body| Node::Expression(body, depth + 1, nodes + 1)),
                                );
                                pending.extend(
                                    branches
                                        .iter()
                                        .map(|branch| Node::Branch(branch, depth + 1, nodes + 1)),
                                );
                            }
                            Step::Guard { expression }
                            | Step::Assign {
                                value: expression, ..
                            } => pending.push(Node::Expression(expression, depth + 1, nodes + 1)),
                            Step::Bind { domain, .. } | Step::Choose { domain, .. } => pending
                                .extend(
                                    domain
                                        .children()
                                        .into_iter()
                                        .map(|body| Node::Expression(body, depth + 1, nodes + 1)),
                                ),
                        }
                    }
                }
            }
        }
    }

    let mut colors = vec![0_u8; definitions.len()];
    let mut order = vec![];
    for root in 0..definitions.len() {
        if colors[root] != 0 {
            continue;
        }
        colors[root] = 1;
        let mut pending = vec![(root, 0)];
        while let Some((definition, edge)) = pending.last_mut() {
            if let Some(&callee) = edges[*definition].get(*edge) {
                *edge += 1;
                match colors[callee] {
                    0 => {
                        colors[callee] = 1;
                        pending.push((callee, 0));
                    }
                    1 => {}
                    _ => {}
                }
            } else {
                colors[*definition] = 2;
                order.push(*definition);
                pending.pop();
            }
        }
    }
    let mut reverse = vec![vec![]; definitions.len()];
    for (caller, callees) in edges.iter().enumerate() {
        for callee in callees {
            reverse[*callee].push(caller);
        }
    }
    let mut assigned = vec![false; definitions.len()];
    let mut components = vec![];
    for root in order.into_iter().rev() {
        if assigned[root] {
            continue;
        }
        assigned[root] = true;
        let mut ids = vec![];
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            ids.push(id);
            for caller in &reverse[id] {
                if !assigned[*caller] {
                    assigned[*caller] = true;
                    pending.push(*caller);
                }
            }
        }
        let recursive = ids.len() > 1 || edges[ids[0]].contains(&ids[0]);
        if recursive && ids.iter().any(|id| !definitions[*id].recursive) {
            return Err(invalid("Recursive safety definition graph"));
        }
        components.push((ids, recursive));
    }
    components.reverse();
    Ok(components)
}

type CachedAnalysis = Result<Summary, (&'static str, String)>;

struct Analyzer<'metadata> {
    metadata: &'metadata Metadata,
    cache: HashMap<(usize, ContextKey), CachedAnalysis>,
}

impl<'metadata> Analyzer<'metadata> {
    fn new(metadata: &'metadata Metadata) -> Self {
        Self {
            metadata,
            cache: HashMap::new(),
        }
    }
}

impl Analyzer<'_> {
    fn domain(&mut self, domain: &Domain, context: Context) -> Result<Summary, CoreError> {
        let mut summary = Summary::default();
        for child in domain.children() {
            summary.merge(self.expression(child, context)?);
        }
        Ok(summary)
    }

    fn bindings(
        &mut self,
        bounds: &[Binding],
        context: Context,
    ) -> Result<(Summary, Context), CoreError> {
        if bounds.is_empty()
            || bounds.len() > 40
            || context.locals + bounds.len() > context.capacity
        {
            return Err(invalid("Invalid safety binding count"));
        }
        let mut child = context;
        let mut summary = Summary::default();
        for bound in bounds {
            validate_wire_index(bound.slot)?;
            if let Some(size) = bound.tuple_size {
                validate_wire_index(size)?;
            }
            if bound.slot != child.locals || bound.tuple_size.is_some_and(|size| size > 256) {
                return Err(invalid("Invalid bound variable slot"));
            }
            summary.merge(self.domain(&bound.domain, child)?);
            child.locals += 1;
        }
        Ok((summary, child))
    }

    fn branches(&mut self, branches: &[Branch], context: Context) -> Result<Summary, CoreError> {
        branch_expansion(branches, context.depth)?;
        let mut summary = Summary::default();
        for branch in branches {
            if branch.steps.len() > 256 {
                return Err(invalid("Safety branch exceeds 256 steps"));
            }
            let mut context = context;
            for step in &branch.steps {
                match step {
                    Step::Bind {
                        slot, tuple_size, ..
                    } => {
                        validate_wire_index(*slot)?;
                        if let Some(size) = tuple_size {
                            validate_wire_index(*size)?;
                        }
                    }
                    Step::Assign { slot, .. } | Step::Choose { slot, .. } => {
                        validate_wire_index(*slot)?
                    }
                    _ => {}
                }
                match step {
                    Step::Frame {
                        arguments,
                        captures,
                        parameter_count,
                        capture_count,
                        local_scope,
                        branches,
                    } => {
                        let child = self.frame_context(
                            *parameter_count,
                            *capture_count,
                            *local_scope,
                            context,
                        )?;
                        let required = self.branches(branches, child)?;
                        summary.merge(self.actuals(
                            arguments,
                            captures,
                            *parameter_count,
                            *capture_count,
                            required,
                            context,
                        )?);
                    }
                    Step::Call {
                        definition,
                        arguments,
                        captures,
                    } => {
                        summary.merge(self.call(
                            *definition,
                            arguments,
                            captures,
                            BodyKind::Action,
                            context,
                        )?);
                    }
                    Step::Guard { expression } => {
                        summary.merge(self.expression(expression, context)?)
                    }
                    Step::Assign { slot, value } => {
                        if *slot >= self.metadata.variables {
                            return Err(invalid("Invalid safety assignment slot"));
                        }
                        summary.merge(self.expression(value, context)?);
                    }
                    Step::Choose { slot, domain } => {
                        if *slot >= self.metadata.variables {
                            return Err(invalid("Invalid safety assignment slot"));
                        }
                        summary.merge(self.domain(domain, context)?);
                    }
                    Step::Bind {
                        slot,
                        domain,
                        tuple_size,
                    } => {
                        if *slot != context.locals
                            || context.locals >= context.capacity
                            || tuple_size.is_some_and(|size| size > 256)
                        {
                            return Err(invalid("Invalid bound variable slot"));
                        }
                        summary.merge(self.domain(domain, context)?);
                        context.locals += 1;
                    }
                }
            }
        }
        Ok(summary)
    }

    fn frame_context(
        &self,
        parameters: usize,
        captures: usize,
        capacity: usize,
        context: Context,
    ) -> Result<Context, CoreError> {
        for count in [parameters, captures, capacity] {
            validate_wire_index(count)?;
        }
        if capacity > 40 {
            return Err(invalid("Invalid safety frame local scope"));
        }
        Ok(Context {
            parameters,
            captures,
            capacity,
            locals: 0,
            old: false,
            ..context.child()
        })
    }

    fn actuals(
        &mut self,
        arguments: &[Expression],
        captures: &[Expression],
        parameters: usize,
        capture_count: usize,
        required: Summary,
        context: Context,
    ) -> Result<Summary, CoreError> {
        if arguments.len() != parameters || captures.len() != capture_count {
            return Err(invalid("Invalid safety frame arity or capture count"));
        }
        let mut summary = Summary {
            contextual: required.contextual,
            stateful: required.stateful,
            ..Summary::default()
        };
        for (actuals, uses) in [
            (arguments, &required.parameters),
            (captures, &required.captures),
        ] {
            for (slot, actual) in actuals.iter().enumerate() {
                let phases = uses.get(&slot).copied().unwrap_or_default();
                if phases == 0 {
                    self.expression(actual, context.child())?;
                }
                for phase in 0..4 {
                    if phases & (1 << phase) != 0 {
                        summary.merge(self.expression(
                            actual,
                            Context {
                                phase,
                                ..context.child()
                            },
                        )?);
                    }
                }
            }
        }
        Ok(summary)
    }

    fn call(
        &mut self,
        definition: usize,
        arguments: &[Expression],
        captures: &[Expression],
        kind: BodyKind,
        context: Context,
    ) -> Result<Summary, CoreError> {
        validate_wire_index(definition)?;
        let callee = self
            .metadata
            .definitions
            .get(definition)
            .ok_or_else(|| invalid("Unknown safety definition ID"))?;
        if callee.kind != kind {
            return Err(invalid("Invalid safety call body kind"));
        }
        if arguments.len() != callee.parameter_count || captures.len() != callee.capture_count {
            return Err(invalid("Invalid safety call arity or capture count"));
        }
        let required = self.metadata.summaries[definition][context.mode()]
            .clone()
            .ok_or_else(|| invalid("Invalid safety call phase"))?;
        self.actuals(
            arguments,
            captures,
            callee.parameter_count,
            callee.capture_count,
            required,
            context,
        )
    }

    fn expression(
        &mut self,
        expression: &Expression,
        context: Context,
    ) -> Result<Summary, CoreError> {
        let key = (std::ptr::from_ref(expression) as usize, context.key());
        if let Some(result) = self.cache.get(&key) {
            return result
                .clone()
                .map_err(|(code, message)| CoreError { code, message });
        }
        let result = self.expression_uncached(expression, context);
        self.cache.insert(
            key,
            result
                .as_ref()
                .cloned()
                .map_err(|error| (error.code, error.message.clone())),
        );
        result
    }

    fn expression_uncached(
        &mut self,
        expression: &Expression,
        context: Context,
    ) -> Result<Summary, CoreError> {
        if context.depth > 48 {
            return Err(invalid("Safety expression nesting exceeds 48"));
        }
        if context.nodes > IR_NODE_LIMIT {
            return Err(ir_nesting());
        }
        if let Some(terms) = chain_terms(expression) {
            let mut summary = Summary::default();
            for (term, offset) in terms {
                summary.merge(self.expression(term, context.term(offset))?);
            }
            return Ok(summary);
        }
        match expression {
            Expression::Call { definition, .. } => validate_wire_index(*definition)?,
            Expression::Parameter { slot }
            | Expression::Capture { slot }
            | Expression::Bound { slot }
            | Expression::Read { slot, .. } => validate_wire_index(*slot)?,
            Expression::Fold { accumulator, .. } => validate_wire_index(*accumulator)?,
            _ => {}
        }
        let mut summary = Summary::default();
        match expression {
            Expression::Frame {
                arguments,
                captures,
                parameter_count,
                capture_count,
                local_scope,
                body,
            } => {
                let child =
                    self.frame_context(*parameter_count, *capture_count, *local_scope, context)?;
                let required = self.expression(body, child)?;
                return self.actuals(
                    arguments,
                    captures,
                    *parameter_count,
                    *capture_count,
                    required,
                    context,
                );
            }
            Expression::Call {
                definition,
                arguments,
                captures,
            } => {
                return self.call(
                    *definition,
                    arguments,
                    captures,
                    BodyKind::Expression,
                    context,
                );
            }
            Expression::Parameter { slot } => {
                if *slot >= context.parameters {
                    return Err(invalid("Invalid safety parameter reference"));
                }
                summary.parameters.insert(*slot, 1 << context.phase);
                return Ok(summary);
            }
            Expression::Capture { slot } => {
                if *slot >= context.captures {
                    return Err(invalid("Invalid safety capture reference"));
                }
                summary.captures.insert(*slot, 1 << context.phase);
                return Ok(summary);
            }
            Expression::Prime { argument } => {
                if context.phase != 1 {
                    return Err(invalid("Prime requires an action context"));
                }
                summary.merge(self.expression(
                    argument,
                    Context {
                        phase: 2,
                        ..context.child()
                    },
                )?);
                summary.contextual = true;
                return Ok(summary);
            }
            Expression::Read { slot, next } => {
                if *slot >= self.metadata.variables
                    || (context.phase == 0 && !*next)
                    || (context.phase == 2 && *next)
                    || context.phase == 3
                {
                    return Err(invalid("Invalid safety variable read"));
                }
                summary.contextual = true;
                summary.stateful = true;
            }
            Expression::Bound { slot } => {
                if *slot >= context.locals {
                    return Err(invalid("Unbound quantified variable"));
                }
                summary.contextual = true;
            }
            Expression::Old => {
                if !context.old {
                    return Err(invalid("Unbound EXCEPT @"));
                }
                summary.contextual = true;
            }
            Expression::UnmatchedCase { location } => {
                if location.is_empty() {
                    return Err(invalid("Invalid unmatched CASE location"));
                }
                summary.stateful = true;
                summary.contextual = true;
            }
            Expression::Choose { binding, body } => {
                let (binding_summary, child) =
                    self.bindings(std::slice::from_ref(binding), context.child())?;
                summary.merge(binding_summary);
                summary.merge(self.expression(body, child)?);
                return Ok(summary);
            }
            Expression::Forall { bounds, body }
            | Expression::Exists { bounds, body }
            | Expression::Function { bounds, body }
            | Expression::Filter { bounds, body }
            | Expression::MapSet { bounds, body } => {
                let (binding_summary, child) = self.bindings(bounds, context.child())?;
                summary.merge(binding_summary);
                summary.merge(self.expression(body, child)?);
                return Ok(summary);
            }
            Expression::Fold {
                accumulator,
                initial,
                collection,
                body,
                ..
            } => {
                if *accumulator != context.locals || context.locals + 2 > context.capacity {
                    return Err(invalid("Invalid fold variable slot"));
                }
                summary.merge(self.expression(initial, context.child())?);
                summary.merge(self.expression(collection, context.child())?);
                summary.merge(self.expression(
                    body,
                    Context {
                        locals: context.locals + 2,
                        ..context.child()
                    },
                )?);
                return Ok(summary);
            }
            Expression::SelectSeq {
                slot,
                sequence,
                body,
            }
            | Expression::SortSeq {
                slot,
                sequence,
                body,
            } => {
                validate_wire_index(*slot)?;
                let arity = if matches!(expression, Expression::SortSeq { .. }) {
                    2
                } else {
                    1
                };
                if *slot != context.locals || context.locals + arity > context.capacity {
                    return Err(invalid("Invalid sequence callback variable slot"));
                }
                summary.merge(self.expression(sequence, context.child())?);
                summary.merge(self.expression(
                    body,
                    Context {
                        locals: context.locals + arity,
                        ..context.child()
                    },
                )?);
                return Ok(summary);
            }
            Expression::Update { target, updates } => {
                summary.merge(self.expression(target, context.child())?);
                for update in updates {
                    if update.path.is_empty() || update.path.len() > 48 {
                        return Err(invalid("Empty EXCEPT path"));
                    }
                    for part in &update.path {
                        summary.merge(self.expression(part, context.child())?);
                    }
                    summary.merge(self.expression(
                        &update.value,
                        Context {
                            old: true,
                            ..context.child()
                        },
                    )?);
                }
                return Ok(summary);
            }
            Expression::Enabled {
                branches,
                subscript,
            } => {
                if context.phase == 3 {
                    return Err(invalid("ENABLED is not a constant"));
                }
                summary.merge(self.branches(
                    branches,
                    Context {
                        phase: 1,
                        ..context.child()
                    },
                )?);
                if let Some(subscript) = subscript {
                    summary.merge(self.expression(
                        subscript,
                        Context {
                            phase: 2,
                            ..context.child()
                        },
                    )?);
                }
                summary.stateful = true;
                summary.contextual = true;
                return Ok(summary);
            }
            _ => {}
        }
        for child in expression.children() {
            summary.merge(self.expression(child, context.child())?);
        }
        Ok(summary)
    }

    fn formula(
        &mut self,
        formula: &TemporalFormula,
        depth: usize,
        context: Context,
    ) -> Result<Summary, CoreError> {
        if depth > 40 {
            return Err(invalid("Temporal formula nesting exceeds 40"));
        }
        let mut summary = Summary::default();
        match formula {
            TemporalFormula::Call {
                definition,
                arguments,
                captures,
            } => {
                return self.call(
                    *definition,
                    arguments,
                    captures,
                    BodyKind::Temporal,
                    context,
                );
            }
            TemporalFormula::Frame {
                arguments,
                captures,
                parameter_count,
                capture_count,
                local_scope,
                formula,
            } => {
                let child =
                    self.frame_context(*parameter_count, *capture_count, *local_scope, context)?;
                let required = self.formula(formula, depth + 1, child)?;
                return self.actuals(
                    arguments,
                    captures,
                    *parameter_count,
                    *capture_count,
                    required,
                    context,
                );
            }
            TemporalFormula::Atom { expression } => {
                summary.merge(self.expression(expression, context)?);
            }
            TemporalFormula::Not { formula }
            | TemporalFormula::Always { formula }
            | TemporalFormula::Eventually { formula } => {
                summary.merge(self.formula(formula, depth + 1, context)?);
            }
            TemporalFormula::And { left, right } | TemporalFormula::Or { left, right } => {
                summary.merge(self.formula(left, depth + 1, context)?);
                summary.merge(self.formula(right, depth + 1, context)?);
            }
            TemporalFormula::Forall { bounds, formula }
            | TemporalFormula::Exists { bounds, formula } => {
                if bounds.is_empty() || context.locals + bounds.len() > context.capacity {
                    return Err(invalid("Invalid temporal binding count"));
                }
                for (index, bound) in bounds.iter().enumerate() {
                    if bound.slot != context.locals + index
                        || bound.tuple_size.is_some_and(|size| size > 256)
                    {
                        return Err(invalid("Invalid temporal binding slot"));
                    }
                    summary.merge(self.domain(
                        &bound.domain,
                        Context {
                            phase: 3,
                            ..context
                        },
                    )?);
                }
                summary.merge(self.formula(
                    formula,
                    depth + 1,
                    Context {
                        locals: context.locals + bounds.len(),
                        ..context
                    },
                )?);
            }
        }
        Ok(summary)
    }
}

impl Metadata {
    fn expression(&self, expression: &Expression, context: Context) -> Result<Summary, CoreError> {
        Analyzer::new(self).expression(expression, context)
    }

    fn branches(&self, branches: &[Branch], context: Context) -> Result<Summary, CoreError> {
        Analyzer::new(self).branches(branches, context)
    }

    fn formula(
        &self,
        formula: &TemporalFormula,
        depth: usize,
        context: Context,
    ) -> Result<Summary, CoreError> {
        Analyzer::new(self).formula(formula, depth, context)
    }

    pub(crate) fn is_constant(&self, expression: &Expression) -> bool {
        self.expression(expression, Context::root(1))
            .is_ok_and(|summary| {
                !summary.stateful && summary.parameters.is_empty() && summary.captures.is_empty()
            })
    }

    pub(crate) fn is_context_free(&self, expression: &Expression) -> bool {
        self.expression(expression, Context::root(1))
            .is_ok_and(|summary| {
                !summary.contextual && summary.parameters.is_empty() && summary.captures.is_empty()
            })
    }
}

pub(crate) fn validate_program(program: &SafetyProgram) -> Result<Metadata, CoreError> {
    validate_wire_index(program.variables.len())?;
    let definitions = program
        .definitions
        .as_deref()
        .ok_or_else(|| invalid("Missing safety IR v2 definitions"))?;
    validate_wire_index(definitions.len())?;
    let order = definition_order(definitions)?;
    let mut metadata = Metadata {
        definitions: definitions
            .iter()
            .map(|definition| DefinitionSignature {
                kind: definition.body_kind,
                parameter_count: definition.parameter_count,
                capture_count: definition.capture_count,
            })
            .collect(),
        summaries: (0..definitions.len())
            .map(|_| std::array::from_fn(|_| None))
            .collect(),
        variables: program.variables.len(),
    };
    for (ids, recursive) in order {
        if recursive {
            for id in &ids {
                metadata.summaries[*id] = std::array::from_fn(|_| Some(Summary::default()));
            }
        }
        let mut errors = HashMap::new();
        loop {
            let mut changed = false;
            for id in &ids {
                let definition = &definitions[*id];
                let mut summaries = std::array::from_fn(|_| None);
                for (phase, target) in summaries.iter_mut().enumerate() {
                    if recursive && metadata.summaries[*id][phase].is_none() {
                        continue;
                    }
                    let context = Context {
                        parameters: definition.parameter_count,
                        captures: definition.capture_count,
                        capacity: definition.local_scope,
                        ..Context::root(phase as u8)
                    };
                    let result = match &definition.body {
                        DefinitionBody::Temporal(formula) if phase == 1 => {
                            metadata.formula(formula, 0, context)
                        }
                        DefinitionBody::Temporal(_) => {
                            Err(invalid("Invalid safety temporal definition phase"))
                        }
                        DefinitionBody::Expression(body) => metadata.expression(body, context),
                        DefinitionBody::Action(branches) if phase <= 1 => {
                            metadata.branches(branches, context).map(|mut summary| {
                                summary.stateful = true;
                                summary
                            })
                        }
                        DefinitionBody::Action(_) => {
                            Err(invalid("Invalid safety action definition phase"))
                        }
                    };
                    match result {
                        Ok(summary) => *target = Some(summary),
                        Err(error) => {
                            errors.entry(*id).or_insert(error);
                        }
                    }
                }
                changed |= metadata.summaries[*id] != summaries;
                metadata.summaries[*id] = summaries;
            }
            if !recursive || !changed {
                break;
            }
        }
        for id in ids {
            if metadata.summaries[id].iter().all(Option::is_none) {
                return Err(errors
                    .remove(&id)
                    .unwrap_or_else(|| invalid("Invalid safety definition")));
            }
        }
    }
    metadata.branches(&program.initial, Context::root(0))?;
    metadata.branches(&program.next, Context::root(1))?;
    for (predicates, phase) in [
        (&program.invariants, 2),
        (&program.initial_properties, 2),
        (&program.constraints, 2),
        (&program.action_properties, 1),
        (&program.action_constraints, 1),
        (&program.assumptions, 3),
    ] {
        for predicate in predicates {
            metadata.expression(&predicate.expression, Context::root(phase))?;
        }
    }
    for formula in program
        .properties
        .iter()
        .map(|property| &property.formula)
        .chain(&program.fairness)
    {
        metadata.formula(formula, 0, Context::root(1))?;
    }
    Ok(metadata)
}
