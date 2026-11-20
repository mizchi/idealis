#[path = "safety_coverage.rs"]
mod coverage;
use crate::safety_expressions::{EvalContext, validate_branches, visit_branch};
use coverage::{NoCoverage, Recording, WithCoverage};
#[cfg(not(target_arch = "wasm32"))]
#[path = "safety_parallel.rs"]
mod parallel;
use crate::expansions::Successor;
use crate::ir::{
    Branch, SafetyBounds, SafetyCoverageReport, SafetyOptions, SafetyProgram, SafetyResult,
    SafetyTraceStep, SafetyViolation, Scalar,
};
use crate::safety_partial_order::PartialOrder;
use crate::safety_prepared::PreparedProgram;
use crate::safety_symmetry::{Symmetry, visit_representatives};
use crate::safety_values::Arena;
use crate::temporal::GraphEdge;
use crate::{
    CoreError, ExpansionOptions, ExpansionStats, IncrementalSafetyResult, SafetyExpansionCache,
    Value,
};
use std::collections::{HashMap, HashSet};

struct Node {
    parent: Option<usize>,
    action: Option<usize>,
    depth: u32,
}

#[derive(Clone, Copy)]
struct BranchCursor<'a> {
    definition: &'a Branch,
    index: usize,
}

struct StateStore {
    width: usize,
    values: Vec<Scalar>,
    heads: HashMap<u64, usize>,
    links: Vec<Option<usize>>,
}

impl StateStore {
    fn new(width: usize) -> Self {
        Self {
            width,
            values: vec![],
            heads: HashMap::new(),
            links: vec![],
        }
    }
    fn state(&self, index: usize) -> &[Scalar] {
        &self.values[index * self.width..(index + 1) * self.width]
    }
    fn find(&self, hash: u64, values: &[Option<Scalar>]) -> Option<usize> {
        if values.len() != self.width {
            return None;
        }
        let mut candidate = self.heads.get(&hash).copied();
        while let Some(index) = candidate {
            if self
                .state(index)
                .iter()
                .zip(values)
                .all(|(stored, actual)| Some(*stored) == *actual)
            {
                return Some(index);
            }
            candidate = self.links[index];
        }
        None
    }
    fn push(&mut self, hash: u64, values: &[Option<Scalar>]) -> Result<usize, CoreError> {
        if values.len() != self.width || values.iter().any(Option::is_none) {
            return Err(CoreError::protocol("Incomplete safety state"));
        }
        let index = self.links.len();
        self.values.extend(values.iter().flatten().copied());
        self.links.push(self.heads.insert(hash, index));
        Ok(index)
    }
}

fn get_hash(values: &[Option<Scalar>]) -> Option<u64> {
    values
        .iter()
        .try_fold(0xcbf2_9ce4_8422_2325_u64, |hash, value| {
            let bits = match (*value)? {
                Scalar::Int(value) => u64::from(value as u32) << 1,
                Scalar::Bool(value) => (u64::from(value) << 1) | 1,
                Scalar::Ref(value) => u64::from(value) ^ 0x9e37_79b9_7f4a_7c15,
            };
            Some(hash.wrapping_mul(0x0000_0100_0000_01b3) ^ bits)
        })
}

struct Search<'a, 'cache, R: Recording = NoCoverage> {
    cache: Option<&'cache mut SafetyExpansionCache>,
    program: &'a SafetyProgram,
    bounds: SafetyBounds,
    nodes: Vec<Node>,
    store: StateStore,
    result: SafetyResult,
    graph: Option<Vec<Vec<GraphEdge>>>,
    roots: Vec<usize>,
    coverage: R,
    symmetry: Option<Symmetry>,
    options: SafetyOptions,
    partial_order: Option<PartialOrder>,
    validated: bool,
}

/// Concrete reachable states in program variable order. Successors are state indices
/// after state/action constraints. Only actual Next edges are included.
#[derive(Default, Debug)]
pub struct SafetyGraph {
    pub states: Vec<Vec<Value>>,
    pub initial: Vec<usize>,
    pub successors: Vec<Vec<usize>>,
}

