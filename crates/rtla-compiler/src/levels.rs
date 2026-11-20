use crate::analysis::{Node, analysis_children};
use crate::{Result, unsupported};
use rtla::ast::{self, DefinitionKind, Expression, ExpressionKind as E, Module};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

#[derive(Clone, Default, PartialEq, Eq)]
struct Value {
    base: u8,
    parameters: BTreeSet<usize>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Summary {
    value: Value,
    limits: BTreeMap<usize, u8>,
}

impl Summary {
    fn merge(&mut self, other: Self) {
        self.value.base = self.value.base.max(other.value.base);
        self.value.parameters.extend(other.value.parameters);
        for (parameter, maximum) in other.limits {
            self.limits
                .entry(parameter)
                .and_modify(|old| *old = (*old).min(maximum))
                .or_insert(maximum);
        }
    }

    fn constrain(&mut self, maximum: u8, expression: &Expression) -> Result<()> {
        if self.value.base > maximum {
            return unsupported(format!(
                "level error at line {}, column {}: level {} exceeds {}",
                expression.location.line, expression.location.column, self.value.base, maximum
            ));
        }
        for parameter in &self.value.parameters {
            self.limits
                .entry(*parameter)
                .and_modify(|old| *old = (*old).min(maximum))
                .or_insert(maximum);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Symbol {
    Constant,
    Variable,
    Parameter(usize),
    Definition(usize),
}

struct Scope {
    parent: Option<usize>,
    symbols: BTreeMap<String, Symbol>,
}

struct Definition<'a> {
    syntax: &'a ast::Definition,
    parameters: Vec<usize>,
}

struct Analysis<'a> {
    scopes: Vec<Scope>,
    definitions: Vec<Definition<'a>>,
    expression_scopes: HashMap<*const Expression, usize>,
    nested_definitions: HashMap<*const Expression, Vec<usize>>,
    next_parameter: usize,
    summaries: Vec<Summary>,
}

fn children(expression: &Expression) -> Vec<&Expression> {
    let mut pending = analysis_children(Node::Source(expression));
    let mut result = vec![];
    while let Some(edge) = pending.pop() {
        match edge.node {
            Node::Source(child) => result.push(child),
            Node::SourceDefinition(_) => {}
            node => pending.extend(analysis_children(node)),
        }
    }
    result.reverse();
    result
}

impl<'a> Analysis<'a> {
    fn scope(&mut self, parent: usize) -> usize {
        let id = self.scopes.len();
        self.scopes.push(Scope {
            parent: Some(parent),
            symbols: BTreeMap::new(),
        });
        id
    }

    fn symbol(&self, mut scope: usize, name: &str) -> Option<Symbol> {
        loop {
            if let Some(symbol) = self.scopes[scope].symbols.get(name) {
                return Some(*symbol);
            }
            scope = self.scopes[scope].parent?;
        }
    }

    fn register(
        &mut self,
        definitions: &'a [ast::Definition],
        scope: usize,
        depth: usize,
    ) -> Result<()> {
        let first = self.definitions.len();
        for definition in definitions {
            let id = self.definitions.len();
            self.scopes[scope]
                .symbols
                .insert(definition.name.clone(), Symbol::Definition(id));
            self.definitions.push(Definition {
                syntax: definition,
                parameters: vec![],
            });
        }
        for (offset, definition) in definitions.iter().enumerate() {
            let mut local = self.scope(scope);
            match &definition.head {
                DefinitionKind::Operator { params } => {
                    for parameter in params {
                        let id = self.next_parameter;
                        self.next_parameter += 1;
                        self.scopes[local]
                            .symbols
                            .insert(parameter.name.clone(), Symbol::Parameter(id));
                        self.definitions[first + offset].parameters.push(id);
                    }
                }
                DefinitionKind::Function { bounds } => local = self.bounds(bounds, local, depth)?,
            }
            self.collect(&definition.body, local, depth)?;
        }
        Ok(())
    }

