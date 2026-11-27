use crate::CoreError;
use crate::ir::{
    DefinitionBody, Expression, SafetyBounds, SafetyProgram, SafetyTraceStep, SafetyViolation,
    Scalar, TemporalFormula,
};
use crate::safety_expressions::{
    CallFrame, EvalContext, validate_constant_bindings, visit_bindings,
};
use crate::safety_values::Arena;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

impl TemporalFormula {
    pub(crate) fn validate(&self, variables: usize, depth: usize) -> Result<(), CoreError> {
        self.validate_locals(variables, depth, 0)
    }
    fn validate_locals(
        &self,
        variables: usize,
        depth: usize,
        locals: usize,
    ) -> Result<(), CoreError> {
        if depth > 40 {
            return Err(CoreError::protocol("Temporal formula nesting exceeds 40"));
        }
        match self {
            Self::Frame { .. } | Self::Call { .. } => {
                return Err(CoreError::protocol(
                    "Operator references require safety IR v2",
                ));
            }
            Self::Atom { expression } => expression.validate_locals(variables, 1, locals)?,
            Self::Not { formula } | Self::Always { formula } | Self::Eventually { formula } => {
                formula.validate_locals(variables, depth + 1, locals)?
            }
            Self::And { left, right } | Self::Or { left, right } => {
                left.validate_locals(variables, depth + 1, locals)?;
                right.validate_locals(variables, depth + 1, locals)?;
            }
            Self::Forall { bounds, formula } | Self::Exists { bounds, formula } => {
                let locals = validate_constant_bindings(bounds, variables, locals)?;
                formula.validate_locals(variables, depth + 1, locals)?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(crate) struct GraphEdge {
    pub target: usize,
    pub action: Option<usize>,
}

struct LexicalSnapshot {
    locals: Vec<Scalar>,
    frame: Option<Arc<CallFrame>>,
    old: Option<Scalar>,
}

enum Formula<'a> {
    True,
    False,
    Atom(&'a Expression, bool, LexicalSnapshot),
    And(usize, usize),
    Or(usize, usize),
    Until(usize, usize),
    Release(usize, usize),
    Recurring(EdgePredicate),
}

/// A propositional condition on the current graph edge.
#[derive(Clone, Copy)]
enum EdgePredicate {
    Atom(usize),
    Either(usize, usize),
}

struct Pool<'a> {
    formulas: Vec<Formula<'a>>,
    definitions: &'a [crate::ir::Definition],
    acceptance: Vec<usize>,
    frame_sequence: usize,
    entries: Vec<Vec<usize>>,
    active_entries: Vec<usize>,
    lower_entered: HashSet<usize>,
}
impl<'a> Pool<'a> {
    fn add(&mut self, formula: Formula<'a>) -> usize {
        match &formula {
            Formula::And(left, right) => {
                if matches!(self.formulas[*left], Formula::False)
                    || matches!(self.formulas[*right], Formula::True)
                {
                    return *left;
                }
                if matches!(self.formulas[*right], Formula::False)
                    || matches!(self.formulas[*left], Formula::True)
                {
                    return *right;
                }
            }
            Formula::Or(left, right) => {
                if matches!(self.formulas[*left], Formula::True)
                    || matches!(self.formulas[*right], Formula::False)
                {
                    return *left;
                }
                if matches!(self.formulas[*right], Formula::True)
                    || matches!(self.formulas[*left], Formula::False)
                {
                    return *right;
                }
            }
            _ => {}
        }
        let index = self.formulas.len();
        if matches!(formula, Formula::Until(..)) {
            self.acceptance.push(index);
        }
        self.formulas.push(formula);
        self.entries.push(self.active_entries.clone());
        index
    }
    fn edge_predicate(&self, id: usize) -> Option<EdgePredicate> {
        let is_atom = |id: usize| matches!(self.formulas[id], Formula::Atom(..));
        match self.formulas[id] {
            Formula::Atom(..) => Some(EdgePredicate::Atom(id)),
            Formula::Or(left, right) if is_atom(left) && is_atom(right) => {
                Some(EdgePredicate::Either(left, right))
            }
            _ => None,
        }
    }

    fn lower(
        &mut self,
        formula: &'a TemporalFormula,
        negative: bool,
        context: EvalContext<'_>,
        entries: &[usize],
        arena: &mut Arena,
    ) -> Result<usize, CoreError> {
        let previous = std::mem::replace(&mut self.active_entries, entries.to_vec());
        let result = self.lower_inner(formula, negative, context, entries, arena);
        self.active_entries = previous;
        result
    }
    fn lower_inner(
        &mut self,
        formula: &'a TemporalFormula,
        negative: bool,
        mut context: EvalContext<'_>,
        entries: &[usize],
        arena: &mut Arena,
    ) -> Result<usize, CoreError> {
        context.descend(arena)?;
        arena.tick()?;
        Ok(match formula {
            TemporalFormula::Call {
                arguments,
                captures,
                definition,
            } => {
                let definitions = self.definitions;
                let DefinitionBody::Temporal(formula) = &definitions
                    .get(*definition)
                    .ok_or_else(|| CoreError::protocol("Unknown safety definition ID"))?
                    .body
                else {
                    return Err(CoreError::protocol("Invalid safety call body kind"));
                };
                let frame = CallFrame::new(arguments, captures, context);
                let mut entries = entries.to_vec();
                entries.push(self.frame_sequence);
                self.frame_sequence += 1;
                self.lower(
                    formula,
                    negative,
                    EvalContext {
                        locals: &[],
                        frame: Some(&frame),
                        old: None,
                        ..context
                    },
                    &entries,
                    arena,
                )?
            }
            TemporalFormula::Frame {
                arguments,
                captures,
                formula,
                ..
            } => {
                let frame = CallFrame::new(arguments, captures, context);
                let mut entries = entries.to_vec();
                entries.push(self.frame_sequence);
                self.frame_sequence += 1;
                self.lower(
                    formula,
                    negative,
                    EvalContext {
                        locals: &[],
                        frame: Some(&frame),
                        old: None,
                        ..context
                    },
                    &entries,
                    arena,
                )?
            }
            TemporalFormula::Atom { expression } => self.add(Formula::Atom(
                expression,
                negative,
                LexicalSnapshot {
                    locals: context.locals.to_vec(),
                    frame: context.frame.cloned().map(Arc::new),
                    old: context.old,
                },
            )),
            TemporalFormula::Not { formula } => {
                self.lower(formula, !negative, context, entries, arena)?
            }
            TemporalFormula::And { left, right } | TemporalFormula::Or { left, right } => {
                let left = self.lower(left, negative, context, entries, arena)?;
                let right = self.lower(right, negative, context, entries, arena)?;
                if matches!(formula, TemporalFormula::And { .. }) != negative {
                    self.add(Formula::And(left, right))
                } else {
                    self.add(Formula::Or(left, right))
                }
            }
            TemporalFormula::Always { formula: inner }
            | TemporalFormula::Eventually { formula: inner } => {
                let child = self.lower(inner, negative, context, entries, arena)?;
                if matches!(formula, TemporalFormula::Eventually { .. }) != negative {
                    let truth = self.add(Formula::True);
                    self.add(Formula::Until(truth, child))
                } else if let Formula::Until(truth, condition) = self.formulas[child]
                    && matches!(self.formulas[truth], Formula::True)
                    && let Some(predicate) = self.edge_predicate(condition)
                {
                    // The freshly lowered eventuality has an acceptance
                    // entry. Reuse it for GF p, whose loop must visit p.
                    self.formulas[child] = Formula::Recurring(predicate);
                    child
                } else {
                    let falsity = self.add(Formula::False);
                    self.add(Formula::Release(falsity, child))
                }
            }
            TemporalFormula::Forall {
                bounds,
                formula: inner,
            }
            | TemporalFormula::Exists {
                bounds,
                formula: inner,
            } => {
                let conjunction = matches!(formula, TemporalFormula::Forall { .. }) != negative;
                let mut result = self.add(if conjunction {
                    Formula::True
                } else {
                    Formula::False
                });
                for entry in entries {
                    if self.lower_entered.insert(*entry) {
                        arena.consume(1)?;
                    }
                }
                visit_bindings(bounds, 0, context, arena, &mut |ctx, arena| {
                    let child = self.lower(inner, negative, ctx, entries, arena)?;
                    result = self.add(if conjunction {
                        Formula::And(result, child)
                    } else {
                        Formula::Or(result, child)
                    });
                    Ok(true)
                })?;
                result
            }
        })
    }
}
fn evaluate_atom(
    expression: &Expression,
    negative: bool,
    lexical: &LexicalSnapshot,
    ctx: EvalContext<'_>,
    arena: &mut Arena,
) -> Result<bool, CoreError> {
    let value = expression
        .eval(
            EvalContext {
                locals: &lexical.locals,
                frame: lexical.frame.as_deref(),
                old: lexical.old,
                ..ctx
            },
            arena,
        )?
        .boolean()?;
    Ok(value != negative)
}

impl EdgePredicate {
    fn evaluate(
        self,
        pool: &Pool<'_>,
        ctx: EvalContext<'_>,
        entered: &mut HashSet<usize>,
        arena: &mut Arena,
    ) -> Result<bool, CoreError> {
        let mut atom = |id: usize, arena: &mut Arena| {
            arena.tick()?;
            enter(&pool.entries[id], entered, arena)?;
            let Formula::Atom(expression, negative, lexical) = &pool.formulas[id] else {
                unreachable!("checked edge predicate");
            };
            evaluate_atom(expression, *negative, lexical, ctx, arena)
        };
        match self {
            Self::Atom(id) => atom(id, arena),
            Self::Either(left, right) => {
                let left = atom(left, arena)?;
                let right = atom(right, arena)?;
                Ok(left || right)
            }
        }
    }
}

fn enter(
    entries: &[usize],
    entered: &mut HashSet<usize>,
    arena: &mut Arena,
) -> Result<(), CoreError> {
    for entry in entries {
        if entered.insert(*entry) {
            arena.consume(1)?;
        }
    }
    Ok(())
}

#[derive(Clone)]
enum Seen {
    Dense([u64; 4]),
    Sparse(HashSet<usize>),
}
impl Seen {
    fn new(formulas: usize) -> Self {
        if formulas <= 256 {
            Self::Dense([0; 4])
        } else {
            Self::Sparse(HashSet::new())
        }
    }
    fn insert(&mut self, id: usize) -> bool {
        match self {
            Self::Dense(words) => {
                let word = &mut words[id / 64];
                let mask = 1 << (id % 64);
                let fresh = *word & mask == 0;
                *word |= mask;
                fresh
            }
            Self::Sparse(set) => set.insert(id),
        }
    }
    fn contains(&self, id: usize) -> bool {
        match self {
            Self::Dense(words) => words[id / 64] & (1 << (id % 64)) != 0,
            Self::Sparse(set) => set.contains(&id),
        }
    }
}
#[derive(Clone)]
struct Expansion {
    todo: Vec<usize>,
    seen: Seen,
    entered: HashSet<usize>,
    next: Vec<usize>,
    pending: Vec<usize>,
}
struct Alternative {
    next: Vec<usize>,
    pending: Vec<usize>,
}
fn expand(
    pool: &Pool<'_>,
    required: &[usize],
    ctx: EvalContext<'_>,
    arena: &mut Arena,
) -> Result<Vec<Alternative>, CoreError> {
    let mut stack = vec![Expansion {
        todo: required.to_vec(),
        seen: Seen::new(pool.formulas.len()),
        entered: HashSet::new(),
        next: vec![],
        pending: vec![],
    }];
    let mut result = vec![];
    'alternative: while let Some(mut state) = stack.pop() {
        while let Some(id) = state.todo.pop() {
            arena.tick()?;
            if !state.seen.insert(id) {
                continue;
            }
            enter(&pool.entries[id], &mut state.entered, arena)?;
            match &pool.formulas[id] {
                Formula::True => {}
                Formula::False => continue 'alternative,
                Formula::Atom(expression, negative, lexical) => {
                    if !evaluate_atom(expression, *negative, lexical, ctx, arena)? {
                        continue 'alternative;
                    }
                }
                Formula::And(left, right) => {
                    state.todo.push(*right);
                    state.todo.push(*left);
                }
                Formula::Or(left, right) => {
                    let mut other = state.clone();
                    other.todo.push(*right);
                    stack.push(other);
                    state.todo.push(*left);
                }
                Formula::Until(left, right) => {
                    let mut other = state.clone();
                    other.todo.push(*left);
                    other.next.push(id);
                    other.pending.push(id);
                    stack.push(other);
                    state.todo.push(*right);
                }
                Formula::Recurring(predicate) => {
                    state.next.push(id);
                    if !predicate.evaluate(pool, ctx, &mut state.entered, arena)? {
                        state.pending.push(id);
                    }
                }
                Formula::Release(left, right) => {
                    let mut other = state.clone();
                    other.todo.push(*right);
                    other.next.push(id);
                    stack.push(other);
                    state.todo.push(*right);
                    state.todo.push(*left);
                }
            }
        }
        state.next.sort_unstable();
        state.next.dedup();
        state.pending.retain(|id| match pool.formulas[*id] {
            Formula::Until(_, right) => !state.seen.contains(right),
            Formula::Recurring(_) => true,
            _ => false,
        });
        state.pending.sort_unstable();
        state.pending.dedup();
        result.push(Alternative {
            next: state.next,
            pending: state.pending,
        });
    }
    Ok(result)
}
#[derive(Clone)]
struct ProductEdge {
    target: usize,
    action: Option<usize>,
    pending: Vec<usize>,
}
struct ProductNode {
    state: usize,
    required: Vec<usize>,
    parent: Option<(usize, usize)>,
}
struct Cycle {
    entry: usize,
    edges: Vec<(usize, usize)>,
}
struct Product<'a> {
    pool: Pool<'a>,
    nodes: Vec<ProductNode>,
    edges: Vec<Vec<ProductEdge>>,
    lookup: HashMap<(usize, Vec<usize>), usize>,
    maximum: usize,
}
impl Product<'_> {
    fn node(
        &mut self,
        state: usize,
        required: Vec<usize>,
        parent: Option<(usize, usize)>,
    ) -> Result<usize, CoreError> {
        if let Some(index) = self.lookup.get(&(state, required.clone())) {
            return Ok(*index);
        }
        if self.nodes.len() == self.maximum {
            return Err(CoreError {
                code: "temporal-budget",
                message: "Temporal product node budget exhausted".into(),
            });
        }
        let index = self.nodes.len();
        self.lookup.insert((state, required.clone()), index);
        self.nodes.push(ProductNode {
            state,
            required,
            parent,
        });
        self.edges.push(vec![]);
        Ok(index)
    }
    fn components(&self, arena: &mut Arena) -> Result<Vec<Vec<usize>>, CoreError> {
        let mut visited = vec![false; self.nodes.len()];
        let mut order = vec![];
        for root in 0..self.nodes.len() {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            let mut stack = vec![(root, 0)];
            while let Some((node, position)) = stack.last_mut() {
                arena.tick()?;
                if let Some(edge) = self.edges[*node].get(*position) {
                    *position += 1;
                    if !visited[edge.target] {
                        visited[edge.target] = true;
                        stack.push((edge.target, 0));
                    }
                } else {
                    order.push(*node);
                    stack.pop();
                }
            }
        }
        let mut reverse = vec![vec![]; self.nodes.len()];
        for (source, edges) in self.edges.iter().enumerate() {
            for edge in edges {
                arena.tick()?;
                reverse[edge.target].push(source);
            }
        }
        visited.fill(false);
        let mut components = vec![];
        for root in order.into_iter().rev() {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            let mut component = vec![];
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                arena.tick()?;
                component.push(node);
                for parent in &reverse[node] {
                    if !visited[*parent] {
                        visited[*parent] = true;
                        stack.push(*parent);
                    }
                }
            }
            component.sort_unstable();
            components.push(component);
        }
        Ok(components)
    }
    fn path(
        &self,
        start: usize,
        end: usize,
        members: &HashSet<usize>,
        arena: &mut Arena,
    ) -> Result<Vec<(usize, usize)>, CoreError> {
        if start == end {
            return Ok(vec![]);
        }
        let mut queue = VecDeque::from([start]);
        let mut parents = HashMap::from([(start, None)]);
        while let Some(source) = queue.pop_front() {
            arena.tick()?;
            for (index, edge) in self.edges[source].iter().enumerate() {
                if members.contains(&edge.target) && !parents.contains_key(&edge.target) {
                    parents.insert(edge.target, Some((source, index)));
                    if edge.target == end {
                        let mut path = vec![];
                        let mut node = end;
                        while let Some(Some((source, index))) = parents.get(&node) {
                            path.push((*source, *index));
                            node = *source;
                        }
                        path.reverse();
                        return Ok(path);
                    }
                    queue.push_back(edge.target);
                }
            }
        }
        Err(CoreError::protocol("Broken temporal component"))
    }
    fn accepting_cycle(&self, arena: &mut Arena) -> Result<Option<Cycle>, CoreError> {
        for component in self.components(arena)? {
            let entry = component[0];
            let members: HashSet<_> = component.iter().copied().collect();
            if component.len() == 1 && !self.edges[entry].iter().any(|edge| edge.target == entry) {
                continue;
            }
            let mut selected = vec![];
            let mut accepting = true;
            for obligation in &self.pool.acceptance {
                let mut witness = None;
                'find: for source in &component {
                    for (index, edge) in self.edges[*source].iter().enumerate() {
                        arena.tick()?;
                        if members.contains(&edge.target) && !edge.pending.contains(obligation) {
                            witness = Some((*source, index));
                            break 'find;
                        }
                    }
                }
                if let Some(edge) = witness {
                    selected.push(edge);
                } else {
                    accepting = false;
                    break;
                }
            }
            if !accepting {
                continue;
            }
            if selected.is_empty() {
                let Some(index) = self.edges[entry]
                    .iter()
                    .position(|edge| members.contains(&edge.target))
                else {
                    continue;
                };
                selected.push((entry, index));
            }
            let mut current = entry;
            let mut cycle = vec![];
            for (source, index) in selected {
                cycle.extend(self.path(current, source, &members, arena)?);
                cycle.push((source, index));
                current = self.edges[source][index].target;
            }
            cycle.extend(self.path(current, entry, &members, arena)?);
            return Ok(Some(Cycle {
                entry,
                edges: cycle,
            }));
        }
        Ok(None)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn check(
    program: &SafetyProgram,
    bounds: SafetyBounds,
    values: &[Scalar],
    width: usize,
    graph: &[Vec<GraphEdge>],
    roots: &[usize],
    arena: &mut Arena,
) -> Result<Option<SafetyViolation>, CoreError> {
    for property in &program.properties {
        let mut pool = Pool {
            definitions: program.definitions.as_deref().unwrap_or_default(),
            formulas: vec![],
            acceptance: vec![],
            frame_sequence: 0,
            entries: vec![],
            active_entries: vec![],
            lower_entered: HashSet::new(),
        };
        let mut root = pool.lower(
            &property.formula,
            true,
            EvalContext::new(&[], &[]),
            &[],
            arena,
        )?;
        for fairness in &program.fairness {
            let fairness = pool.lower(fairness, false, EvalContext::new(&[], &[]), &[], arena)?;
            root = pool.add(Formula::And(fairness, root));
        }
        if matches!(pool.formulas[root], Formula::False) {
            return Err(CoreError::protocol(
                "Temporal formula is a tautology (its negation is unsatisfiable)",
            ));
        }
        let mut product = Product {
            pool,
            nodes: vec![],
            edges: vec![],
            lookup: HashMap::new(),
            maximum: bounds.max_temporal_nodes.unwrap_or(bounds.max_states) as usize,
        };
        for state in roots {
            product.node(*state, vec![root], None)?;
        }
        let mut index = 0;
        while index < product.nodes.len() {
            arena.tick()?;
            let source = product.nodes[index].state;
            let required = product.nodes[index].required.clone();
            for edge in &graph[source] {
                let current = &values[source * width..(source + 1) * width];
                let next: Vec<_> = values[edge.target * width..(edge.target + 1) * width]
                    .iter()
                    .copied()
                    .map(Some)
                    .collect();
                for alternative in expand(
                    &product.pool,
                    &required,
                    EvalContext::new(current, &next),
                    arena,
                )? {
                    arena.tick()?;
                    let ordinal = product.edges[index].len();
                    let target =
                        product.node(edge.target, alternative.next, Some((index, ordinal)))?;
                    product.edges[index].push(ProductEdge {
                        target,
                        action: edge.action,
                        pending: alternative.pending,
                    });
                }
            }
            index += 1;
        }
        if let Some(cycle) = product.accepting_cycle(arena)? {
            let entry = cycle.entry;
            let cycle = cycle.edges;
            let mut prefix = vec![];
            let mut current = entry;
            while let Some((source, index)) = product.nodes[current].parent {
                prefix.push((source, index));
                current = source;
            }
            prefix.reverse();
            let mut trace = vec![SafetyTraceStep {
                action: None,
                values: values[product.nodes[current].state * width
                    ..(product.nodes[current].state + 1) * width]
                    .iter()
                    .map(|value| arena.portable(*value))
                    .collect::<Result<_, _>>()?,
            }];
            let label = |action: Option<usize>| {
                action
                    .map(|index| program.next[index].name.clone())
                    .unwrap_or_else(|| "<stuttering>".into())
            };
            for (source, index) in &prefix {
                let edge = &product.edges[*source][*index];
                let state = product.nodes[edge.target].state;
                trace.push(SafetyTraceStep {
                    action: Some(label(edge.action)),
                    values: values[state * width..(state + 1) * width]
                        .iter()
                        .map(|value| arena.portable(*value))
                        .collect::<Result<_, _>>()?,
                });
            }
            let loop_start = trace.len() - 1;
            for (source, index) in cycle.iter().take(cycle.len() - 1) {
                let edge = &product.edges[*source][*index];
                let state = product.nodes[edge.target].state;
                trace.push(SafetyTraceStep {
                    action: Some(label(edge.action)),
                    values: values[state * width..(state + 1) * width]
                        .iter()
                        .map(|value| arena.portable(*value))
                        .collect::<Result<_, _>>()?,
                });
            }
            let (source, index) = cycle[cycle.len() - 1];
            return Ok(Some(SafetyViolation {
                name: property.name.clone(),
                trace,
                loop_start: Some(loop_start),
                loop_action: Some(label(product.edges[source][index].action)),
            }));
        }
    }
    Ok(None)
}