pub struct PreparedSafetyProgram {
    program: Box<SafetyProgram>,
    prepared: PreparedProgram,
    metadata: Option<std::sync::Arc<crate::safety_definitions::Metadata>>,
}

impl PreparedSafetyProgram {
    pub fn check(&self, bounds: SafetyBounds) -> Result<SafetyResult, CoreError> {
        self.check_with_options(bounds, SafetyOptions::default())
    }

    pub fn check_with_options(
        &self,
        bounds: SafetyBounds,
        options: SafetyOptions,
    ) -> Result<SafetyResult, CoreError> {
        self.program.validate_options(options)?;
        self.program
            .check_measured::<NoCoverage>(bounds, options, Some(self), None, None)
    }

    pub fn check_with_coverage(
        &self,
        bounds: SafetyBounds,
    ) -> Result<SafetyCoverageReport, CoreError> {
        self.program.check_measured::<WithCoverage>(
            bounds,
            SafetyOptions::default(),
            Some(self),
            None,
            None,
        )
    }
}

impl SafetyProgram {
    pub fn prepare(self) -> Result<PreparedSafetyProgram, CoreError> {
        self.validate_header()?;
        let program = Box::new(self);
        let metadata = if program.version == 2 {
            Some(std::sync::Arc::new(
                crate::safety_definitions::validate_program(&program)?,
            ))
        } else {
            if program.definitions.is_some() {
                return Err(CoreError::protocol("Definitions require safety IR v2"));
            }
            validate_branches(&program.initial, program.variables.len(), 0, 0)?;
            validate_branches(&program.next, program.variables.len(), 1, 0)?;
            for (predicates, phase) in [
                (&program.invariants, 2),
                (&program.initial_properties, 2),
                (&program.action_properties, 1),
                (&program.action_constraints, 1),
                (&program.constraints, 2),
                (&program.assumptions, 3),
            ] {
                for predicate in predicates {
                    predicate
                        .expression
                        .validate(program.variables.len(), phase, 0)?;
                }
            }
            for formula in program
                .properties
                .iter()
                .map(|property| &property.formula)
                .chain(&program.fairness)
            {
                formula.validate(program.variables.len(), 0)?;
            }
            None
        };
        let prepared = PreparedProgram::new(&program);
        Ok(PreparedSafetyProgram {
            program,
            prepared,
            metadata,
        })
    }

    pub fn check(&self, bounds: SafetyBounds) -> Result<SafetyResult, CoreError> {
        self.check_with_workers(bounds, 1, None)
    }
    pub fn check_with_options(
        &self,
        bounds: SafetyBounds,
        options: SafetyOptions,
    ) -> Result<SafetyResult, CoreError> {
        self.validate_options(options)?;
        self.check_measured::<NoCoverage>(bounds, options, None, None, None)
    }