    fn bounds(
        &mut self,
        bounds: &'a [ast::Bound],
        mut scope: usize,
        depth: usize,
    ) -> Result<usize> {
        for bound in bounds {
            self.collect(&bound.set, scope, depth + 1)?;
            scope = self.scope(scope);
            match &bound.pattern {
                ast::Pattern::Name { name } => {
                    self.scopes[scope]
                        .symbols
                        .insert(name.clone(), Symbol::Constant);
                }
                ast::Pattern::Tuple { names } => {
                    for name in names {
                        self.scopes[scope]
                            .symbols
                            .insert(name.clone(), Symbol::Constant);
                    }
                }
            }
        }
        Ok(scope)
    }

    fn collect(&mut self, expression: &'a Expression, scope: usize, depth: usize) -> Result<()> {
        if depth > 64 {
            return unsupported("source level analysis nesting exceeds 64");
        }
        self.expression_scopes.insert(expression, scope);
        match &expression.node {
            E::Let { definitions, body }
            | E::LetModule {
                definitions, body, ..
            } => {
                let local = self.scope(scope);
                let first = self.definitions.len();
                self.register(definitions, local, depth + 1)?;
                self.nested_definitions
                    .insert(expression, (first..first + definitions.len()).collect());
                self.collect(body, local, depth + 1)?;
            }
            E::Forall { bounds, body }
            | E::Exists { bounds, body }
            | E::FunctionConstruction { bounds, body }
            | E::SetMap {
                bounds,
                expression: body,
            } => {
                let local = self.scope(scope);
                let local = self.bounds(bounds, local, depth)?;
                self.collect(body, local, depth + 1)?;
            }
            E::SetFilter { bound, predicate } => {
                let local = self.scope(scope);
                let local = self.bounds(std::slice::from_ref(bound), local, depth)?;
                self.collect(predicate, local, depth + 1)?;
            }
            E::Choose { pattern, set, body } => {
                let local = self.scope(scope);
                if let Some(set) = set {
                    self.collect(set, scope, depth + 1)?;
                }
                let names = match pattern {
                    ast::Pattern::Name { name } => std::slice::from_ref(name),
                    ast::Pattern::Tuple { names } => names,
                };
                for name in names {
                    self.scopes[local]
                        .symbols
                        .insert(name.clone(), Symbol::Constant);
                }
                self.collect(body, local, depth + 1)?;
            }
            _ => {
                for child in children(expression) {
                    self.collect(child, scope, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn referenced(&self, expression: &Expression, name: &str) -> Option<Symbol> {
        self.symbol(
            self.expression_scopes[&(expression as *const Expression)],
            name,
        )
    }

    fn apply(
        &self,
        expression: &Expression,
        symbol: Symbol,
        actuals: Vec<Summary>,
    ) -> Result<Summary> {
        match symbol {
            Symbol::Definition(id) => {
                let definition = &self.definitions[id];
                if definition.parameters.len() != actuals.len() {
                    return unsupported(format!(
                        "source level arity error for {}",
                        definition.syntax.name
                    ));
                }
                let summary = &self.summaries[id];
                let mut result = Summary::default();
                result.value.base = summary.value.base;
                result.value.parameters = summary
                    .value
                    .parameters
                    .difference(&definition.parameters.iter().copied().collect())
                    .copied()
                    .collect();
                for (parameter, maximum) in &summary.limits {
                    if !definition.parameters.contains(parameter) {
                        result.limits.insert(*parameter, *maximum);
                    }
                }
                for (parameter, mut actual) in definition.parameters.iter().zip(actuals) {
                    if let Some(maximum) = summary.limits.get(parameter) {
                        actual.constrain(*maximum, expression)?;
                    }
                    if !summary.value.parameters.contains(parameter) {
                        actual.value = Value::default();
                    }
                    result.merge(actual);
                }
                Ok(result)
            }
            Symbol::Parameter(id) => {
                let mut result = Summary::default();
                result.value.parameters.insert(id);
                for actual in actuals {
                    result.merge(actual);
                }
                Ok(result)
            }
            Symbol::Variable | Symbol::Constant => {
                let mut result = Summary::default();
                result.value.base = u8::from(matches!(symbol, Symbol::Variable));
                for actual in actuals {
                    result.merge(actual);
                }
                Ok(result)
            }
        }
    }

    fn operator(
        &self,
        expression: &Expression,
        children: Vec<(&Expression, u8)>,
        base: u8,
        weighted: bool,
    ) -> Result<Summary> {
        let mut result = Summary::default();
        for (child, maximum) in children {
            let mut summary = self.expression(child)?;
            summary.constrain(maximum, expression)?;
            result.merge(summary);
        }
        if weighted {
            result.value.base = result.value.base.max(base);
        } else {
            result.value = Value {
                base,
                parameters: BTreeSet::new(),
            };
        }
        Ok(result)
    }

    fn expression(&self, expression: &Expression) -> Result<Summary> {
        match &expression.node {
            E::Identifier { name } => {
                if let Some(Symbol::Definition(id)) = self.referenced(expression, name)
                    && !self.definitions[id].parameters.is_empty()
                {
                    let mut summary = self.summaries[id].clone();
                    for parameter in &self.definitions[id].parameters {
                        summary.value.parameters.remove(parameter);
                        summary.limits.remove(parameter);
                    }
                    return Ok(summary);
                }
                self.referenced(expression, name)
                    .map_or(Ok(Summary::default()), |symbol| {
                        self.apply(expression, symbol, vec![])
                    })
            }
            E::Apply { operator, args } => {
                if let Some(symbol) = self.referenced(expression, operator) {
                    return self.apply(
                        expression,
                        symbol,
                        args.iter()
                            .map(|argument| self.expression(argument))
                            .collect::<Result<_>>()?,
                    );
                }
                let (base, maximum, weighted) = match operator.as_str() {
                    "[]" | "<>" | "~>" | "-+->" => (3, 3, false),
                    "ENABLED" => (1, 2, false),
                    "UNCHANGED" => (2, 1, false),
                    "/\\" | "\\/" | "=>" | "<=>" | "~" | "\\lnot" => (0, 3, true),
                    _ => (0, 2, true),
                };
                self.operator(
                    expression,
                    args.iter().map(|argument| (argument, maximum)).collect(),
                    base,
                    weighted,
                )
            }
            E::Prime { expression: child } => self.operator(expression, vec![(child, 1)], 2, false),
            E::Unchanged { expression: child } => {
                self.operator(expression, vec![(child, 1)], 2, false)
            }
            E::Enabled { expression: child } => {
                self.operator(expression, vec![(child, 2)], 1, false)
            }
            E::Always { expression: child } | E::Eventually { expression: child } => {
                self.operator(expression, vec![(child, 3)], 3, false)
            }
            E::BoxAction { action, subscript } | E::AngleAction { action, subscript } => {
                self.operator(expression, vec![(action, 2), (subscript, 1)], 2, false)
            }
            E::WeakFairness { action, subscript } | E::StrongFairness { action, subscript } => {
                self.operator(expression, vec![(subscript, 1), (action, 2)], 3, false)
            }
            E::Let { body, .. } | E::LetModule { body, .. } => {
                let mut result = self.expression(body)?;
                for id in &self.nested_definitions[&(expression as *const Expression)] {
                    for (parameter, maximum) in &self.summaries[*id].limits {
                        if !self.definitions[*id].parameters.contains(parameter) {
                            result
                                .limits
                                .entry(*parameter)
                                .and_modify(|old| *old = (*old).min(*maximum))
                                .or_insert(*maximum);
                        }
                    }
                }
                Ok(result)
            }
            E::Forall { bounds, body }
            | E::Exists { bounds, body }
            | E::FunctionConstruction { bounds, body } => self.operator(
                expression,
                bounds
                    .iter()
                    .map(|bound| (&bound.set, 2))
                    .chain(std::iter::once((body.as_ref(), 3)))
                    .collect(),
                0,
                true,
            ),
            E::And { .. }
            | E::Or { .. }
            | E::If { .. }
            | E::Case { .. }
            | E::Record { .. }
            | E::Label { .. } => self.operator(
                expression,
                children(expression)
                    .into_iter()
                    .map(|child| (child, 3))
                    .collect(),
                0,
                true,
            ),
            E::AssumeProve { .. } => Ok(Summary::default()),
            _ => self.operator(
                expression,
                children(expression)
                    .into_iter()
                    .map(|child| (child, 2))
                    .collect(),
                0,
                true,
            ),
        }
    }
}

pub(crate) fn validate(module: &Module) -> Result<BTreeMap<(usize, String), u8>> {
    let mut analysis = Analysis {
        scopes: vec![Scope {
            parent: None,
            symbols: BTreeMap::new(),
        }],
        definitions: vec![],
        expression_scopes: HashMap::new(),
        nested_definitions: HashMap::new(),
        next_parameter: 0,
        summaries: vec![],
    };
    for declaration in &module.variables {
        analysis.scopes[0]
            .symbols
            .insert(declaration.name.clone(), Symbol::Variable);
    }
    for declaration in &module.constants {
        analysis.scopes[0]
            .symbols
            .insert(declaration.name.clone(), Symbol::Constant);
    }
    analysis.register(&module.definitions, 0, 0)?;
    for assumption in &module.assumptions {
        analysis.collect(&assumption.expression, 0, 0)?;
    }
    analysis
        .summaries
        .resize(analysis.definitions.len(), Summary::default());
    let mut dependencies = vec![BTreeSet::new(); analysis.definitions.len()];
    let mut callers = vec![vec![]; analysis.definitions.len()];
    for (id, definition) in analysis.definitions.iter().enumerate() {
        let mut pending = vec![&definition.syntax.body];
        if let DefinitionKind::Function { bounds } = &definition.syntax.head {
            pending.extend(bounds.iter().map(|bound| &bound.set));
        }
        while let Some(expression) = pending.pop() {
            if let E::Identifier { name } | E::Apply { operator: name, .. } = &expression.node
                && let Some(Symbol::Definition(dependency)) = analysis.referenced(expression, name)
            {
                dependencies[id].insert(dependency);
            }
            if let Some(nested) = analysis
                .nested_definitions
                .get(&(expression as *const Expression))
            {
                dependencies[id].extend(nested);
            }
            pending.extend(children(expression));
        }
        for dependency in &dependencies[id] {
            callers[*dependency].push(id);
        }
    }
    let mut seen = vec![false; dependencies.len()];
    let mut order = vec![];
    for root in 0..seen.len() {
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            if finish {
                order.push(id);
            } else if !seen[id] {
                seen[id] = true;
                pending.push((id, true));
                pending.extend(
                    dependencies[id]
                        .iter()
                        .map(|dependency| (*dependency, false)),
                );
            }
        }
    }
    let mut queue: VecDeque<_> = order.into();
    let mut queued = vec![true; analysis.definitions.len()];
    while let Some(id) = queue.pop_front() {
        queued[id] = false;
        let definition = &analysis.definitions[id];
        let mut summary = analysis.expression(&definition.syntax.body)?;
        if let DefinitionKind::Function { bounds } = &definition.syntax.head {
            for bound in bounds {
                let mut domain = analysis.expression(&bound.set)?;
                domain.constrain(2, &bound.set)?;
                summary.merge(domain);
            }
        }
        if definition.syntax.is_recursive {
            summary.constrain(2, &definition.syntax.body)?;
            for parameter in &definition.parameters {
                if summary
                    .limits
                    .get(parameter)
                    .is_some_and(|maximum| *maximum < 2)
                {
                    return unsupported(
                        "level error: recursive operator parameters cannot be primed",
                    );
                }
                summary
                    .limits
                    .entry(*parameter)
                    .and_modify(|maximum| *maximum = (*maximum).min(2))
                    .or_insert(2);
            }
        }
        if analysis.summaries[id] != summary {
            analysis.summaries[id] = summary;
            for caller in &callers[id] {
                if !queued[*caller] {
                    queue.push_back(*caller);
                    queued[*caller] = true;
                }
            }
        }
    }
    for assumption in &module.assumptions {
        analysis
            .expression(&assumption.expression)?
            .constrain(0, &assumption.expression)?;
    }
    Ok(analysis
        .definitions
        .iter()
        .enumerate()
        .map(|(id, definition)| {
            (
                (
                    definition.syntax.location.start,
                    definition.syntax.name.clone(),
                ),
                analysis.summaries[id].value.base,
            )
        })
        .collect())
}