    fn validate_options(&self, options: SafetyOptions) -> Result<(), CoreError> {
        if options.partial_order && self.symmetry.is_some() {
            return Err(CoreError::protocol(
                "Unsupported reduction: partial order with symmetry",
            ));
        }
        if !(1..=256).contains(&options.workers)
            || (options.symmetry_actions && self.symmetry.is_none())
        {
            return Err(CoreError::protocol(
                "Invalid safety options: workers must be 1 to 256 and symmetry actions require symmetry",
            ));
        }
        Ok(())
    }
    pub fn check_with_coverage(
        &self,
        bounds: SafetyBounds,
    ) -> Result<SafetyCoverageReport, CoreError> {
        self.check_measured::<WithCoverage>(bounds, SafetyOptions::default(), None, None, None)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn check_parallel(
        &self,
        bounds: SafetyBounds,
        workers: usize,
    ) -> Result<SafetyResult, CoreError> {
        if workers == 0 || workers > 256 {
            return Err(CoreError::protocol(
                "Safety workers must be between 1 and 256",
            ));
        }
        self.check_with_workers(bounds, workers, None)
    }
    pub fn check_incremental(
        &self,
        bounds: SafetyBounds,
        cache: &mut SafetyExpansionCache,
        options: ExpansionOptions<'_>,
    ) -> Result<IncrementalSafetyResult, CoreError> {
        if !options.enabled {
            return Ok(IncrementalSafetyResult {
                result: self.check(bounds)?,
                stats: ExpansionStats::default(),
            });
        }
        cache.prepare(options.build_digest, options.max_entries)?;
        let result = self.check_with_workers(bounds, 1, Some(cache))?;
        Ok(IncrementalSafetyResult {
            result,
            stats: cache.stats,
        })
    }
    fn check_with_workers(
        &self,
        bounds: SafetyBounds,
        workers: usize,
        cache: Option<&mut SafetyExpansionCache>,
    ) -> Result<SafetyResult, CoreError> {
        self.check_measured::<NoCoverage>(
            bounds,
            SafetyOptions {
                workers,
                ..Default::default()
            },
            None,
            None,
            cache,
        )
    }
    /// Export only after a complete, unreduced check of this program.
    pub fn reachable_graph(&self, bounds: SafetyBounds) -> Result<SafetyGraph, CoreError> {
        if self.symmetry.is_some() {
            return Err(CoreError::protocol(
                "Concrete graph export requires unreduced states",
            ));
        }
        let mut graph = SafetyGraph::default();
        let result = self.check_measured::<NoCoverage>(
            bounds,
            SafetyOptions::default(),
            None,
            Some(&mut graph),
            None,
        )?;
        if result.status != "complete" {
            return Err(CoreError::protocol(&format!(
                "Cannot export {} graph: {}",
                result.status,
                result
                    .reason
                    .as_deref()
                    .or(result.violation.as_ref().map(|v| v.name.as_str()))
                    .unwrap_or("no complete graph")
            )));
        }
        Ok(graph)
    }

    fn check_measured<R: Recording>(
        &self,
        bounds: SafetyBounds,
        options: SafetyOptions,
        preparation: Option<&PreparedSafetyProgram>,
        graph_output: Option<&mut SafetyGraph>,
        cache: Option<&mut SafetyExpansionCache>,
    ) -> Result<R::Output, CoreError> {
        if preparation.is_none() {
            self.validate_header()?;
        }
        if bounds.max_states == 0 || bounds.max_work == 0 {
            return Err(CoreError::protocol("Invalid safety program or bounds"));
        }
        let local;
        let prepared = match preparation {
            Some(preparation) => &preparation.prepared,
            None => {
                local = PreparedProgram::new(self);
                &local
            }
        };
        let mut arena = Arena::with_prepared(bounds.max_work, prepared);
        arena.metadata = preparation.and_then(|preparation| preparation.metadata.clone());
        arena.definitions = self.definitions.as_deref().unwrap_or_default();
        let mut search = Search::<R> {
            cache,
            program: self,
            bounds,
            nodes: vec![],
            graph: (graph_output.is_some() || !self.properties.is_empty()).then(Vec::new),
            roots: vec![],
            store: StateStore::new(self.variables.len()),
            coverage: R::new(self),
            symmetry: None,
            options,
            partial_order: None,
            validated: preparation.is_some(),
            result: SafetyResult {
                status: "complete".into(),
                reason: None,
                states: 0,
                generated: 0,
                depth: 0,
                work: 0,
                violation: None,
            },
        };
        let outcome = search.run(&mut arena, options.workers);
        if let Err(error) = outcome {
            if error.code != "budget"
                && error.code != "temporal-budget"
                && error.code != "evaluation-budget"
            {
                return Err(error);
            }
            search.result.status = "bounded".into();
            search.result.reason = Some(
                if error.code == "temporal-budget" {
                    "temporal"
                } else if error.code == "evaluation-budget" {
                    "evaluation"
                } else {
                    "work"
                }
                .into(),
            );
        }
        if search.result.status == "complete"
            && let Some(output) = graph_output
        {
            output.states = (0..search.nodes.len())
                .map(|i| search.portable(i, &arena))
                .collect::<Result<_, _>>()?;
            output.initial = search.roots;
            output.successors = search
                .graph
                .expect("requested graph")
                .into_iter()
                .map(|edges| {
                    edges
                        .into_iter()
                        .filter(|e| e.action.is_some())
                        .map(|e| e.target)
                        .collect()
                })
                .collect();
        }
        search.result.work = arena.work;
        Ok(search.coverage.finish(search.result))
    }

    fn validate_header(&self) -> Result<(), CoreError> {
        if self.format != "modelt.safety-program"
            || !matches!(self.version, 1 | 2)
            || self.name.is_empty()
            || self.variables.is_empty()
            || self.variables.len() > 256
            || self.variables.iter().any(String::is_empty)
            || self.variables.iter().collect::<HashSet<_>>().len() != self.variables.len()
        {
            return Err(CoreError::protocol("Invalid safety program or bounds"));
        }
        Ok(())
    }
}

impl<'program, R: Recording> Search<'program, '_, R> {
    fn run(&mut self, arena: &mut Arena<'program>, workers: usize) -> Result<(), CoreError> {
        let program = self.program;
        let metadata = if self.validated {
            arena.metadata.clone()
        } else if program.version == 2 {
            Some(std::sync::Arc::new(
                crate::safety_definitions::validate_program(program)?,
            ))
        } else {
            if program.definitions.is_some() {
                return Err(CoreError::protocol("Definitions require safety IR v2"));
            }
            validate_branches(&program.initial, program.variables.len(), 0, 0)?;
            validate_branches(&program.next, program.variables.len(), 1, 0)?;
            None
        };
        arena.metadata = metadata.clone();
        if let Some(cache) = self.cache.as_mut() {
            cache.bind_validated(program)?;
        }
        for definition in arena.definitions {
            match &definition.body {
                crate::ir::DefinitionBody::Temporal(body) => {
                    crate::safety_expressions::register_formula_memberships(
                        body,
                        &mut |expression| arena.register_memberships(expression),
                    );
                }
                crate::ir::DefinitionBody::Expression(body) => arena.register_memberships(body),
                crate::ir::DefinitionBody::Action(branches) => {
                    crate::safety_expressions::register_branch_memberships(
                        branches,
                        &mut |expression| arena.register_memberships(expression),
                    );
                }
            }
        }
        if self.options.partial_order {
            self.partial_order = Some(PartialOrder::prepare(program));
        }
        if program.symmetry.is_some() {
            if R::ENABLED {
                return Err(CoreError::protocol("Unsupported symmetry: coverage"));
            }
            self.symmetry = Some(Symmetry::prepare(program, arena)?);
        }
        crate::safety_expressions::register_branch_memberships(
            &program.initial,
            &mut |expression| arena.register_memberships(expression),
        );
        crate::safety_expressions::register_branch_memberships(&program.next, &mut |expression| {
            arena.register_memberships(expression)
        });
        for predicate in program.invariants.iter().chain(&program.initial_properties) {
            if !self.validated && metadata.is_none() {
                predicate
                    .expression
                    .validate(program.variables.len(), 2, 0)?;
            }
            arena.register_memberships(&predicate.expression);
            if metadata.as_ref().map_or_else(
                || predicate.expression.is_constant(),
                |metadata| metadata.is_constant(&predicate.expression),
            ) && !predicate
                .expression
                .eval(EvalContext::new(&[], &[]), arena)?
                .boolean()?
            {
                return Err(CoreError::protocol(
                    "Constant invariant or property is false",
                ));
            }
        }
        for predicate in program
            .action_properties
            .iter()
            .chain(&program.action_constraints)
        {
            if !self.validated && metadata.is_none() {
                predicate
                    .expression
                    .validate(program.variables.len(), 1, 0)?;
            }
            arena.register_memberships(&predicate.expression);
        }
        for predicate in &program.constraints {
            if !self.validated && metadata.is_none() {
                predicate
                    .expression
                    .validate(program.variables.len(), 2, 0)?;
            }
            arena.register_memberships(&predicate.expression);
        }
        for property in &program.properties {
            if !self.validated && metadata.is_none() {
                property.formula.validate(program.variables.len(), 0)?;
            }
        }
        for fairness in &program.fairness {
            if !self.validated && metadata.is_none() {
                fairness.validate(program.variables.len(), 0)?;
            }
        }
        for predicate in &program.assumptions {
            if !self.validated && metadata.is_none() {
                predicate
                    .expression
                    .validate(program.variables.len(), 3, 0)?;
            }
            arena.register_memberships(&predicate.expression);
            if !predicate
                .expression
                .eval(EvalContext::new(&[], &[]), arena)?
                .boolean()?
            {
                self.result.status = "assumption-violation".into();
                self.result.violation = Some(SafetyViolation {
                    name: predicate.name.clone(),
                    trace: vec![],
                    loop_start: None,
                    loop_action: None,
                });
                return Ok(());
            }
        }
        let mut next = vec![None; program.variables.len()];
        for (index, branch) in program.initial.iter().enumerate() {
            self.walk(
                arena,
                BranchCursor {
                    definition: branch,
                    index,
                },
                0,
                EvalContext::new(&[], &[]),
                &mut next,
                None,
                &mut false,
            )?;
            if self.stopped() {
                return Ok(());
            }
        }
        if self.nodes.is_empty() {
            self.result.status = "empty".into();
            return Ok(());
        }
        let mut index = 0;
        let mut current = vec![Scalar::Int(0); program.variables.len()];
        #[cfg(not(target_arch = "wasm32"))]
        let mut pool = None;
        #[cfg(not(target_arch = "wasm32"))]
        let parallel = !R::ENABLED
            && !self.options.partial_order
            && workers > 1
            && parallel::is_expensive(program);
        #[cfg(target_arch = "wasm32")]
        let _ = workers;
        while index < self.nodes.len() {
            let end = self.nodes.len();
            #[cfg(not(target_arch = "wasm32"))]
            if parallel
                && end - index >= if self.symmetry.is_some() { 64 } else { 16 }
                && program.action_properties.is_empty()
                && program.properties.is_empty()
                && program.fairness.is_empty()
            {
                if pool.is_none() {
                    pool = Some(parallel::Workers::new(workers, self.store.width)?);
                }
                self.parallel_frontier(index..end, pool.as_mut().unwrap(), arena)?;
                if self.stopped() {
                    return Ok(());
                }
                index = end;
                continue;
            }
            while index < end {
                self.expand(index, arena, &mut current, &mut next)?;
                if self.stopped() {
                    return Ok(());
                }
                index += 1;
            }
        }
        if self.result.status == "complete"
            && !program.properties.is_empty()
            && let Some(graph) = &mut self.graph
        {
            for (index, edges) in graph.iter_mut().enumerate() {
                edges.push(GraphEdge {
                    target: index,
                    action: None,
                });
            }
            if let Some(violation) = crate::temporal::check(
                program,
                self.bounds,
                &self.store.values,
                self.store.width,
                graph,
                &self.roots,
                arena,
            )? {
                self.result.status = "liveness-violation".into();
                self.result.violation = Some(violation);
            }
        }
        Ok(())
    }
    fn stopped(&self) -> bool {
        self.result.violation.is_some()
            || self.result.reason.as_deref() == Some("work")
            || self.result.reason.as_deref() == Some("states")
    }
    fn expand(
        &mut self,
        index: usize,
        arena: &mut Arena,
        current: &mut [Scalar],
        next: &mut [Option<Scalar>],
    ) -> Result<(), CoreError> {
        if let Some(cache) = self.cache.take() {
            let result = self.expand_cached(index, arena, cache, current, next);
            self.cache = Some(cache);
            return result;
        }
        let program = self.program;
        current.copy_from_slice(self.store.state(index));
        let mut enabled = false;
        let selected = self
            .partial_order
            .as_ref()
            .and_then(|partial_order| partial_order.selected(current));
        for (branch_index, branch) in program.next.iter().enumerate() {
            if selected.is_some_and(|selected| selected != branch_index) {
                continue;
            }
            self.walk(
                arena,
                BranchCursor {
                    definition: branch,
                    index: branch_index,
                },
                0,
                EvalContext::new(current, &[]),
                next,
                Some(index),
                &mut enabled,
            )?;
            if self.stopped() {
                return Ok(());
            }
        }
        if !enabled && program.check_deadlock {
            self.deadlock(index, arena)?;
        }
        Ok(())
    }

    fn expand_cached(
        &mut self,
        index: usize,
        arena: &mut Arena,
        cache: &mut SafetyExpansionCache,
        current_buffer: &mut [Scalar],
        next_buffer: &mut [Option<Scalar>],
    ) -> Result<(), CoreError> {
        arena.tick()?;
        let mut remaining = 64 * 1024usize;
        let mut state = Vec::with_capacity(self.store.width);
        for value in self.store.state(index) {
            let Some(value) = arena.portable_bounded(*value, &mut remaining)? else {
                return self.expand(index, arena, current_buffer, next_buffer);
            };
            state.push(value);
        }
        let key = SafetyExpansionCache::key(&state)?;
        if let Some(successors) = cache.lookup(&key, self.store.width, self.program.next.len())? {
            for successor in successors {
                let values = successor
                    .values
                    .iter()
                    .map(|value| arena.data(value).map(Some))
                    .collect::<Result<Vec<_>, _>>()?;
                self.accept(&values, Some(index), successor.action as usize, arena)?;
                if self.stopped() {
                    return Ok(());
                }
            }
            if successors.is_empty() && self.program.check_deadlock {
                self.deadlock(index, arena)?;
            }
            return Ok(());
        }
        if !cache.can_insert() {
            return self.expand(index, arena, current_buffer, next_buffer);
        }
        let current = self.store.state(index).to_vec();
        let mut next = vec![None; self.store.width];
        let mut successors = Some(vec![]);
        let mut enabled = false;
        for (action, branch) in self.program.next.iter().enumerate() {
            visit_branch(
                branch,
                0,
                EvalContext::new(&current, &[]),
                &mut next,
                arena,
                &mut |values, arena| {
                    enabled = true;
                    self.accept(values, Some(index), action, arena)?;
                    if let Some(stored) = &mut successors {
                        let mut portable = vec![];
                        let mut fits = remaining >= 128;
                        remaining = remaining.saturating_sub(128);
                        for value in values.iter() {
                            if !fits {
                                break;
                            }
                            match arena.portable_bounded(
                                value.ok_or_else(|| {
                                    CoreError::protocol("Incomplete cached state")
                                })?,
                                &mut remaining,
                            )? {
                                Some(value) => portable.push(value),
                                None => fits = false,
                            }
                        }
                        if fits {
                            stored.push(Successor {
                                action: u32::try_from(action).map_err(|_| {
                                    CoreError::protocol("Invalid cached action index")
                                })?,
                                values: portable,
                            });
                        } else {
                            successors = None;
                        }
                    }
                    Ok(self.stopped())
                },
            )?;
            if self.stopped() {
                return Ok(());
            }
        }
        let deadlock = !enabled && self.program.check_deadlock;
        if let Some(successors) = successors {
            cache.insert(key, successors)?;
        }
        if deadlock {
            self.deadlock(index, arena)?;
        }
        Ok(())
    }

    fn deadlock(&mut self, index: usize, arena: &Arena) -> Result<(), CoreError> {
        self.result.status = "deadlock".into();
        self.result.reason = None;
        self.result.violation = Some(SafetyViolation {
            name: "deadlock".into(),
            trace: self.trace(index, arena)?,
            loop_start: None,
            loop_action: None,
        });
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        arena: &mut Arena,
        branch: BranchCursor<'_>,
        position: usize,
        context: EvalContext<'_>,
        next: &mut [Option<Scalar>],
        parent: Option<usize>,
        enabled: &mut bool,
    ) -> Result<(), CoreError> {
        let before = if R::ENABLED {
            Some((arena.work, self.result.generated, self.result.states))
        } else {
            None
        };
        let members = if self.options.symmetry_actions && parent.is_some() {
            self.symmetry
                .as_ref()
                .unwrap()
                .action_members(branch.definition, context, arena)?
        } else {
            None
        };
        let outcome = if let Some(members) = members {
            visit_representatives(
                branch.definition,
                context,
                next,
                arena,
                &members,
                &mut |values, arena| {
                    *enabled = true;
                    self.accept(values, parent, branch.index, arena)?;
                    Ok(self.stopped())
                },
            )
        } else {
            visit_branch(
                branch.definition,
                position,
                context,
                next,
                arena,
                &mut |values, arena| {
                    *enabled = true;
                    self.accept(values, parent, branch.index, arena)?;
                    Ok(self.stopped())
                },
            )
        };
        if let Some((work, generated, states)) = before {
            self.coverage.record(
                parent.is_none(),
                branch.index,
                arena.work - work,
                self.result.generated - generated,
                self.result.states - states,
            );
        }
        outcome?;
        Ok(())
    }
    fn accept(
        &mut self,
        values: &[Option<Scalar>],
        parent: Option<usize>,
        action: usize,
        arena: &mut Arena,
    ) -> Result<(), CoreError> {
        let hash = get_hash(values).ok_or_else(|| {
            let branch = if parent.is_some() {
                &self.program.next[action]
            } else {
                &self.program.initial[action]
            };
            let missing: Vec<_> = self
                .program
                .variables
                .iter()
                .zip(values)
                .filter(|(_, value)| value.is_none())
                .map(|(name, _)| name.as_str())
                .collect();
            CoreError::protocol(&format!(
                "Incomplete safety state in action {}; unassigned: {}",
                branch.name,
                missing.join(", ")
            ))
        })?;
        self.result.generated += 1;
        if !self.program.constraints.is_empty()
            || (parent.is_some() && !self.program.action_constraints.is_empty())
        {
            let current: Vec<_> = values.iter().flatten().copied().collect();
            let mut in_model = true;
            for predicate in &self.program.constraints {
                if !predicate
                    .expression
                    .eval(EvalContext::new(&current, &[]), arena)?
                    .boolean()?
                {
                    in_model = false;
                    break;
                }
            }
            if in_model && let Some(parent) = parent {
                for predicate in &self.program.action_constraints {
                    if !predicate
                        .expression
                        .eval(EvalContext::new(self.store.state(parent), values), arena)?
                        .boolean()?
                    {
                        in_model = false;
                        break;
                    }
                }
            }
            if !in_model {
                if parent.is_some_and(|index| self.nodes[index].depth >= self.bounds.max_depth) {
                    self.result.status = "bounded".into();
                    self.result.reason = Some("depth".into());
                    return Ok(());
                }
                if let Some((status, name)) =
                    check_properties(self.program, &current, parent, &self.store, true, arena)?
                {
                    let mut trace = parent
                        .map(|index| self.trace(index, arena))
                        .transpose()?
                        .unwrap_or_default();
                    trace.push(SafetyTraceStep {
                        action: parent.map(|_| self.program.next[action].name.clone()),
                        values: current
                            .iter()
                            .map(|value| arena.portable(*value))
                            .collect::<Result<_, _>>()?,
                    });
                    self.result.status = status.into();
                    self.result.reason = None;
                    self.result.violation = Some(SafetyViolation {
                        name: name.into(),
                        trace,
                        loop_start: None,
                        loop_action: None,
                    });
                }
                return Ok(());
            }
        }
        let key = self
            .symmetry
            .as_ref()
            .map(|symmetry| symmetry.key(values, arena))
            .transpose()?;
        let key_hash = key.as_ref().map(|key| Symmetry::hash(key));
        let existing =
            if let (Some(symmetry), Some(key), Some(hash)) = (&self.symmetry, &key, key_hash) {
                symmetry.find(hash, key)
            } else {
                self.store.find(hash, values)
            };
        let index = if let Some(index) = existing {
            index
        } else {
            let depth = parent.map_or(0, |index| self.nodes[index].depth + 1);
            if depth > self.bounds.max_depth {
                self.result.status = "bounded".into();
                self.result.reason = Some("depth".into());
                return Ok(());
            }
            if self.nodes.len() == self.bounds.max_states as usize {
                self.result.status = "bounded".into();
                self.result.reason = Some("states".into());
                return Ok(());
            }
            let index = self.store.push(hash, values)?;
            if let (Some(symmetry), Some(key), Some(hash)) = (&mut self.symmetry, key, key_hash) {
                symmetry.insert(hash, key, index);
            }
            self.nodes.push(Node {
                parent,
                action: parent.map(|_| action),
                depth,
            });
            if let Some(graph) = &mut self.graph {
                graph.push(vec![]);
            }
            self.result.states = self.nodes.len();
            self.result.depth = self.result.depth.max(depth);
            index
        };
        if let Some(parent) = parent {
            if let Some(graph) = &mut self.graph {
                graph[parent].push(GraphEdge {
                    target: index,
                    action: Some(action),
                });
            }
        } else if !self.roots.contains(&index) {
            self.roots.push(index);
        }
        let checks_candidate = (existing.is_none() && !self.program.invariants.is_empty())
            || if parent.is_some() {
                !self.program.action_properties.is_empty()
            } else {
                !self.program.initial_properties.is_empty()
            };
        if checks_candidate
            && let Some((status, name)) = check_properties(
                self.program,
                self.store.state(index),
                parent,
                &self.store,
                existing.is_none(),
                arena,
            )?
        {
            let trace = if let Some(parent) = parent {
                let mut trace = self.trace(parent, arena)?;
                trace.push(SafetyTraceStep {
                    action: Some(self.program.next[action].name.clone()),
                    values: self.portable(index, arena)?,
                });
                trace
            } else {
                self.trace(index, arena)?
            };
            self.result.status = status.into();
            self.result.reason = None;
            self.result.violation = Some(SafetyViolation {
                name: name.into(),
                trace,
                loop_start: None,
                loop_action: None,
            });
        }
        Ok(())
    }
    fn portable(&self, index: usize, arena: &Arena) -> Result<Vec<Value>, CoreError> {
        self.store
            .state(index)
            .iter()
            .map(|value| arena.portable(*value))
            .collect()
    }
    fn trace(&self, mut index: usize, arena: &Arena) -> Result<Vec<SafetyTraceStep>, CoreError> {
        let mut trace = vec![];
        loop {
            let node = &self.nodes[index];
            trace.push(SafetyTraceStep {
                action: node
                    .action
                    .map(|index| self.program.next[index].name.clone()),
                values: self.portable(index, arena)?,
            });
            match node.parent {
                Some(parent) => index = parent,
                None => break,
            }
        }
        trace.reverse();
        Ok(trace)
    }
}

fn check_properties<'a>(
    program: &'a SafetyProgram,
    values: &[Scalar],
    parent: Option<usize>,
    store: &StateStore,
    is_new: bool,
    arena: &mut Arena,
) -> Result<Option<(&'static str, &'a str)>, CoreError> {
    if is_new {
        for predicate in &program.invariants {
            if !predicate
                .expression
                .eval(EvalContext::new(values, &[]), arena)?
                .boolean()?
            {
                return Ok(Some(("invariant-violation", &predicate.name)));
            }
        }
    }
    if let Some(parent) = parent {
        if program.action_properties.is_empty() {
            return Ok(None);
        }
        let parent = store.state(parent);
        let next: Vec<_> = values.iter().copied().map(Some).collect();
        for predicate in &program.action_properties {
            if !predicate
                .expression
                .eval(EvalContext::new(parent, &next), arena)?
                .boolean()?
            {
                return Ok(Some(("property-violation", &predicate.name)));
            }
        }
    } else {
        for predicate in &program.initial_properties {
            if !predicate
                .expression
                .eval(EvalContext::new(values, &[]), arena)?
                .boolean()?
            {
                return Ok(Some(("property-violation", &predicate.name)));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{Scalar, StateStore};

    #[test]
    fn colliding_hashes_keep_distinct_typed_states() {
        let mut store = StateStore::new(2);
        let first = [Some(Scalar::Int(0)), Some(Scalar::Int(1))];
        let second = [Some(Scalar::Int(0)), Some(Scalar::Int(2))];
        let third = [Some(Scalar::Bool(false)), Some(Scalar::Int(1))];
        for values in [&first, &second, &third] {
            store.push(0, values).unwrap();
        }
        assert_eq!(store.find(0, &first), Some(0));
        assert_eq!(store.find(0, &second), Some(1));
        assert_eq!(store.find(0, &third), Some(2));
        assert_eq!(
            store.find(0, &[Some(Scalar::Int(0)), Some(Scalar::Int(3))]),
            None
        );
        assert_eq!(store.find(0, &first[..1]), None);
        assert!(store.push(0, &[Some(Scalar::Int(0)), None]).is_err());
        assert_eq!(store.find(0, &third), Some(2));
    }
}
