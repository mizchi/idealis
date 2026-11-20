use rtla::ast::{
    self, Config, ConfigValue, Definition, DefinitionKind, Expression, ExpressionKind as E, Module,
    Pattern,
};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

mod analysis;
mod elaborate;
mod functions;
mod labels;
mod levels;
mod link;
mod old;
mod rewrite;
mod select;
mod source;
mod symbols;
mod symmetry;
mod typed;
mod typed_expr;
mod typed_lower;
pub use analysis::analysis_roles;
use analysis::{Node, NodeMut};
use rtlc::ir;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    pub message: String,
}
impl std::fmt::Display for CompileError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "Unsupported TLA input: {}", self.message)
    }
}
impl std::error::Error for CompileError {}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVersion {
    #[default]
    V1,
    V2,
}
impl TryFrom<u8> for IrVersion {
    type Error = CompileError;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::V1),
            2 => Ok(Self::V2),
            _ => unsupported("safety IR version must be 1 or 2"),
        }
    }
}
#[allow(non_snake_case)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub irVersion: IrVersion,
}
type Result<T> = std::result::Result<T, CompileError>;
fn unsupported<T>(message: impl Into<String>) -> Result<T> {
    Err(CompileError {
        message: message.into(),
    })
}
fn definition_body(definition: &Definition) -> Expression {
    match &definition.head {
        DefinitionKind::Operator { .. } => definition.body.clone(),
        DefinitionKind::Function { bounds } => Expression {
            node: E::FunctionConstruction {
                bounds: bounds.clone(),
                body: Box::new(definition.body.clone()),
            },
            location: definition.location,
        },
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Initial,
    Next,
    State,
    Constant,
}
#[derive(Clone)]
struct Context {
    origin: usize,
    phase: Phase,
    locals: BTreeMap<String, ir::Expression>,
    calls: Vec<String>,
    call_ids: Vec<usize>,
    depth: usize,
    bound: usize,
    scope: usize,
    order: Vec<String>,
    binding_ids: BTreeMap<String, usize>,
    projection: BTreeMap<usize, ir::Expression>,
    static_targets: BTreeMap<usize, Target>,
    target_actuals: BTreeMap<usize, (Expression, Rc<Context>, HashSet<usize>)>,
    frame: Option<AnonymousFrame>,
    temporal_frames: Vec<AnonymousFrame>,
}
impl Context {
    fn nested(&self) -> Result<Self> {
        if self.depth >= 40 {
            return unsupported("expression or operator nesting exceeds 40");
        }
        Ok(Self {
            depth: self.depth + 1,
            ..self.clone()
        })
    }
}
#[derive(Clone)]
struct AnonymousFrame {
    arguments: Vec<ir::Expression>,
    captures: Vec<ir::Expression>,
}
#[derive(Clone)]
struct Entry {
    id: usize,
    definition: Rc<Definition>,
    locals: BTreeMap<String, ir::Expression>,
    scope: usize,
    order: Vec<String>,
    binding_ids: BTreeMap<String, usize>,
    projection: BTreeMap<usize, ir::Expression>,
    static_targets: BTreeMap<usize, Target>,
    target_actuals: BTreeMap<usize, (Expression, Rc<Context>, HashSet<usize>)>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Slot(usize),
    Tuple(Vec<Target>),
    Binding(usize),
}
#[derive(Clone)]
struct Shared {
    recursive: bool,
    key: (usize, u8, bool, Vec<Target>, ir::BodyKind),
    captures: Vec<usize>,
    definition: Option<ir::Definition>,
    parameter_count: usize,
    pending: Option<(Expression, Context, bool)>,
}
#[derive(Clone, Default, PartialEq, Eq)]
struct LevelSummary {
    base: u8,
    parameters: HashSet<usize>,
    captures: HashSet<usize>,
}
struct Compiler<'a> {
    recursive_ids: HashSet<usize>,
    temporal_ids: HashSet<usize>,
    declaration_ids: BTreeMap<(usize, String), usize>,
    module: &'a Module,
    config: &'a Config,
    slots: BTreeMap<String, usize>,
    constants: BTreeMap<String, ConfigValue>,
    scopes: Vec<BTreeMap<String, Rc<Entry>>>,
    version: IrVersion,
    shared: Vec<Shared>,
    draining: bool,
    target_nesting: usize,
    target_requirements: HashMap<(usize, bool), Vec<usize>>,
    membership_requirements: HashMap<(usize, bool), Vec<usize>>,
    next_binding: usize,
    binding_keys: HashMap<(usize, usize, u8, usize, String), usize>,
    generative_cache: HashMap<(usize, u8), bool>,
    level_cache: std::cell::RefCell<HashMap<usize, LevelSummary>>,
}
fn source_children(value: &Expression) -> Vec<Expression> {
    analysis::analysis_children(analysis::Node::Source(value))
        .into_iter()
        .filter_map(|edge| match edge.node {
            analysis::Node::Source(value) => Some(value.clone()),
            _ => None,
        })
        .collect()
}
fn decode_expression(value: Json) -> Result<ir::Expression> {
    serde_json::from_value(value).map_err(|error| CompileError {
        message: format!("invalid generated IR: {error}"),
    })
}
fn boolean(value: bool) -> Json {
    json!({"op":"literal","value":{"type":"bool","value":value}})
}
fn unmatched_case(location: &ast::Location) -> Json {
    json!({"op":"unmatched-case","location":format!("line {}, column {}", location.line, location.column)})
}
fn integer(value: i64) -> Result<Json> {
    if i32::try_from(value).is_err() {
        return unsupported("integer outside signed 32 bits");
    }
    Ok(json!({"op":"literal","value":{"type":"int","value":value.to_string()}}))
}
fn string(value: &str) -> Json {
    json!({"op":"data","value":{"type":"string","value":value}})
}
fn literal(value: &ConfigValue) -> Result<Json> {
    match value {
        ConfigValue::Number { value } => integer(*value),
        ConfigValue::Boolean { value } => Ok(boolean(*value)),
        ConfigValue::String { value } => Ok(string(value)),
        ConfigValue::Set { elements } => Ok(
            json!({"op":"set","values":elements.iter().map(literal).collect::<Result<Vec<_>>>()?}),
        ),
        ConfigValue::ModelValue { name } => {
            Ok(json!({"op":"data","value":{"type":"model-value","value":name}}))
        }
    }
}
fn level(value: &Json) -> u8 {
    match value {
        Json::Object(fields) => match fields.get("op").and_then(Json::as_str) {
            Some("read") => {
                if value["next"] == true {
                    2
                } else {
                    1
                }
            }
            Some("prime") => 2,
            Some("enabled") => 1,
            _ => fields.values().map(level).max().unwrap_or(0),
        },
        Json::Array(values) => values.iter().map(level).max().unwrap_or(0),
        _ => 0,
    }
}
fn prime(value: Json) -> Result<Json> {
    if level(&value) > 1 {
        return unsupported("prime of an action expression");
    }
    Ok(json!({"op":"prime","argument":value}))
}
fn atom(expression: Json) -> Json {
    json!({"kind":"atom","expression":expression})
}
fn negate(formula: Json) -> Json {
    if formula["kind"] == "atom" {
        atom(json!({"op":"not","argument":formula["expression"]}))
    } else {
        json!({"kind":"not","formula":formula})
    }
}
fn combine(kind: &str, left: Json, right: Json) -> Json {
    if left["kind"] == "atom" && right["kind"] == "atom" {
        atom(json!({"op":kind,"left":left["expression"],"right":right["expression"]}))
    } else {
        json!({"kind":kind,"left":left,"right":right})
    }
}
fn action_dimensions(branches: &[Json]) -> (usize, usize) {
    let mut paths = 0usize;
    let mut longest = 0usize;
    for branch in branches {
        let mut branch_paths = 1usize;
        let mut length = 0usize;
        for step in branch["steps"].as_array().into_iter().flatten() {
            length += 1;
            if step["kind"] == "frame" {
                let inner = step["branches"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let (count, steps) = action_dimensions(inner);
                branch_paths = branch_paths.saturating_mul(count).min(4097);
                length = length.saturating_add(steps).min(257);
            }
        }
        paths = paths.saturating_add(branch_paths).min(4097);
        longest = longest.max(length);
    }
    (paths, longest)
}
fn limited(branches: Vec<Json>) -> Result<Vec<Json>> {
    let (paths, longest) = action_dimensions(&branches);
    if paths > 4096 || longest > 256 || branches.iter().any(|branch| !branch["steps"].is_array()) {
        return unsupported("action expansion exceeds 4096 branches or 256 steps");
    }
    Ok(branches)
}
fn steps(branch: &Json) -> Result<&Vec<Json>> {
    branch["steps"].as_array().ok_or_else(|| CompileError {
        message: "invalid internal action".into(),
    })
}
fn unchanged(value: Json) -> Result<Vec<Json>> {
    if value["op"] == "tuple" {
        let mut result = vec![];
        for item in value["values"].as_array().into_iter().flatten() {
            result.extend(unchanged(item.clone())?);
        }
        return Ok(result);
    }
    if value["op"] != "read" || value["next"] != false {
        return unsupported("generative UNCHANGED requires state variables");
    }
    Ok(vec![
        json!({"kind":"assign","slot":value["slot"],"value":value}),
    ])
}

/// Locate a uniquely named incomplete action in the original source modules.
/// Raw IR can omit source information or reuse an action label.
pub fn incomplete_state_location<'a>(
    error: &rtlc::CoreError,
    modules: impl IntoIterator<Item = &'a Module>,
) -> Option<(&'a str, rtla::ast::Location)> {
    let (action, _) = error
        .message
        .strip_prefix("Incomplete safety state in action ")?
        .split_once("; unassigned: ")?;
    let mut definitions = modules.into_iter().flat_map(|module| {
        module
            .definitions
            .iter()
            .filter(move |d| d.name == action)
            .map(move |d| (module.name.as_str(), d.location))
    });
    let location = definitions.next()?;
    definitions.next().is_none().then_some(location)
}

/// Compile the selected finite TLA+ fragment. Parsing and execution remain separate.
pub fn compile(module: &Module, config: &Config) -> Result<rtlc::SafetyProgram> {
    compile_with_options(module, config, CompileOptions::default())
}

pub fn compile_with_options(
    module: &Module,
    config: &Config,
    options: CompileOptions,
) -> Result<rtlc::SafetyProgram> {
    if link::declares_instances_or_overrides(module, config) || elaborate::needed(module) {
        return compile_modules_with_options(module, config, &[], options);
    }
    symbols::validate(module)?;
    compile_flat(module, config, options, false)
}

/// Check names and, optionally, expression levels in an executable module bundle.
pub fn analyze_modules(root: &Module, modules: &[Module], check_levels: bool) -> Result<()> {
    if std::iter::once(root).chain(modules).any(|module| {
        !module.theorems.is_empty() || !module.directives.is_empty() || !module.modules.is_empty()
    }) {
        return unsupported("standalone analysis of proofs or nested modules");
    }
    let config = rtla::parse_config("").expect("empty configuration");
    let (linked, _, _) = link::resolve(root, &config, modules)?;
    if check_levels {
        levels::validate(&linked)?;
    }
    Ok(())
}

/// Compile an explicit in-memory module bundle without filesystem access.
pub fn compile_modules(
    module: &Module,
    config: &Config,
    modules: &[Module],
) -> Result<rtlc::SafetyProgram> {
    compile_modules_with_options(module, config, modules, CompileOptions::default())
}

pub fn compile_modules_with_options(
    module: &Module,
    config: &Config,
    modules: &[Module],
    options: CompileOptions,
) -> Result<rtlc::SafetyProgram> {
    symbols::validate(module)?;
    for dependency in modules {
        symbols::validate(dependency)?;
    }
    if config.symmetry.is_some() {
        if !modules.is_empty()
            || !module.instances.is_empty()
            || !config.substitutions.is_empty()
            || config
                .constants
                .iter()
                .any(|assignment| assignment.module.is_some() || assignment.args.is_some())
        {
            return unsupported("SYMMETRY requires a flat module and direct constants");
        }
        return compile_flat(module, config, options, false);
    }
    let linking = link::needs_linking(module, config);
    let (linked_module, linked_config, mut action_names) = link::resolve(module, config, modules)?;
    if options.irVersion == IrVersion::V2 && !linking && !elaborate::needed(module) {
        return compile_flat(module, config, options, false);
    }
    let expanded = elaborate::needed(&linked_module)
        .then(|| elaborate::module(&linked_module, &mut action_names))
        .transpose()?;
    let mut program = compile_flat(
        expanded.as_ref().unwrap_or(&linked_module),
        &linked_config,
        options,
        linking,
    )?;
    for branch in program.initial.iter_mut().chain(program.next.iter_mut()) {
        if let Some(name) = action_names.get(&branch.name) {
            branch.name = name.clone();
        }
    }
    Ok(program)
}

fn compile_flat(
    module: &Module,
    config: &Config,
    options: CompileOptions,
    _linking: bool,
) -> Result<rtlc::SafetyProgram> {
    let source_levels = levels::validate(module)?;
    if options.irVersion == IrVersion::V2 && config.symmetry.is_some() {
        return unsupported("safety IR v2 does not support SYMMETRY");
    }
    if module.extends.iter().any(|name| {
        ![
            "Naturals",
            "Integers",
            "Sequences",
            "FiniteSets",
            "TLC",
            "Apalache",
            "Variants",
        ]
        .contains(&name.as_str())
    }) {
        return unsupported("EXTENDS");
    }
    if !config.substitutions.is_empty()
        || config.view.is_some()
        || config.alias.is_some()
        || !config.postconditions.is_empty()
        || !config.possible.is_empty()
        || config.periodic.is_some()
        || config.rl_reward.is_some()
    {
        return unsupported("constrained configuration");
    }
    if if config.specification.is_some() {
        config.init.is_some() || config.next.is_some()
    } else {
        config.init.is_none() || config.next.is_none()
    } {
        return unsupported("use SPECIFICATION or INIT / NEXT");
    }
    if module.variables.is_empty()
        || module.variables.len() > 256
        || module.variables.iter().any(|value| value.arity != 0)
    {
        return unsupported("state variables");
    }
    let symbols: Vec<_> = module
        .variables
        .iter()
        .map(|value| &value.name)
        .chain(module.constants.iter().map(|value| &value.name))
        .chain(module.definitions.iter().map(|value| &value.name))
        .collect();
    if symbols.iter().collect::<HashSet<_>>().len() != symbols.len()
        || module
            .definitions
            .iter()
            .any(|definition| match &definition.head {
                DefinitionKind::Operator { params } => {
                    params.iter().any(|param| symbols.contains(&&param.name))
                        || params
                            .iter()
                            .map(|param| &param.name)
                            .collect::<HashSet<_>>()
                            .len()
                            != params.len()
                }
                _ => false,
            })
    {
        return unsupported("conflicting symbols");
    }
    let mut constants = BTreeMap::new();
    for assignment in &config.constants {
        if assignment.module.is_some()
            || assignment.args.is_some()
            || constants
                .insert(assignment.name.clone(), assignment.value.clone())
                .is_some()
        {
            return unsupported("constant assignment");
        }
    }
    if module
        .constants
        .iter()
        .any(|value| value.arity != 0 || !constants.contains_key(&value.name))
        || constants
            .keys()
            .any(|name| !module.constants.iter().any(|value| &value.name == name))
    {
        return unsupported("missing or extra constants");
    }
    let mut declarations = std::collections::BTreeSet::new();
    let mut recursive_declarations = HashSet::new();
    let mut pending: Vec<_> = module
        .definitions
        .iter()
        .map(Node::SourceDefinition)
        .chain(
            module
                .assumptions
                .iter()
                .map(|value| Node::Source(&value.expression)),
        )
        .chain(
            module
                .theorems
                .iter()
                .map(|value| Node::Source(&value.statement)),
        )
        .collect();
    while let Some(node) = pending.pop() {
        if let Node::SourceDefinition(definition) = node {
            declarations.insert((definition.location.start, definition.name.clone()));
            if definition.is_recursive {
                recursive_declarations.insert((definition.location.start, definition.name.clone()));
            }
        }
        pending.extend(
            analysis::analysis_children(node)
                .into_iter()
                .map(|edge| edge.node),
        );
    }
    let declaration_ids: BTreeMap<_, _> = declarations
        .into_iter()
        .enumerate()
        .map(|(index, key)| (key, index + 1))
        .collect();
    let entries = module
        .definitions
        .iter()
        .map(|definition| {
            (
                definition.name.clone(),
                Rc::new(Entry {
                    id: declaration_ids[&(definition.location.start, definition.name.clone())],
                    definition: Rc::new(definition.clone()),
                    locals: BTreeMap::new(),
                    scope: 0,
                    order: vec![],
                    binding_ids: BTreeMap::new(),
                    projection: BTreeMap::new(),
                    static_targets: BTreeMap::new(),
                    target_actuals: BTreeMap::new(),
                }),
            )
        })
        .collect();
    let mut compiler = Compiler {
        temporal_ids: source_levels
            .iter()
            .filter(|(_, level)| **level == 3)
            .filter_map(|(key, _)| declaration_ids.get(key).copied())
            .collect(),
        recursive_ids: recursive_declarations
            .iter()
            .map(|key| declaration_ids[key])
            .collect(),
        declaration_ids,
        module,
        config,
        constants,
        slots: module
            .variables
            .iter()
            .enumerate()
            .map(|(slot, value)| (value.name.clone(), slot))
            .collect(),
        scopes: vec![entries],
        version: options.irVersion,
        shared: vec![],
        draining: false,
        target_nesting: 0,
        target_requirements: HashMap::new(),
        membership_requirements: HashMap::new(),
        next_binding: 0,
        binding_keys: HashMap::new(),
        generative_cache: HashMap::new(),
        level_cache: std::cell::RefCell::new(HashMap::new()),
    };
    compiler.program()
}

impl Compiler<'_> {
    fn target_context(&mut self, entry: &Entry) -> Context {
        let mut context = self.root(Phase::Next);
        context.origin = entry.id;
        context.scope = entry.scope;
        context.locals = entry.locals.clone();
        context.order = entry.order.clone();
        context.binding_ids = entry.binding_ids.clone();
        context.projection = entry.projection.clone();
        context.static_targets = entry.static_targets.clone();
        context.target_actuals = entry.target_actuals.clone();
        context
    }
    fn resolve_target(&mut self, value: &Expression, context: &Context) -> Result<Target> {
        enum Task {
            Expression(Expression, Box<Context>, HashSet<usize>),
            Tuple(usize),
        }
        let mut tasks = vec![Task::Expression(
            value.clone(),
            Box::new(context.clone()),
            HashSet::new(),
        )];
        let mut results = vec![];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Tuple(count) => {
                    let elements = results.split_off(results.len() - count);
                    results.push(Target::Tuple(elements));
                }
                Task::Expression(value, context, active) => {
                    let (name, args) = match &value.node {
                        E::Tuple { elements: _ } => {
                            let elements = source_children(&value);
                            tasks.push(Task::Tuple(elements.len()));
                            tasks.extend(elements.into_iter().rev().map(|value| {
                                Task::Expression(value, context.clone(), active.clone())
                            }));
                            continue;
                        }
                        E::Label {
                            name: _,
                            params: _,
                            body: _,
                        } => {
                            let body =
                                source_children(&value).pop().ok_or_else(|| CompileError {
                                    message: "generative UNCHANGED requires state variables".into(),
                                })?;
                            tasks.push(Task::Expression(body, context, active));
                            continue;
                        }
                        E::Let {
                            definitions,
                            body: _,
                        } => {
                            let inner = self.let_context(definitions, &context)?;
                            let body =
                                source_children(&value).pop().ok_or_else(|| CompileError {
                                    message: "generative UNCHANGED requires state variables".into(),
                                })?;
                            tasks.push(Task::Expression(body, Box::new(inner), active));
                            continue;
                        }
                        E::Identifier { name } => {
                            if let Some(binding) = context.binding_ids.get(name) {
                                if let Some((actual, caller, ancestors)) =
                                    context.target_actuals.get(binding)
                                {
                                    tasks.push(Task::Expression(
                                        actual.clone(),
                                        Box::new(caller.as_ref().clone()),
                                        ancestors.clone(),
                                    ));
                                } else if let Some(target) = context.static_targets.get(binding) {
                                    results.push(target.clone());
                                } else if context.locals.get(name).is_some_and(|value| {
                                    matches!(
                                        value,
                                        ir::Expression::Parameter { .. }
                                            | ir::Expression::Capture { .. }
                                    )
                                }) {
                                    results.push(Target::Binding(*binding));
                                } else {
                                    return unsupported(
                                        "generative UNCHANGED requires state variables",
                                    );
                                }
                                continue;
                            }
                            if let Some(slot) = self.slots.get(name) {
                                results.push(Target::Slot(*slot));
                                continue;
                            }
                            (name.clone(), vec![])
                        }
                        E::Apply { operator, args: _ } => (
                            operator.clone(),
                            analysis::analysis_children(analysis::Node::SourceCall(&value))
                                .into_iter()
                                .filter_map(|edge| match (edge.descriptor.role, edge.node) {
                                    ("call-actual", analysis::Node::Source(actual)) => {
                                        Some(actual.clone())
                                    }
                                    _ => None,
                                })
                                .collect(),
                        ),
                        _ => return unsupported("generative UNCHANGED requires state variables"),
                    };
                    let entry =
                        self.scopes[context.scope]
                            .get(&name)
                            .cloned()
                            .ok_or_else(|| CompileError {
                                message: "generative UNCHANGED requires state variables".into(),
                            })?;
                    let position = entry.id;
                    if entry.definition.is_recursive || active.contains(&position) {
                        return unsupported("recursive operator definitions");
                    }
                    let DefinitionKind::Operator { params } = &entry.definition.head else {
                        return unsupported("generative UNCHANGED requires state variables");
                    };
                    if params.len() != args.len()
                        || params.iter().any(|parameter| parameter.arity != 0)
                    {
                        return unsupported(format!("operator {name}"));
                    }
                    let mut inner = self.target_context(&entry);
                    for (binding, target) in &context.static_targets {
                        if entry.projection.contains_key(binding) {
                            inner.static_targets.insert(*binding, target.clone());
                        }
                    }
                    for (slot, (parameter, actual)) in params.iter().zip(args).enumerate() {
                        let binding = self.binding_id(position, 0, slot);
                        inner.binding_ids.insert(parameter.name.clone(), binding);
                        inner
                            .locals
                            .insert(parameter.name.clone(), ir::Expression::Parameter { slot });
                        inner
                            .target_actuals
                            .insert(binding, (actual, Rc::new(*context.clone()), active.clone()));
                    }
                    let mut nested_active = active;
                    nested_active.insert(position);
                    tasks.push(Task::Expression(
                        entry.definition.body.clone(),
                        Box::new(inner),
                        nested_active,
                    ));
                }
            }
        }
        results.pop().ok_or_else(|| CompileError {
            message: "generative UNCHANGED requires state variables".into(),
        })
    }
    fn target_bindings(target: &Target, output: &mut Vec<usize>) {
        match target {
            Target::Binding(binding) => {
                if !output.contains(binding) {
                    output.push(*binding);
                }
            }
            Target::Tuple(elements) => {
                for element in elements {
                    Self::target_bindings(element, output);
                }
            }
            Target::Slot(_) => {}
        }
    }
    fn target_json(target: &Target) -> Result<Json> {
        match target {
            Target::Slot(slot) => Ok(json!({"op":"read","slot":slot,"next":false})),
            Target::Tuple(elements) => Ok(
                json!({"op":"tuple","values":elements.iter().map(Self::target_json).collect::<Result<Vec<_>>>()?}),
            ),
            Target::Binding(_) => unsupported("generative UNCHANGED requires state variables"),
        }
    }
    fn requirements(&mut self, name: &str, context: &Context, action: bool) -> Result<Vec<usize>> {
        let position = self.scopes[context.scope][name].id;
        if let Some(result) = self.target_requirements.get(&(position, action)) {
            return Ok(result.clone());
        }
        let (mut result, recursive) = self.requirements_pass(name, context, action, false)?;
        if recursive {
            loop {
                let before = self.target_requirements.clone();
                result = self.requirements_pass(name, context, action, true)?.0;
                if before == self.target_requirements {
                    break;
                }
            }
        }
        Ok(result)
    }
    fn requirements_pass(
        &mut self,
        name: &str,
        context: &Context,
        action: bool,
        refresh: bool,
    ) -> Result<(Vec<usize>, bool)> {
        enum Task {
            Expression(Expression, Context, bool, (usize, bool)),
            Call(
                String,
                Vec<Expression>,
                Context,
                bool,
                Option<(usize, bool)>,
            ),
            Compose(
                (usize, bool),
                Vec<Expression>,
                Context,
                Option<(usize, bool)>,
            ),
            Finish((usize, bool)),
        }
        let mut tasks = vec![Task::Call(
            name.into(),
            vec![],
            context.clone(),
            action,
            None,
        )];
        let mut working: HashMap<(usize, bool), Vec<usize>> = HashMap::new();
        let mut active = HashSet::new();
        let mut visited = HashSet::new();
        let mut recursive = false;
        let mut result = vec![];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Finish(key) => {
                    self.target_requirements
                        .insert(key, working.remove(&key).unwrap_or_default());
                    active.remove(&key);
                }
                Task::Compose(key, args, context, owner) => {
                    let entry = self.scopes[context.scope]
                        .values()
                        .find(|entry| entry.id == key.0)
                        .cloned()
                        .ok_or_else(|| CompileError {
                            message: "lexical capture".into(),
                        })?;
                    let requirements = working
                        .get(&key)
                        .or_else(|| self.target_requirements.get(&key))
                        .cloned()
                        .unwrap_or_default();
                    let mut mapped = vec![];
                    for binding in requirements {
                        let parameter =
                            if let DefinitionKind::Operator { params } = &entry.definition.head {
                                params.iter().enumerate().find_map(|(slot, _)| {
                                    (self.binding_id(key.0, 0, slot) == binding).then_some(slot)
                                })
                            } else {
                                None
                            };
                        if let Some(slot) = parameter {
                            if key.1 && entry.definition.is_recursive {
                                return unsupported(
                                    "recursive operator parameters cannot be primed",
                                );
                            }
                            if let Some(actual) = args.get(slot) {
                                let target = self.resolve_target(actual, &context)?;
                                Self::target_bindings(&target, &mut mapped);
                            } else if owner.is_none() {
                                if !mapped.contains(&binding) {
                                    mapped.push(binding);
                                }
                            } else {
                                return unsupported(format!("operator {}", entry.definition.name));
                            }
                        } else if !mapped.contains(&binding) {
                            mapped.push(binding);
                        }
                    }
                    if let Some(owner) = owner {
                        let output = working.get_mut(&owner).unwrap();
                        for binding in mapped {
                            if !output.contains(&binding) {
                                output.push(binding);
                            }
                        }
                    } else {
                        result = mapped;
                    }
                }
                Task::Call(name, args, context, action, owner) => {
                    let entry =
                        self.scopes[context.scope]
                            .get(&name)
                            .cloned()
                            .ok_or_else(|| CompileError {
                                message: format!("operator {name}"),
                            })?;
                    let key = (entry.id, action);
                    if entry.definition.is_recursive && !action {
                        self.target_requirements.entry(key).or_default();
                        tasks.push(Task::Compose(key, args, context.clone(), owner));
                        continue;
                    }
                    recursive |= entry.definition.is_recursive;
                    if active.contains(&key) {
                        if !entry.definition.is_recursive {
                            return unsupported(
                                "recursive operator definitions require RECURSIVE declarations",
                            );
                        }
                        tasks.push(Task::Compose(key, args, context, owner));
                        continue;
                    }
                    tasks.push(Task::Compose(key, args, context.clone(), owner));
                    if refresh {
                        if !visited.insert(key) {
                            continue;
                        }
                    } else if self.target_requirements.contains_key(&key) {
                        continue;
                    }
                    active.insert(key);
                    working.insert(
                        key,
                        self.target_requirements
                            .get(&key)
                            .cloned()
                            .unwrap_or_default(),
                    );
                    let mut inner = self.target_context(&entry);
                    inner.static_targets.clear();
                    inner.target_actuals.clear();
                    for (name, binding) in &entry.binding_ids {
                        inner
                            .locals
                            .insert(name.clone(), ir::Expression::Capture { slot: *binding });
                    }
                    if let DefinitionKind::Operator { params } = &entry.definition.head {
                        for (slot, parameter) in params.iter().enumerate() {
                            let binding = self.binding_id(key.0, 0, slot);
                            inner.binding_ids.insert(parameter.name.clone(), binding);
                            inner
                                .locals
                                .insert(parameter.name.clone(), ir::Expression::Parameter { slot });
                        }
                    }
                    tasks.push(Task::Finish(key));
                    tasks.push(Task::Expression(
                        definition_body(&entry.definition),
                        inner,
                        action,
                        key,
                    ));
                }
                Task::Expression(value, context, action, owner) => {
                    match &value.node {
                        E::Identifier { name }
                            if self.has(&context, name) && !context.locals.contains_key(name) =>
                        {
                            tasks.push(Task::Call(
                                name.clone(),
                                vec![],
                                context,
                                action,
                                Some(owner),
                            ));
                            continue;
                        }
                        E::Apply { operator, args: _ } if self.has(&context, operator) => {
                            let args =
                                analysis::analysis_children(analysis::Node::SourceCall(&value))
                                    .into_iter()
                                    .filter_map(|edge| match (edge.descriptor.role, edge.node) {
                                        ("call-actual", analysis::Node::Source(actual)) => {
                                            Some(actual.clone())
                                        }
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>();
                            for actual in args.iter().rev() {
                                tasks.push(Task::Expression(
                                    actual.clone(),
                                    context.clone(),
                                    false,
                                    owner,
                                ));
                            }
                            tasks.push(Task::Call(
                                operator.clone(),
                                args,
                                context,
                                action,
                                Some(owner),
                            ));
                            continue;
                        }
                        E::Unchanged { expression: _ }
                        | E::BoxAction {
                            action: _,
                            subscript: _,
                        }
                        | E::AngleAction {
                            action: _,
                            subscript: _,
                        } if action => {
                            for edge in analysis::analysis_children(analysis::Node::Source(&value))
                                .into_iter()
                                .rev()
                            {
                                let analysis::Node::Source(child) = edge.node else {
                                    continue;
                                };
                                match edge.descriptor.role {
                                    "generative-target" | "subscript" => {
                                        let target = self.resolve_target(child, &context)?;
                                        Self::target_bindings(
                                            &target,
                                            working.get_mut(&owner).unwrap(),
                                        );
                                    }
                                    "action-body" => tasks.push(Task::Expression(
                                        child.clone(),
                                        context.clone(),
                                        true,
                                        owner,
                                    )),
                                    _ => {}
                                }
                            }
                            continue;
                        }
                        _ => {}
                    }
                    for (child, inner, descriptor) in self
                        .requirement_children(&value, &context)?
                        .into_iter()
                        .rev()
                    {
                        let child_action = descriptor.action == "start"
                            || action && descriptor.action == "inherit";
                        tasks.push(Task::Expression(child, inner, child_action, owner));
                    }
                }
            }
        }
        Ok((result, recursive))
    }
    fn membership_demands(
        &mut self,
        name: &str,
        context: &Context,
        purpose: bool,
    ) -> Result<Vec<usize>> {
        enum Task {
            Expression(Expression, Context, bool, (usize, bool)),
            Call(
                String,
                Vec<Expression>,
                Context,
                bool,
                Option<(usize, bool)>,
            ),
            Compose(
                (usize, bool),
                Vec<Expression>,
                Context,
                Option<(usize, bool)>,
            ),
            Finish((usize, bool)),
        }
        let mut tasks = vec![Task::Call(
            name.into(),
            vec![],
            context.clone(),
            purpose,
            None,
        )];
        let mut working: HashMap<(usize, bool), Vec<usize>> = HashMap::new();
        let mut active = HashSet::new();
        let mut result = vec![];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Finish(key) => {
                    self.membership_requirements
                        .insert(key, working.remove(&key).unwrap_or_default());
                    active.remove(&key);
                }
                Task::Compose(key, args, context, owner) => {
                    let required = self.membership_requirements[&key].clone();
                    if let Some(owner) = owner {
                        let entry = self.scopes[context.scope]
                            .values()
                            .find(|entry| entry.id == key.0)
                            .cloned()
                            .ok_or_else(|| CompileError {
                                message: "lexical capture".into(),
                            })?;
                        let mut expressions = vec![];
                        for binding in required {
                            let parameter = if let DefinitionKind::Operator { params } =
                                &entry.definition.head
                            {
                                params.iter().enumerate().find_map(|(slot, _)| {
                                    (self.binding_id(key.0, 0, slot) == binding).then_some(slot)
                                })
                            } else {
                                None
                            };
                            if let Some(slot) = parameter {
                                if let Some(actual) = args.get(slot) {
                                    expressions.push(actual.clone());
                                } else {
                                    return unsupported(format!(
                                        "operator {}",
                                        entry.definition.name
                                    ));
                                }
                            } else {
                                let output = working.get_mut(&owner).unwrap();
                                if !output.contains(&binding) {
                                    output.push(binding);
                                }
                            }
                        }
                        tasks.extend(
                            expressions
                                .into_iter()
                                .rev()
                                .map(|value| Task::Expression(value, context.clone(), true, owner)),
                        );
                    } else {
                        result = required;
                    }
                }
                Task::Call(name, args, context, purpose, owner) => {
                    let entry =
                        self.scopes[context.scope]
                            .get(&name)
                            .cloned()
                            .ok_or_else(|| CompileError {
                                message: format!("operator {name}"),
                            })?;
                    let key = (entry.id, purpose);
                    if entry.definition.is_recursive {
                        self.membership_requirements.entry(key).or_default();
                        tasks.push(Task::Compose(key, args, context, owner));
                        continue;
                    }
                    if active.contains(&key) {
                        return unsupported(
                            "recursive operator definitions require RECURSIVE declarations",
                        );
                    }
                    tasks.push(Task::Compose(key, args, context, owner));
                    if self.membership_requirements.contains_key(&key) {
                        continue;
                    }
                    active.insert(key);
                    working.insert(key, vec![]);
                    let mut inner = self.target_context(&entry);
                    for (name, binding) in &entry.binding_ids {
                        inner
                            .locals
                            .insert(name.clone(), ir::Expression::Capture { slot: *binding });
                    }
                    if let DefinitionKind::Operator { params } = &entry.definition.head {
                        for (slot, parameter) in params.iter().enumerate() {
                            let binding = self.binding_id(key.0, 0, slot);
                            inner.binding_ids.insert(parameter.name.clone(), binding);
                            inner
                                .locals
                                .insert(parameter.name.clone(), ir::Expression::Parameter { slot });
                        }
                    }
                    tasks.push(Task::Finish(key));
                    tasks.push(Task::Expression(
                        definition_body(&entry.definition),
                        inner,
                        purpose,
                        key,
                    ));
                }
                Task::Expression(value, context, purpose, owner) => {
                    match &value.node {
                        E::Identifier { name } => {
                            if purpose
                                && let Some(binding) = context.binding_ids.get(name)
                                && context.locals.get(name).is_some_and(|value| {
                                    matches!(
                                        value,
                                        ir::Expression::Parameter { .. }
                                            | ir::Expression::Capture { .. }
                                    )
                                })
                            {
                                let output = working.get_mut(&owner).unwrap();
                                if !output.contains(binding) {
                                    output.push(*binding);
                                }
                            }
                            if self.has(&context, name) && !context.locals.contains_key(name) {
                                tasks.push(Task::Call(
                                    name.clone(),
                                    vec![],
                                    context,
                                    purpose,
                                    Some(owner),
                                ));
                            }
                            continue;
                        }
                        E::Apply { operator, args: _ } if self.has(&context, operator) => {
                            let args =
                                analysis::analysis_children(analysis::Node::SourceCall(&value))
                                    .into_iter()
                                    .filter_map(|edge| match (edge.descriptor.role, edge.node) {
                                        ("call-actual", analysis::Node::Source(actual)) => {
                                            Some(actual.clone())
                                        }
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>();
                            for actual in args.iter().rev() {
                                tasks.push(Task::Expression(
                                    actual.clone(),
                                    context.clone(),
                                    false,
                                    owner,
                                ));
                            }
                            tasks.push(Task::Call(
                                operator.clone(),
                                args,
                                context,
                                purpose,
                                Some(owner),
                            ));
                            continue;
                        }
                        _ => {}
                    }
                    for (child, inner, descriptor) in self
                        .requirement_children(&value, &context)?
                        .into_iter()
                        .rev()
                    {
                        let child_purpose = descriptor.membership == "start"
                            || purpose && descriptor.membership == "inherit";
                        tasks.push(Task::Expression(child, inner, child_purpose, owner));
                    }
                }
            }
        }
        Ok(result)
    }
    fn requirement_children(
        &mut self,
        value: &Expression,
        context: &Context,
    ) -> Result<Vec<(Expression, Context, analysis::Descriptor)>> {
        let scoped = match &value.node {
            E::Let {
                definitions,
                body: _,
            } => Some(self.let_context(definitions, context)?),
            E::Forall { bounds, body: _ }
            | E::Exists { bounds, body: _ }
            | E::FunctionConstruction { bounds, body: _ }
            | E::SetMap {
                bounds,
                expression: _,
            } => Some(self.requirement_bind(bounds, context)),
            E::SetFilter {
                bound,
                predicate: _,
            } => Some(self.requirement_bind(std::slice::from_ref(bound), context)),
            E::Choose {
                pattern,
                set: Some(set),
                body: _,
            } => Some(self.requirement_bind(
                &[ast::Bound {
                    pattern: pattern.clone(),
                    set: *set.clone(),
                }],
                context,
            )),
            _ => None,
        };
        Ok(analysis::analysis_children(analysis::Node::Source(value))
            .into_iter()
            .filter_map(|edge| {
                let analysis::Node::Source(child) = edge.node else {
                    return None;
                };
                let inner =
                    if edge.descriptor.scope == "binder" || matches!(value.node, E::Let { .. }) {
                        scoped.as_ref().unwrap_or(context)
                    } else {
                        context
                    };
                Some((child.clone(), inner.clone(), edge.descriptor))
            })
            .collect())
    }
    fn requirement_bind(&mut self, bounds: &[ast::Bound], context: &Context) -> Context {
        let mut inner = context.clone();
        for bound in bounds {
            let names = match &bound.pattern {
                Pattern::Name { name } => vec![name],
                Pattern::Tuple { names } => names.iter().collect(),
            };
            for (slot, name) in names.iter().enumerate() {
                let binding =
                    self.named_binding_at(context.origin, bound.set.location.start, 1, slot, name);
                inner.binding_ids.insert((*name).clone(), binding);
                inner
                    .locals
                    .insert((*name).clone(), ir::Expression::Bound { slot: 0 });
            }
        }
        inner
    }

    fn binding_id(&mut self, position: usize, kind: u8, slot: usize) -> usize {
        self.binding_at(0, position, kind, slot)
    }
    fn binding_at(&mut self, origin: usize, position: usize, kind: u8, slot: usize) -> usize {
        self.named_binding_at(origin, position, kind, slot, "")
    }
    fn named_binding_at(
        &mut self,
        origin: usize,
        position: usize,
        kind: u8,
        slot: usize,
        name: &str,
    ) -> usize {
        let key = (origin, position, kind, slot, name.into());
        if let Some(id) = self.binding_keys.get(&key) {
            return *id;
        }
        let id = self.next_binding;
        self.next_binding += 1;
        self.binding_keys.insert(key, id);
        id
    }
    fn level(&self, value: &Json) -> u8 {
        level(value)
    }
    fn prepare_levels(&self, order: &[usize]) {
        self.level_cache.borrow_mut().clear();
        loop {
            let mut changed = false;
            for id in order {
                let body = &self.shared[*id].definition.as_ref().unwrap().body;
                let mut summary =
                    typed::level_summary(Node::Body(body), &self.level_cache.borrow());
                if self.shared[*id].key.4 == ir::BodyKind::Action {
                    summary.base =
                        summary
                            .base
                            .max(if self.shared[*id].key.1 == 1 { 1 } else { 2 });
                }
                if self.shared[*id].key.4 == ir::BodyKind::Temporal {
                    summary.base = 3;
                }
                changed |= self.level_cache.borrow().get(id) != Some(&summary);
                self.level_cache.borrow_mut().insert(*id, summary);
            }
            if !changed || !self.shared.iter().any(|entry| entry.recursive) {
                break;
            }
        }
    }

    fn prime(&mut self, value: Json) -> Result<Json> {
        prime(value)
    }
    fn validate_temporal(&self, formula: &Json, path: &[String], direct: bool) -> Result<()> {
        validate_temporal(formula, path, direct)
    }
    fn shared_call(
        &mut self,
        name: &str,
        args: &[Expression],
        context: &Context,
        membership: bool,
    ) -> Result<Json> {
        let child = context.nested()?;
        let demanded = self.membership_demands(name, context, membership)?;
        let position = self.scopes[context.scope][name].id;
        let arguments = args
            .iter()
            .enumerate()
            .map(|(slot, argument)| {
                if membership || demanded.contains(&self.binding_id(position, 0, slot)) {
                    self.membership_set(argument, &child)
                } else {
                    self.expression(argument, &child)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let targets = self.call_targets(name, args, context)?;
        self.shared_arguments(name, arguments, context, membership, targets)
    }
    fn call_targets(
        &mut self,
        name: &str,
        args: &[Expression],
        context: &Context,
    ) -> Result<BTreeMap<usize, Target>> {
        self.call_targets_purpose(name, args, context, false)
    }
    fn call_targets_purpose(
        &mut self,
        name: &str,
        args: &[Expression],
        context: &Context,
        action: bool,
    ) -> Result<BTreeMap<usize, Target>> {
        let entry = self.scopes[context.scope]
            .get(name)
            .cloned()
            .ok_or_else(|| CompileError {
                message: format!("operator {name}"),
            })?;
        let position = entry.id;
        let required = self.requirements(name, context, action)?;
        let mut targets = BTreeMap::new();
        for binding in required {
            let parameter = if let DefinitionKind::Operator { params } = &entry.definition.head {
                params.iter().enumerate().find_map(|(slot, _)| {
                    (self.binding_id(position, 0, slot) == binding).then_some(slot)
                })
            } else {
                None
            };
            let target = if let Some(slot) = parameter {
                self.resolve_target(
                    args.get(slot).ok_or_else(|| CompileError {
                        message: format!("operator {name}"),
                    })?,
                    context,
                )?
            } else if let Some((actual, caller, _)) = context
                .target_actuals
                .get(&binding)
                .or_else(|| entry.target_actuals.get(&binding))
            {
                self.resolve_target(actual, caller)?
            } else {
                context
                    .static_targets
                    .get(&binding)
                    .or_else(|| entry.static_targets.get(&binding))
                    .cloned()
                    .ok_or_else(|| CompileError {
                        message: "generative UNCHANGED requires state variables".into(),
                    })?
            };
            Self::target_json(&target)?;
            targets.insert(binding, target);
        }
        Ok(targets)
    }
    fn shared_arguments(
        &mut self,
        name: &str,
        arguments: Vec<Json>,
        context: &Context,
        membership: bool,
        targets: BTreeMap<usize, Target>,
    ) -> Result<Json> {
        Ok(json!(
            self.shared_arguments_ir(
                name,
                arguments
                    .into_iter()
                    .map(decode_expression)
                    .collect::<Result<_>>()?,
                context,
                membership,
                targets
            )?
        ))
    }
    fn shared_arguments_ir(
        &mut self,
        name: &str,
        arguments: Vec<ir::Expression>,
        context: &Context,
        membership: bool,
        targets: BTreeMap<usize, Target>,
    ) -> Result<ir::Expression> {
        self.shared_body_ir(
            name,
            arguments,
            context,
            membership,
            targets,
            ir::BodyKind::Expression,
        )
    }
    fn shared_body_ir(
        &mut self,
        name: &str,
        arguments: Vec<ir::Expression>,
        context: &Context,
        membership: bool,
        mut targets: BTreeMap<usize, Target>,
        kind: ir::BodyKind,
    ) -> Result<ir::Expression> {
        let entry = self.scopes[context.scope]
            .get(name)
            .map(|entry| entry.as_ref().clone())
            .ok_or_else(|| CompileError {
                message: format!("operator {name}"),
            })?;
        let position = entry.id;
        let (body, params) = match &entry.definition.head {
            DefinitionKind::Operator { params } => {
                if params.len() != arguments.len()
                    || params.iter().any(|parameter| parameter.arity != 0)
                {
                    return unsupported(format!("operator {name}"));
                }
                (
                    entry.definition.body.clone(),
                    params
                        .iter()
                        .map(|parameter| parameter.name.clone())
                        .collect::<Vec<_>>(),
                )
            }
            DefinitionKind::Function { bounds } => {
                if !arguments.is_empty() {
                    return unsupported(format!("function operator {name}"));
                }
                (
                    Expression {
                        node: E::FunctionConstruction {
                            bounds: bounds.clone(),
                            body: Box::new(entry.definition.body.clone()),
                        },
                        location: entry.definition.location,
                    },
                    vec![],
                )
            }
        };
        let phase = match context.phase {
            Phase::Constant => 0,
            Phase::Initial => 1,
            Phase::State => 2,
            Phase::Next => 3,
        };
        let required = self.requirements(name, context, kind == ir::BodyKind::Action)?;
        for binding in &required {
            if !targets.contains_key(binding)
                && let Some(target) = context
                    .static_targets
                    .get(binding)
                    .or_else(|| entry.static_targets.get(binding))
            {
                targets.insert(*binding, target.clone());
            }
        }
        let signature = required
            .iter()
            .map(|binding| {
                targets.get(binding).cloned().ok_or_else(|| CompileError {
                    message: "generative UNCHANGED requires state variables".into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let key = (position, phase, membership, signature, kind);
        let id = if let Some(id) = self.shared.iter().position(|shared| shared.key == key) {
            id
        } else {
            let id = self.shared.len();
            let candidates: Vec<_> = entry.projection.keys().copied().collect();
            let mut inner = self.root(context.phase);
            inner.origin = entry.id;
            inner.scope = entry.scope;
            inner.static_targets = targets;
            for (slot, binding) in candidates.iter().enumerate() {
                inner
                    .projection
                    .insert(*binding, ir::Expression::Capture { slot });
            }
            for name in &entry.order {
                let binding = entry.binding_ids[name];
                inner
                    .locals
                    .insert(name.clone(), inner.projection[&binding].clone());
                inner.binding_ids.insert(name.clone(), binding);
                inner.order.push(name.clone());
            }
            for (slot, name) in params.iter().enumerate() {
                let value = ir::Expression::Parameter { slot };
                inner.locals.insert(name.clone(), value.clone());
                inner.order.retain(|other| other != name);
                inner.order.push(name.clone());
                let binding = self.binding_id(position, 0, slot);
                inner.binding_ids.insert(name.clone(), binding);
                inner.projection.insert(binding, value);
            }
            self.shared.push(Shared {
                recursive: entry.definition.is_recursive,
                key,
                captures: candidates.clone(),
                definition: None,
                parameter_count: params.len(),
                pending: Some((body, inner, membership)),
            });
            id
        };
        let capture_bindings = self.shared[id].captures.clone();
        let mut captures = vec![];
        for binding in capture_bindings {
            let value = context
                .projection
                .get(&binding)
                .or_else(|| entry.projection.get(&binding))
                .cloned()
                .ok_or_else(|| CompileError {
                    message: "lexical capture".into(),
                })?;
            captures.push(value);
        }
        Ok(ir::Expression::Call {
            definition: id,
            arguments,
            captures,
        })
    }
    fn lower_definition(&mut self, id: usize) -> Result<()> {
        if let Some((body, context, membership)) = self.shared[id].pending.take() {
            let body = if self.shared[id].key.4 == ir::BodyKind::Temporal {
                ir::DefinitionBody::Temporal(Box::new(self.temporal_ir(&body, &context)?))
            } else if self.shared[id].key.4 == ir::BodyKind::Action {
                ir::DefinitionBody::Action(self.action_ir(&body, &context, "<action>")?)
            } else if membership {
                self.membership_ir(&body, &context)?.into()
            } else {
                self.expression_ir(&body, &context)?.into()
            };
            self.shared[id].definition = Some(ir::Definition {
                recursive: self.shared[id].recursive,
                id,
                parameter_count: self.shared[id].parameter_count,
                capture_count: self.shared[id].captures.len(),
                body_kind: self.shared[id].key.4,
                local_scope: typed::local_scope(Node::Body(&body)),
                body,
            });
        }
        Ok(())
    }
    fn lower_pending(&mut self) -> Result<()> {
        if self.draining {
            return Ok(());
        }
        self.draining = true;
        let mut id = 0;
        while id < self.shared.len() {
            self.lower_definition(id)?;
            id += 1;
        }
        self.draining = false;
        self.level_cache.borrow_mut().clear();
        let order = self.definition_order()?;
        self.prepare_levels(&order);
        Ok(())
    }
    fn finish_shared(&mut self, mut program: ir::SafetyProgram) -> Result<ir::SafetyProgram> {
        self.lower_pending()?;
        let order = self.definition_order()?;
        self.prepare_levels(&order);
        loop {
            let mut anonymous_changed = typed::compact_anonymous(NodeMut::Program(&mut program));
            for entry in &mut self.shared {
                anonymous_changed |= typed::compact_anonymous(NodeMut::Body(
                    &mut entry.definition.as_mut().unwrap().body,
                ));
            }
            let mappings: Vec<HashMap<_, _>> = self
                .shared
                .iter()
                .map(|entry| {
                    let used =
                        typed::capture_slots(Node::Body(&entry.definition.as_ref().unwrap().body));
                    (0..entry.captures.len())
                        .filter(|slot| used.contains(slot))
                        .enumerate()
                        .map(|(new, old)| (old, new))
                        .collect()
                })
                .collect();
            if self
                .shared
                .iter()
                .zip(&mappings)
                .all(|(entry, mapping)| entry.captures.len() == mapping.len())
                && !anonymous_changed
            {
                break;
            }
            for (id, entry) in self.shared.iter_mut().enumerate() {
                let mut slot = 0;
                entry.captures.retain(|_| {
                    let keep = mappings[id].contains_key(&slot);
                    slot += 1;
                    keep
                });
                let definition = entry.definition.as_mut().unwrap();
                typed::remap_captures(NodeMut::Body(&mut definition.body), &mappings[id]);
                definition.capture_count = entry.captures.len();
                typed::trim_call_captures(NodeMut::Body(&mut definition.body), &mappings);
            }
            typed::trim_call_captures(NodeMut::Program(&mut program), &mappings);
        }
        self.prepare_levels(&order);
        for entry in &self.shared {
            typed::check_primes(
                Node::Body(&entry.definition.as_ref().unwrap().body),
                &self.level_cache.borrow(),
            )?;
        }
        typed::check_primes(Node::Program(&program), &self.level_cache.borrow())?;
        typed::check_shared_expansion(
            &program,
            &self
                .shared
                .iter()
                .map(|entry| entry.definition.as_ref().unwrap())
                .collect::<Vec<_>>(),
            &order,
        )?;
        let mut reached = HashSet::new();
        let mut pending: Vec<_> = typed::call_ids(Node::Program(&program))
            .into_iter()
            .collect();
        while let Some(id) = pending.pop() {
            if reached.insert(id) {
                pending.extend(typed::call_ids(Node::Body(
                    &self.shared[id].definition.as_ref().unwrap().body,
                )));
            }
        }
        let mut order: Vec<_> = (0..self.shared.len())
            .filter(|id| self.shared[*id].key.4 != ir::BodyKind::Temporal || reached.contains(id))
            .collect();
        order.sort_by(|left, right| self.shared[*left].key.cmp(&self.shared[*right].key));
        let mapping: HashMap<_, _> = order
            .iter()
            .enumerate()
            .map(|(new, old)| (*old, new))
            .collect();
        typed::remap_calls(NodeMut::Program(&mut program), &mapping);
        program.definitions = Some(
            order
                .iter()
                .enumerate()
                .map(|(new, old)| {
                    let mut definition = self.shared[*old].definition.take().unwrap();
                    definition.id = new;
                    typed::remap_calls(NodeMut::Definition(&mut definition), &mapping);
                    definition
                })
                .collect(),
        );
        typed::check_depth(Node::Program(&program))?;
        Ok(program)
    }
    fn definition_order(&self) -> Result<Vec<usize>> {
        let edges: Vec<Vec<_>> = self
            .shared
            .iter()
            .map(|entry| {
                let mut calls = HashSet::new();
                calls.extend(typed::call_ids(Node::Body(
                    &entry.definition.as_ref().unwrap().body,
                )));
                calls.into_iter().collect()
            })
            .collect();
        let mut states = vec![0_u8; self.shared.len()];
        let mut order = vec![];
        for root in 0..self.shared.len() {
            if states[root] != 0 {
                continue;
            }
            states[root] = 1;
            let mut pending = vec![(root, 0)];
            while let Some((id, edge)) = pending.last_mut() {
                if let Some(&callee) = edges[*id].get(*edge) {
                    *edge += 1;
                    match states[callee] {
                        0 => {
                            states[callee] = 1;
                            pending.push((callee, 0));
                        }
                        1 => {
                            let start = pending.iter().position(|(id, _)| *id == callee).unwrap();
                            if pending[start..]
                                .iter()
                                .any(|(id, _)| !self.shared[*id].recursive)
                            {
                                return unsupported(
                                    "recursive operator definitions require RECURSIVE declarations",
                                );
                            }
                        }
                        _ => {}
                    }
                } else {
                    states[*id] = 2;
                    order.push(*id);
                    pending.pop();
                }
            }
        }
        Ok(order)
    }
    fn root(&self, phase: Phase) -> Context {
        Context {
            origin: 0,
            phase,
            locals: BTreeMap::new(),
            calls: vec![],
            call_ids: vec![],
            depth: 0,
            bound: 0,
            scope: 0,
            order: vec![],
            binding_ids: BTreeMap::new(),
            projection: BTreeMap::new(),
            static_targets: BTreeMap::new(),
            target_actuals: BTreeMap::new(),
            frame: None,
            temporal_frames: vec![],
        }
    }
    fn has(&self, context: &Context, name: &str) -> bool {
        self.scopes[context.scope].contains_key(name)
    }
    fn extends(&self, name: &str) -> bool {
        self.module.extends.iter().any(|item| item == name)
    }
    fn arithmetic(&self) -> bool {
        self.module.extends.iter().any(|name| {
            ["Naturals", "Integers", "Sequences", "FiniteSets"].contains(&name.as_str())
        })
    }
    fn call(
        &mut self,
        name: &str,
        args: &[Expression],
        context: &Context,
    ) -> Result<(Expression, Context)> {
        let entry = self.scopes[context.scope]
            .get(name)
            .map(|entry| entry.as_ref().clone())
            .ok_or_else(|| CompileError {
                message: format!("operator {name}"),
            })?;
        if self.version == IrVersion::V1 && args.iter().any(free_old) && self.rebinds_old(&entry) {
            return unsupported("EXCEPT @ arguments across a nested EXCEPT require safety IR v2");
        }
        if entry.definition.is_recursive
            || if self.version == IrVersion::V1 {
                context.calls.iter().any(|item| item == name)
            } else {
                context.call_ids.contains(&entry.id)
            }
        {
            return unsupported(format!("operator {name}"));
        }
        let position = entry.id;
        if self.version == IrVersion::V2 {
            let mut inner = self.target_context(&entry);
            inner.phase = context.phase;
            inner.temporal_frames = context.temporal_frames.clone();
            inner.depth = context.nested()?.depth;
            inner.bound = 0;
            inner.calls = context.calls.clone();
            inner.call_ids = context.call_ids.clone();
            inner.calls.push(name.into());
            inner.call_ids.push(position);
            inner.locals.clear();
            inner.projection.clear();
            inner.static_targets.extend(context.static_targets.clone());
            inner.target_actuals.extend(context.target_actuals.clone());
            let candidates: Vec<_> = entry.projection.keys().copied().collect();
            let captures = candidates
                .iter()
                .map(|binding| {
                    context
                        .projection
                        .get(binding)
                        .or_else(|| entry.projection.get(binding))
                        .cloned()
                        .ok_or_else(|| CompileError {
                            message: "lexical capture".into(),
                        })
                })
                .collect::<Result<Vec<_>>>()?;
            for (slot, binding) in candidates.iter().enumerate() {
                inner
                    .projection
                    .insert(*binding, ir::Expression::Capture { slot });
                if let Some(target) = context.static_targets.get(binding) {
                    inner.static_targets.insert(*binding, target.clone());
                }
                if let Some(actual) = context.target_actuals.get(binding) {
                    inner.target_actuals.insert(*binding, actual.clone());
                }
            }
            for name in &entry.order {
                inner.locals.insert(
                    name.clone(),
                    inner.projection[&entry.binding_ids[name]].clone(),
                );
            }
            let mut arguments = vec![];
            let body = match &entry.definition.head {
                DefinitionKind::Operator { params } => {
                    if params.len() != args.len()
                        || params.iter().any(|parameter| parameter.arity != 0)
                    {
                        return unsupported(format!("operator {name}"));
                    }
                    let demanded = self.membership_demands(name, context, false)?;
                    for (slot, (parameter, actual)) in params.iter().zip(args).enumerate() {
                        let binding = self.binding_id(position, 0, slot);
                        arguments.push(if demanded.contains(&binding) {
                            self.membership_ir(actual, context)?
                        } else {
                            self.expression_ir(actual, context)?
                        });
                        let parameter_value = ir::Expression::Parameter { slot };
                        inner
                            .locals
                            .insert(parameter.name.clone(), parameter_value.clone());
                        inner.binding_ids.insert(parameter.name.clone(), binding);
                        inner.projection.insert(binding, parameter_value);
                        inner.order.retain(|name| name != &parameter.name);
                        inner.order.push(parameter.name.clone());
                        inner.target_actuals.insert(
                            binding,
                            (actual.clone(), Rc::new(context.clone()), HashSet::new()),
                        );
                    }
                    entry.definition.body.clone()
                }
                DefinitionKind::Function { bounds } => {
                    if !args.is_empty() {
                        return unsupported(format!("function operator {name}"));
                    }
                    Expression {
                        node: E::FunctionConstruction {
                            bounds: bounds.clone(),
                            body: Box::new(entry.definition.body.clone()),
                        },
                        location: entry.definition.location,
                    }
                }
            };
            if !arguments.is_empty() || !captures.is_empty() || context.bound > 0 {
                inner.frame = Some(AnonymousFrame {
                    arguments,
                    captures,
                });
            }
            if let Some(frame) = &inner.frame {
                inner.temporal_frames.push(frame.clone());
            }
            return Ok((body, inner));
        }
        let mut locals = entry.locals;
        if self.version == IrVersion::V2 {
            for (name, binding) in &entry.binding_ids {
                if let Some(value) = context.projection.get(binding) {
                    locals.insert(name.clone(), value.clone());
                }
            }
        }
        let mut projection: BTreeMap<_, _> = entry
            .projection
            .iter()
            .map(|(binding, value)| {
                (
                    *binding,
                    context.projection.get(binding).unwrap_or(value).clone(),
                )
            })
            .collect();
        let mut binding_ids = entry.binding_ids;
        let mut order = entry.order;
        let body = match &entry.definition.head {
            DefinitionKind::Operator { params } => {
                if params.len() != args.len() || params.iter().any(|param| param.arity != 0) {
                    return unsupported(format!("operator {name}"));
                }
                for (slot, (param, argument)) in params.iter().zip(args).enumerate() {
                    locals.insert(
                        param.name.clone(),
                        decode_expression(self.expression(argument, context)?)?,
                    );
                    order.retain(|name| name != &param.name);
                    order.push(param.name.clone());
                    let binding = self.binding_id(position, 0, slot);
                    binding_ids.insert(param.name.clone(), binding);
                    projection.insert(binding, locals[&param.name].clone());
                }
                entry.definition.body.clone()
            }
            DefinitionKind::Function { bounds } => {
                if !args.is_empty() {
                    return unsupported(format!("function operator {name}"));
                }
                Expression {
                    node: E::FunctionConstruction {
                        bounds: bounds.clone(),
                        body: Box::new(entry.definition.body.clone()),
                    },
                    location: entry.definition.location,
                }
            }
        };
        let mut child = context.nested()?;
        child.origin = entry.id;
        child.locals = locals;
        child.order = order;
        child.binding_ids = binding_ids;
        child.projection = projection;
        child.scope = entry.scope;
        child.calls.push(name.into());
        child.call_ids.push(position);
        Ok((body, child))
    }
    fn rebinds_old(&self, entry: &Entry) -> bool {
        let mut visited = HashSet::new();
        let mut pending = vec![(Node::SourceDefinition(&entry.definition), entry.scope)];
        while let Some((node, scope)) = pending.pop() {
            if matches!(
                node,
                Node::Source(Expression {
                    node: E::Except { .. },
                    ..
                })
            ) {
                return true;
            }
            if let Node::Source(Expression {
                node: E::Identifier { name } | E::Apply { operator: name, .. },
                ..
            }) = node
                && let Some(callee) = self.scopes[scope].get(name)
                && visited.insert(callee.id)
            {
                pending.push((Node::SourceDefinition(&callee.definition), callee.scope));
            }
            pending.extend(
                analysis::analysis_children(node)
                    .into_iter()
                    .map(|edge| (edge.node, scope)),
            );
        }
        false
    }
    fn let_context(&mut self, definitions: &[Definition], context: &Context) -> Result<Context> {
        for definition in definitions {
            let imported = match definition.name.as_str() {
                "SelectSeq" => self.extends("Sequences"),
                "SortSeq" | "Assert" => self.extends("TLC"),
                _ => false,
            };
            if imported {
                return unsupported(format!("duplicate LET definition {}", definition.name));
            }
        }
        if definitions
            .iter()
            .map(|definition| &definition.name)
            .collect::<HashSet<_>>()
            .len()
            != definitions.len()
        {
            return unsupported("duplicate LET definitions");
        }
        let scope = self.scopes.len();
        let mut entries = self.scopes[context.scope].clone();
        for definition in definitions {
            entries.insert(
                definition.name.clone(),
                Rc::new(Entry {
                    id: self.declaration_ids[&(definition.location.start, definition.name.clone())],
                    definition: Rc::new(definition.clone()),
                    locals: context.locals.clone(),
                    scope,
                    order: context.order.clone(),
                    binding_ids: context.binding_ids.clone(),
                    projection: context.projection.clone(),
                    static_targets: context.static_targets.clone(),
                    target_actuals: context.target_actuals.clone(),
                }),
            );
        }
        self.scopes.push(entries);
        Ok(Context {
            scope,
            ..context.nested()?
        })
    }
    fn bind(&mut self, bounds: &[ast::Bound], context: &Context) -> Result<(Vec<Json>, Context)> {
        let mut child = context.clone();
        let mut bindings = vec![];
        for bound in bounds {
            if child.bound >= 40 {
                return unsupported("more than 40 bound variables");
            }
            let slot = child.bound;
            let mut binding = json!({"slot":slot,"domain":self.domain(&bound.set,context,true)?});
            match &bound.pattern {
                Pattern::Name { name } => {
                    child.order.retain(|item| item != name);
                    child.order.push(name.clone());
                    let binding_id =
                        self.named_binding_at(context.origin, bound.set.location.start, 1, 0, name);
                    child.binding_ids.insert(name.clone(), binding_id);
                    child
                        .projection
                        .insert(binding_id, ir::Expression::Bound { slot });
                    child
                        .locals
                        .insert(name.clone(), ir::Expression::Bound { slot });
                }
                Pattern::Tuple { names } => {
                    binding["tupleSize"] = json!(names.len());
                    for (index, name) in names.iter().enumerate() {
                        child.order.retain(|item| item != name);
                        child.order.push(name.clone());
                        let binding_id = self.named_binding_at(
                            context.origin,
                            bound.set.location.start,
                            1,
                            index,
                            name,
                        );
                        child.binding_ids.insert(name.clone(), binding_id);
                        child.projection.insert(
                            binding_id,
                            ir::Expression::At {
                                target: Box::new(ir::Expression::Bound { slot }),
                                key: Box::new(decode_expression(integer(index as i64 + 1)?)?),
                            },
                        );
                        child.locals.insert(
                            name.clone(),
                            ir::Expression::At {
                                target: Box::new(ir::Expression::Bound { slot }),
                                key: Box::new(decode_expression(integer(index as i64 + 1)?)?),
                            },
                        );
                    }
                }
            }
            child = child.nested()?;
            child.bound = slot + 1;
            bindings.push(binding);
        }
        Ok((bindings, child))
    }
    fn domain(&mut self, value: &Expression, context: &Context, choice: bool) -> Result<Json> {
        if self.version == IrVersion::V2 {
            return Ok(json!(self.domain_ir(value, context, choice)?));
        }
        match &value.node {
            E::SetEnumeration { elements } => {
                Ok(json!({"kind":"set","values":self.expressions(elements,&context.nested()?)?}))
            }
            E::Apply { operator, args } if operator == ".." && args.len() == 2 => {
                if !self.arithmetic() {
                    return unsupported("range requires Naturals");
                }
                let child = context.nested()?;
                Ok(
                    json!({"kind":"range","lower":self.expression(&args[0],&child)?,"upper":self.expression(&args[1],&child)?}),
                )
            }
            E::Identifier { name } if name == "BOOLEAN" => {
                Ok(json!({"kind":"set","values":[boolean(false),boolean(true)]}))
            }
            E::Identifier { name } if name == "Nat" || name == "Int" => {
                if choice || !self.arithmetic() || (name == "Int" && !self.extends("Integers")) {
                    return unsupported("infinite enumeration");
                }
                Ok(
                    json!({"kind":"range","lower":integer(if name == "Nat" {0} else {i32::MIN as i64})?,"upper":integer(i32::MAX as i64)?}),
                )
            }
            E::Identifier { name }
                if self.has(context, name) && !context.locals.contains_key(name) =>
            {
                if self.version == IrVersion::V2 {
                    return Ok(
                        json!({"kind":"value","value":self.shared_call(name, &[], context, false)?}),
                    );
                }
                let (body, child) = self.call(name, &[], context)?;
                self.domain(&body, &child, choice)
            }
            _ => Ok(json!({"kind":"value","value":self.expression(value,context)?})),
        }
    }
    fn expressions(&mut self, values: &[Expression], context: &Context) -> Result<Vec<Json>> {
        values
            .iter()
            .map(|value| self.expression(value, context))
            .collect()
    }
    fn variable_reads(&mut self, value: Json, message: &str) -> Result<Vec<Json>> {
        if self.version == IrVersion::V1 {
            return Ok(vec![value]);
        }
        if self.target_nesting >= 40 {
            return unsupported("expression or operator nesting exceeds 40");
        }
        self.target_nesting += 1;
        self.lower_pending()?;
        struct Frame {
            definition: usize,
            arguments: Vec<Json>,
            captures: Vec<Json>,
            parent: Option<usize>,
        }
        let mut frames: Vec<Frame> = vec![];
        let mut pending: Vec<(Json, Option<usize>)> = vec![(value, None)];
        let mut reads = vec![];
        while let Some((value, frame)) = pending.pop() {
            match value["op"].as_str() {
                Some("tuple") => {
                    pending.extend(
                        value["values"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .rev()
                            .map(|value| (value.clone(), frame)),
                    );
                }
                Some("read") if value["next"] == false => reads.push(value),
                Some("call") => {
                    let id = value["definition"].as_u64().unwrap_or(0) as usize;
                    let mut ancestor = frame;
                    while let Some(index) = ancestor {
                        if frames[index].definition == id {
                            return unsupported("recursive operator definitions");
                        }
                        ancestor = frames[index].parent;
                    }
                    self.lower_definition(id)?;
                    let next = frames.len();
                    frames.push(Frame {
                        definition: id,
                        arguments: value["arguments"].as_array().cloned().unwrap_or_default(),
                        captures: value["captures"].as_array().cloned().unwrap_or_default(),
                        parent: frame,
                    });
                    pending.push((
                        json!(self.shared[id].definition.as_ref().unwrap().body),
                        Some(next),
                    ));
                }
                Some("parameter" | "capture") => {
                    let Some(frame) = frame.and_then(|id| frames.get(id)) else {
                        return unsupported(message);
                    };
                    let slot = value["slot"].as_u64().unwrap_or(0) as usize;
                    let actuals = if value["op"] == "parameter" {
                        &frame.arguments
                    } else {
                        &frame.captures
                    };
                    let Some(actual) = actuals.get(slot) else {
                        return unsupported(message);
                    };
                    pending.push((actual.clone(), frame.parent));
                }
                _ => return unsupported(message),
            }
        }
        self.target_nesting -= 1;
        Ok(reads)
    }
    fn expression(&mut self, value: &Expression, context: &Context) -> Result<Json> {
        if self.version == IrVersion::V2 {
            return Ok(json!(self.expression_ir(value, context)?));
        }
        let child = context.nested()?;
        match &value.node {
            E::Number { value } => integer(i64::try_from(*value).map_err(|_| CompileError {
                message: "integer outside signed 32 bits".into(),
            })?),
            E::String { value } => {
                if value.chars().any(|character| character as u32 > 255) {
                    return unsupported("TLC 1.7.4 string literal outside byte range");
                }
                Ok(string(value))
            }
            E::Identifier { name } => {
                if let Some(local) = context.locals.get(name) {
                    return Ok(json!(local));
                }
                if let Some(constant) = self.constants.get(name) {
                    return literal(constant);
                }
                if name == "TRUE" || name == "FALSE" {
                    return Ok(boolean(name == "TRUE"));
                }
                if name == "BOOLEAN" {
                    return Ok(json!({"op":"set","values":[boolean(false),boolean(true)]}));
                }
                if let Some(slot) = self.slots.get(name) {
                    if context.phase == Phase::Constant {
                        return unsupported("variable in constant expression");
                    }
                    return Ok(
                        json!({"op":"read","slot":slot,"next":context.phase == Phase::Initial}),
                    );
                }
                if self.version == IrVersion::V2 {
                    return self.shared_call(name, &[], context, false);
                }
                let (body, expanded) = self.call(name, &[], context)?;
                self.expression(&body, &expanded)
            }
            E::Let { definitions, body } => {
                let expanded = self.let_context(definitions, context)?;
                self.expression(body, &expanded)
            }
            E::Label { body, .. } => self.expression(body, &child),
            E::Prime { expression } if context.phase == Phase::Next => {
                let mut inner = self.expression(expression, &child)?;
                if inner["op"] == "read" && inner["next"] == false {
                    inner["next"] = json!(true);
                    Ok(inner)
                } else {
                    self.prime(inner)
                }
            }
            E::Unchanged { expression } if context.phase == Phase::Next => {
                let inner = self.expression(expression, &child)?;
                Ok(json!({"op":"eq","left":inner,"right":self.prime(inner.clone())?}))
            }
            E::Enabled { expression } => {
                let next = Context {
                    phase: Phase::Next,
                    ..child
                };
                Ok(json!({"op":"enabled","branches":self.action(expression,&next,"ENABLED")?}))
            }
            E::ExceptAt => Ok(json!(
                context
                    .locals
                    .get("@")
                    .cloned()
                    .unwrap_or(ir::Expression::Old)
            )),
            E::SetEnumeration { elements } | E::Tuple { elements } => Ok(
                json!({"op":if matches!(value.node,E::Tuple{..}) {"tuple"} else {"set"},"values":self.expressions(elements,&child)?}),
            ),
            E::Record { fields } | E::RecordSet { fields } => {
                let fields = fields
                    .iter()
                    .map(|field| Ok(json!([field.name, self.expression(&field.value, &child)?])))
                    .collect::<Result<Vec<_>>>()?;
                Ok(
                    json!({"op":if matches!(value.node,E::Record{..}) {"record"} else {"record-set"},"fields":fields}),
                )
            }
            E::FunctionSet { domain, range } => Ok(
                json!({"op":"function-set","left":self.expression(domain,&child)?,"right":self.expression(range,&child)?}),
            ),
            E::FunctionApplication { function, args } => {
                let key = self.key(args, &child)?;
                Ok(json!({"op":"at","target":self.expression(function,&child)?,"key":key}))
            }
            E::FieldAccess { record, field } => {
                Ok(json!({"op":"at","target":self.expression(record,&child)?,"key":string(field)}))
            }
            E::Except { function, updates } => {
                let mut lowered = vec![];
                for update in updates {
                    let mut path = vec![];
                    for part in &update.path {
                        path.push(match part {
                            ast::ExceptPath::Field { name } => string(name),
                            ast::ExceptPath::Index { args } => self.key(args, &child)?,
                        });
                    }
                    let mut old = child.clone();
                    if self.version == IrVersion::V2 {
                        old.locals.insert("@".into(), ir::Expression::Old);
                        old.order.retain(|name| name != "@");
                        old.order.push("@".into());
                        let binding =
                            self.binding_at(context.origin, update.value.location.start, 2, 0);
                        old.binding_ids.insert("@".into(), binding);
                        old.projection.insert(binding, ir::Expression::Old);
                    }
                    lowered.push(json!({"path":path,"value":self.expression(&update.value,&old)?}));
                }
                Ok(
                    json!({"op":"update","target":self.expression(function,&child)?,"updates":lowered}),
                )
            }
            E::Forall { bounds, body }
            | E::Exists { bounds, body }
            | E::FunctionConstruction { bounds, body } => {
                let (bindings, bound) = self.bind(bounds, &child)?;
                let op = match value.node {
                    E::Forall { .. } => "forall",
                    E::Exists { .. } => "exists",
                    _ => "function",
                };
                Ok(json!({"op":op,"bounds":bindings,"body":self.expression(body,&bound)?}))
            }
            E::SetMap { bounds, expression } => {
                let (bindings, bound) = self.bind(bounds, &child)?;
                Ok(
                    json!({"op":"map-set","bounds":bindings,"body":self.expression(expression,&bound)?}),
                )
            }
            E::SetFilter { bound, predicate } => {
                let (bindings, bound) = self.bind(std::slice::from_ref(bound), &child)?;
                Ok(
                    json!({"op":"filter","bounds":bindings,"body":self.expression(predicate,&bound)?}),
                )
            }
            E::Choose {
                pattern,
                set: Some(set),
                body,
            } => {
                let bound = ast::Bound {
                    pattern: pattern.clone(),
                    set: *set.clone(),
                };
                let (bindings, context) = self.bind(&[bound], &child)?;
                Ok(
                    json!({"op":"choose","binding":bindings[0],"body":self.expression(body,&context)?}),
                )
            }
            E::If {
                condition,
                consequent,
                alternative,
            } => Ok(
                json!({"op":"if","condition":self.expression(condition,&child)?,"consequent":self.expression(consequent,&child)?,"alternative":self.expression(alternative,&child)?}),
            ),
            E::Case { arms, other } => {
                let mut result = if let Some(other) = other {
                    self.expression(other, &child)?
                } else if !arms.is_empty() {
                    unmatched_case(&value.location)
                } else {
                    return unsupported("empty CASE");
                };
                for arm in arms.iter().rev() {
                    result = json!({"op":"if","condition":self.expression(&arm.condition,&child)?,"consequent":self.expression(&arm.body,&child)?,"alternative":result});
                }
                Ok(result)
            }
            E::And { operands, .. } | E::Or { operands, .. } => {
                let and = matches!(value.node, E::And { .. });
                let mut operands = operands.iter();
                let mut result = if self.version == IrVersion::V2 {
                    if let Some(first) = operands.next() {
                        self.expression(first, &child)?
                    } else {
                        boolean(and)
                    }
                } else {
                    boolean(and)
                };
                for operand in operands {
                    result = json!({"op":if and {"and"} else {"or"},"left":result,"right":self.expression(operand,&child)?});
                }
                Ok(result)
            }
            E::BoxAction { action, subscript } | E::AngleAction { action, subscript } => {
                let boxed = matches!(value.node, E::BoxAction { .. });
                let subscript = self.expression(subscript, &child)?;
                let unchanged =
                    json!({"op":"eq","left":subscript,"right":self.prime(subscript.clone())?});
                Ok(
                    json!({"op":if boxed {"or"} else {"and"},"left":self.expression(action,&child)?,"right":if boxed {unchanged} else {json!({"op":"not","argument":unchanged})}}),
                )
            }
            E::Apply { operator, args } => self.apply(operator, args, context, &child),
            _ => unsupported(format!(
                "expression at {}:{}",
                value.location.line, value.location.column
            )),
        }
    }
    fn fold(
        &mut self,
        sequence: bool,
        args: &[Expression],
        context: &Context,
        child: &Context,
    ) -> Result<Json> {
        let initial = self.expression(&args[1], child)?;
        let collection = self.expression(&args[2], child)?;
        let body = self.callback(&args[0], 2, context, child)?;
        Ok(
            json!({"op":"fold", "accumulator":context.bound,"initial":initial,"collection":collection,"order":if sequence { "sequence-left" } else { "set-members" },"body":body}),
        )
    }
    fn callback(
        &mut self,
        argument: &Expression,
        arity: usize,
        context: &Context,
        child: &Context,
    ) -> Result<Json> {
        let E::Identifier { name } = &argument.node else {
            return unsupported("callback operator argument");
        };
        if context.locals.contains_key(name) {
            return unsupported(format!("callback operator {name}"));
        }
        let entry = self.scopes[context.scope]
            .get(name)
            .map(|entry| entry.as_ref().clone())
            .ok_or_else(|| CompileError {
                message: format!("operator {name}"),
            })?;
        if entry.definition.is_recursive
            || if self.version == IrVersion::V1 {
                context.calls.iter().any(|item| item == name)
            } else {
                context.call_ids.contains(&entry.id)
            }
        {
            return unsupported(format!("operator {name}"));
        }
        let DefinitionKind::Operator { params } = &entry.definition.head else {
            return unsupported(format!("callback operator {name}"));
        };
        if params.len() != arity || params.iter().any(|param| param.arity != 0) {
            return unsupported(format!("callback operator {name}"));
        }
        let accumulator = context.bound;
        if accumulator + arity > 40 {
            return unsupported("more than 40 bound variables");
        }
        let mut locals = entry.locals;
        for (index, param) in params.iter().enumerate() {
            locals.insert(
                param.name.clone(),
                ir::Expression::Bound {
                    slot: accumulator + index,
                },
            );
        }
        let mut inner = child.nested()?;
        inner.origin = entry.id;
        inner.locals = locals;
        inner.scope = entry.scope;
        inner.calls.push(name.clone());
        inner.bound = accumulator + arity;
        self.expression(&entry.definition.body, &inner)
    }
    fn key(&mut self, args: &[Expression], context: &Context) -> Result<Json> {
        if args.len() == 1 {
            self.expression(&args[0], context)
        } else {
            Ok(json!({"op":"tuple","values":self.expressions(args,context)?}))
        }
    }

    fn apply(
        &mut self,
        operator: &str,
        args: &[Expression],
        context: &Context,
        child: &Context,
    ) -> Result<Json> {
        if self.has(context, operator) {
            if self.version == IrVersion::V2 {
                return self.shared_call(operator, args, context, false);
            }
            let (body, expanded) = self.call(operator, args, context)?;
            return self.expression(&body, &expanded);
        }
        if (operator == "SelectSeq" && self.extends("Sequences"))
            || (operator == "SortSeq" && self.extends("TLC"))
        {
            if args.len() != 2 {
                return unsupported(format!("operator arity {operator}"));
            }
            let sequence = self.expression(&args[0], child)?;
            let body = self.callback(
                &args[1],
                if operator == "SortSeq" { 2 } else { 1 },
                context,
                child,
            )?;
            return Ok(
                json!({"op":if operator == "SelectSeq" { "select-seq" } else { "sort-seq" },"slot":context.bound,"sequence":sequence,"body":body}),
            );
        }
        if operator == "Assert" && self.extends("TLC") {
            if args.len() != 2 {
                return unsupported("operator arity Assert");
            }
            return Ok(
                json!({"op":"assert","condition":self.expression(&args[0],child)?,"message":self.expression(&args[1],child)?}),
            );
        }
        if self.extends("Apalache") && ["ApaFoldSet", "ApaFoldSeqLeft"].contains(&operator) {
            if args.len() != 3 {
                return unsupported("fold arity");
            }
            return self.fold(operator == "ApaFoldSeqLeft", args, context, child);
        }
        if self.extends("Variants") {
            match operator {
                "Variant" if args.len() == 2 => {
                    if context.bound >= 256 {
                        return unsupported("too many bound variables");
                    }
                    let tag = self.expression(&args[0], child)?;
                    let payload = self.expression(
                        &args[1],
                        &Context {
                            bound: context.bound + 1,
                            ..child.clone()
                        },
                    )?;
                    return Ok(
                        json!({"op":"function","bounds":[{"slot":context.bound,"domain":{"kind":"set","values":[tag]}}],"body":payload}),
                    );
                }
                "VariantTag" if args.len() == 1 => {
                    return Ok(
                        json!({"op":"variant-tag","argument":self.expression(&args[0],child)?}),
                    );
                }
                "VariantGetUnsafe" if args.len() == 2 => {
                    return Ok(
                        json!({"op":"at","target":self.expression(&args[1],child)?,"key":self.expression(&args[0],child)?}),
                    );
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
            return Ok(
                json!({"op":if ["~","\\lnot"].contains(&operator) {"not"} else {"neg"},"argument":self.expression(&args[0],child)?}),
            );
        }
        let unary = match operator {
            "DOMAIN" => Some("domain"),
            "Cardinality" => Some("cardinality"),
            "Len" => Some("len"),
            "Head" => Some("head"),
            "Tail" => Some("tail"),
            "SUBSET" => Some("powerset"),
            "UNION" => Some("union"),
            _ => None,
        };
        if let Some(op) = unary.filter(|_| args.len() == 1) {
            if ["len", "head", "tail"].contains(&op) && !self.extends("Sequences") {
                return unsupported("sequence operator requires Sequences");
            }
            if op == "cardinality" && !self.extends("FiniteSets") {
                return unsupported("Cardinality requires FiniteSets");
            }
            return Ok(json!({"op":op,"argument":self.expression(&args[0],child)?}));
        }
        if args.len() == 2 && ["\\in", "\\notin"].contains(&operator) {
            let left = self.expression(&args[0], child)?;
            let membership = self.membership(left, &args[1], child)?;
            return Ok(if operator == "\\notin" {
                json!({"op":"not","argument":membership})
            } else {
                membership
            });
        }
        if self.version == IrVersion::V2 && operator == "<=>" && args.len() == 2 {
            let left = self.expression(&args[0], child)?;
            let right = self.expression(&args[1], child)?;
            return Ok(
                json!({"op":"eq","left":{"op":"not","argument":left},"right":{"op":"not","argument":right}}),
            );
        }
        if operator == ".." && args.len() == 2 {
            let value = Expression {
                node: E::Apply {
                    operator: operator.into(),
                    args: args.to_vec(),
                },
                location: args[0].location,
            };
            return Ok(
                json!({"op":"map-set","bounds":[{"slot":context.bound,"domain":self.domain(&value,child,true)?}],"body":{"op":"bound","slot":context.bound}}),
            );
        }
        if operator == "Append" && args.len() == 2 && self.extends("Sequences") {
            return Ok(
                json!({"op":"append","left":self.expression(&args[0],child)?,"right":self.expression(&args[1],child)?}),
            );
        }
        if operator == "SubSeq" && args.len() == 3 && self.extends("Sequences") {
            return Ok(
                json!({"op":"subseq","sequence":self.expression(&args[0],child)?,"from":self.expression(&args[1],child)?,"to":self.expression(&args[2],child)?}),
            );
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
        Ok(
            json!({"op":op,"left":self.expression(&args[0],child)?,"right":self.expression(&args[1],child)?}),
        )
    }

    fn membership_set(&mut self, value: &Expression, context: &Context) -> Result<Json> {
        if self.version == IrVersion::V2 {
            return Ok(json!(self.membership_ir(value, context)?));
        }
        let child = context.nested()?;
        match &value.node {
            E::Let { definitions, body } if self.version == IrVersion::V2 => {
                let inner = self.let_context(definitions, context)?;
                self.membership_set(body, &inner)
            }
            E::Label { body, .. } if self.version == IrVersion::V2 => {
                self.membership_set(body, &child)
            }
            E::If {
                condition,
                consequent,
                alternative,
            } if self.version == IrVersion::V2 => Ok(
                json!({"op":"if","condition":self.expression(condition,&child)?,"consequent":self.membership_set(consequent,&child)?,"alternative":self.membership_set(alternative,&child)?}),
            ),
            E::Case { arms, other } if self.version == IrVersion::V2 && !arms.is_empty() => {
                let mut result = if let Some(other) = other {
                    self.membership_set(other, &child)?
                } else {
                    unmatched_case(&value.location)
                };
                for arm in arms.iter().rev() {
                    result = json!({"op":"if","condition":self.expression(&arm.condition,&child)?,"consequent":self.membership_set(&arm.body,&child)?,"alternative":result});
                }
                Ok(result)
            }
            E::Identifier { name }
                if self.version == IrVersion::V2 && (name == "Nat" || name == "Int") =>
            {
                if !self.arithmetic() || name == "Int" && !self.extends("Integers") {
                    return unsupported("infinite enumeration");
                }
                Ok(
                    json!({"op":"map-set","bounds":[{"slot":context.bound,"domain":{"kind":"range","lower":integer(if name == "Nat" { 0 } else { i32::MIN as i64 })?,"upper":integer(i32::MAX as i64)?}}],"body":{"op":"bound","slot":context.bound}}),
                )
            }

            E::Identifier { name }
                if self.has(context, name) && !context.locals.contains_key(name) =>
            {
                if self.version == IrVersion::V2 {
                    return self.shared_call(name, &[], context, true);
                }
                let (body, expanded) = self.call(name, &[], context)?;
                self.membership_set(&body, &expanded)
            }
            E::Apply { operator, args }
                if self.version == IrVersion::V2 && self.has(context, operator) =>
            {
                self.shared_call(operator, args, context, true)
            }
            E::Apply { operator, args }
                if operator == "Seq" && args.len() == 1 && self.extends("Sequences") =>
            {
                Ok(json!({"op":"sequence-set","argument":self.membership_set(&args[0],&child)?}))
            }
            E::FunctionSet { domain, range } => Ok(
                json!({"op":"function-set","left":self.expression(domain,&child)?,"right":self.membership_set(range,&child)?}),
            ),
            E::RecordSet { fields } => {
                let fields = fields
                    .iter()
                    .map(|field| {
                        Ok(json!([
                            field.name,
                            self.membership_set(&field.value, &child)?
                        ]))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(json!({"op":"record-set","fields":fields}))
            }
            _ => self.expression(value, context),
        }
    }
    fn membership(&mut self, left: Json, value: &Expression, context: &Context) -> Result<Json> {
        if self.version == IrVersion::V2
            && match &value.node {
                E::Identifier { name } => {
                    self.has(context, name) || context.locals.contains_key(name)
                }
                E::Apply { operator, .. } => self.has(context, operator),
                _ => false,
            }
        {
            return Ok(
                json!({"op":"function-in","left":left,"right":self.membership_set(value, context)?}),
            );
        }
        match &value.node {
            E::Identifier { name }
                if self.has(context, name) && !context.locals.contains_key(name) =>
            {
                let (body, expanded) = self.call(name, &[], context)?;
                self.membership(left, &body, &expanded)
            }
            E::Apply { operator, args }
                if operator == "Seq" && args.len() == 1 && self.extends("Sequences") =>
            {
                Ok(
                    json!({"op":"sequence-in","left":left,"right":self.membership_set(&args[0],&context.nested()?)?}),
                )
            }
            E::RecordSet { .. } | E::FunctionSet { .. } => Ok(
                json!({"op":"function-in","left":left,"right":self.membership_set(value,&context.nested()?)?}),
            ),
            E::Apply { operator, args } if operator == "SUBSET" && args.len() == 1 => Ok(
                json!({"op":"function-in","left":left,"right":{"op":"powerset","argument":self.expression(&args[0],context)?}}),
            ),
            _ => Ok(json!({"op":"in","value":left,"domain":self.domain(value,context,false)?})),
        }
    }
    fn target(&self, value: &Expression, context: &Context) -> Option<usize> {
        match &value.node {
            E::Identifier { name }
                if context.phase == Phase::Initial && !context.locals.contains_key(name) =>
            {
                self.slots.get(name).copied()
            }
            E::Prime { expression: _ }
                if self.version == IrVersion::V2 && context.phase == Phase::Next =>
            {
                analysis::analysis_children(analysis::Node::Source(value))
                    .into_iter()
                    .filter(|edge| edge.descriptor.role == "prime-argument")
                    .find_map(|edge| match edge.node {
                        analysis::Node::Source(Expression {
                            node: E::Identifier { name },
                            location: _,
                        }) if !context.locals.contains_key(name) => self.slots.get(name).copied(),
                        _ => None,
                    })
            }
            E::Prime { expression } if context.phase == Phase::Next => match &expression.node {
                E::Identifier { name } if !context.locals.contains_key(name) => {
                    self.slots.get(name).copied()
                }
                _ => None,
            },
            _ => None,
        }
    }
    fn generative_call(
        &mut self,
        name: &str,
        context: &Context,
        active: &mut HashSet<usize>,
    ) -> Result<bool> {
        enum Task {
            Call(String, Context),
            Expression(Expression, Context),
            Finish(usize, Option<(usize, u8)>),
        }
        let mut tasks = vec![Task::Call(name.into(), context.clone())];
        let mut results = vec![];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Finish(count, key) => {
                    let result = results.drain(results.len() - count..).any(|value| value);
                    if let Some((position, phase)) = key {
                        active.remove(&position);
                        if result || !self.recursive_ids.contains(&position) {
                            self.generative_cache.insert((position, phase), result);
                        }
                    }
                    results.push(result);
                }
                Task::Call(name, context) => {
                    let entry =
                        self.scopes[context.scope]
                            .get(&name)
                            .cloned()
                            .ok_or_else(|| CompileError {
                                message: format!("operator {name}"),
                            })?;
                    let position = entry.id;
                    let phase = match context.phase {
                        Phase::Constant => 0,
                        Phase::Initial => 1,
                        Phase::State => 2,
                        Phase::Next => 3,
                    };
                    if active.contains(&position) {
                        if entry.definition.is_recursive {
                            results.push(false);
                            continue;
                        }
                        return unsupported(format!("operator {name}"));
                    }
                    if let Some(result) = self.generative_cache.get(&(position, phase)) {
                        results.push(*result);
                        continue;
                    }
                    let DefinitionKind::Operator { params } = &entry.definition.head else {
                        self.generative_cache.insert((position, phase), false);
                        results.push(false);
                        continue;
                    };
                    active.insert(position);
                    let mut child = context;
                    child.scope = entry.scope;
                    child.locals = entry.locals.clone();
                    for parameter in params {
                        child
                            .locals
                            .insert(parameter.name.clone(), decode_expression(boolean(false))?);
                    }
                    tasks.push(Task::Finish(1, Some((position, phase))));
                    tasks.push(Task::Expression(entry.definition.body.clone(), child));
                }
                Task::Expression(value, context) => {
                    let children = match &value.node {
                        E::Identifier { name }
                            if self.has(&context, name) && !context.locals.contains_key(name) =>
                        {
                            tasks.push(Task::Call(name.clone(), context));
                            continue;
                        }
                        E::Apply { operator, .. } if self.has(&context, operator) => {
                            tasks.push(Task::Call(operator.clone(), context));
                            continue;
                        }
                        E::Apply { operator, args } if args.len() == 2 => {
                            let equality =
                                operator == "=" || operator == ":=" && self.extends("Apalache");
                            let generative = if self.version == IrVersion::V2 {
                                (equality || operator == "\\in")
                                    && analysis::analysis_children(analysis::Node::Source(&value))
                                        .into_iter()
                                        .filter(|edge| {
                                            equality
                                                || edge.descriptor.action == "target-if-identifier"
                                        })
                                        .any(|edge| match edge.node {
                                            analysis::Node::Source(candidate) => {
                                                self.target(candidate, &context).is_some()
                                            }
                                            _ => false,
                                        })
                            } else {
                                (equality || operator == "\\in")
                                    && self.target(&args[0], &context).is_some()
                                    || equality && self.target(&args[1], &context).is_some()
                            };
                            results.push(generative);
                            continue;
                        }
                        E::Unchanged { .. } if context.phase == Phase::Next => {
                            results.push(true);
                            continue;
                        }
                        E::BoxAction { .. } | E::AngleAction { .. }
                            if self.version == IrVersion::V2 && context.phase == Phase::Next =>
                        {
                            results.push(true);
                            continue;
                        }
                        _ => self
                            .requirement_children(&value, &context)?
                            .into_iter()
                            .filter(|(_, _, descriptor)| descriptor.action == "inherit")
                            .map(|(value, context, _)| (value, context))
                            .collect::<Vec<_>>(),
                    };
                    tasks.push(Task::Finish(children.len(), None));
                    tasks.extend(
                        children
                            .into_iter()
                            .rev()
                            .map(|(value, context)| Task::Expression(value, context)),
                    );
                }
            }
        }
        Ok(results.pop().unwrap_or(false))
    }
    fn action(&mut self, value: &Expression, context: &Context, name: &str) -> Result<Vec<Json>> {
        if self.version == IrVersion::V2 {
            return Ok(self
                .action_ir(value, context, name)?
                .into_iter()
                .map(|value| json!(value))
                .collect());
        }
        let child = context.nested()?;
        match &value.node {
            E::BoxAction { action, subscript } | E::AngleAction { action, subscript }
                if self.version == IrVersion::V2 && context.phase == Phase::Next =>
            {
                let target = self.resolve_target(subscript, context)?;
                let subscript = Self::target_json(&target)?;
                let mut branches = self.action(action, &child, name)?;
                if matches!(value.node, E::BoxAction { .. }) {
                    let reads = self.variable_reads(
                        subscript,
                        "generative UNCHANGED requires state variables",
                    )?;
                    let mut steps = vec![];
                    for read in reads {
                        steps.extend(unchanged(read)?);
                    }
                    branches.push(json!({"name":name,"steps":steps}));
                } else {
                    let next = self.prime(subscript.clone())?;
                    let guard = json!({"kind":"guard","expression":{"op":"ne","left":subscript,"right":next}});
                    for branch in &mut branches {
                        let mut joined = steps(branch)?.clone();
                        joined.push(guard.clone());
                        branch["steps"] = json!(joined);
                    }
                }
                limited(branches)
            }
            E::Identifier { name } if self.has(context, name) => {
                if self.version == IrVersion::V2
                    && !self.generative_call(name, context, &mut HashSet::new())?
                {
                    return Ok(vec![
                        json!({"name":name,"steps":[{"kind":"guard","expression":self.shared_call(name,&[],context,false)?}]} ),
                    ]);
                }
                let (body, expanded) = self.call(name, &[], context)?;
                let branches = self.action(&body, &expanded, name)?;
                Ok(branches)
            }
            E::Apply { operator, args } if self.has(context, operator) => {
                if self.version == IrVersion::V2
                    && !self.generative_call(operator, context, &mut HashSet::new())?
                {
                    return Ok(vec![
                        json!({"name":operator,"steps":[{"kind":"guard","expression":self.shared_call(operator,args,context,false)?}]} ),
                    ]);
                }
                let (body, expanded) = self.call(operator, args, context)?;
                let branches = self.action(&body, &expanded, operator)?;
                Ok(branches)
            }
            E::Let { definitions, body } => {
                let expanded = self.let_context(definitions, context)?;
                self.action(body, &expanded, name)
            }
            E::Label { body, .. } => self.action(body, &child, name),
            E::Exists { bounds, body } => {
                let (bindings, bound) = self.bind(bounds, &child)?;
                let mut prefix = vec![];
                for mut binding in bindings {
                    binding["kind"] = json!("bind");
                    prefix.push(binding);
                }
                let mut branches = self.action(body, &bound, name)?;
                for branch in &mut branches {
                    let mut joined = prefix.clone();
                    joined.extend(steps(branch)?.clone());
                    branch["steps"] = json!(joined);
                }
                limited(branches)
            }
            E::Or { operands, .. } => {
                let mut result = vec![];
                for operand in operands {
                    result.extend(self.action(operand, &child, name)?);
                    result = limited(result)?;
                }
                Ok(result)
            }
            E::And { operands, .. } => {
                let mut result = vec![json!({"name":name,"steps":[]})];
                for operand in operands {
                    let mut next = vec![];
                    for branch in &result {
                        let bound = context.bound
                            + steps(branch)?
                                .iter()
                                .filter(|step| step["kind"] == "bind")
                                .count();
                        for mut addition in self.action(
                            operand,
                            &Context {
                                bound,
                                ..child.clone()
                            },
                            name,
                        )? {
                            let mut joined = steps(branch)?.clone();
                            joined.extend(steps(&addition)?.clone());
                            addition["steps"] = json!(joined);
                            next.push(addition);
                            if next.len() > 4096 {
                                return unsupported("action expansion exceeds 4096 branches");
                            }
                        }
                    }
                    result = limited(next)?;
                }
                Ok(result)
            }
            E::If {
                condition,
                consequent,
                alternative,
            } => {
                let condition = self.expression(condition, &child)?;
                let mut result = vec![];
                for (body, guard) in [
                    (consequent, condition.clone()),
                    (alternative, json!({"op":"not","argument":condition})),
                ] {
                    for mut branch in self.action(body, &child, name)? {
                        let mut joined = vec![json!({"kind":"guard","expression":guard})];
                        joined.extend(steps(&branch)?.clone());
                        branch["steps"] = json!(joined);
                        result.push(branch);
                    }
                }
                limited(result)
            }
            E::Case { arms, other } if !arms.is_empty() => {
                let mut result = vec![];
                let mut previous: Vec<Json> = vec![];
                for arm in arms {
                    let condition = self.expression(&arm.condition, &child)?;
                    let mut guards: Vec<Json> = previous
                        .iter()
                        .map(|earlier| json!({"op":"not","argument":earlier}))
                        .collect();
                    guards.push(condition.clone());
                    for mut branch in self.action(&arm.body, &child, name)? {
                        let mut joined: Vec<Json> = guards
                            .iter()
                            .map(|guard| json!({"kind":"guard","expression":guard}))
                            .collect();
                        joined.extend(steps(&branch)?.clone());
                        branch["steps"] = json!(joined);
                        result.push(branch);
                    }
                    previous.push(condition);
                    result = limited(result)?;
                }
                let guards: Vec<Json> = previous
                    .iter()
                    .map(|earlier| json!({"kind":"guard","expression":{"op":"not","argument":earlier}}))
                    .collect();
                if let Some(other) = other {
                    for mut branch in self.action(other, &child, name)? {
                        let mut joined = guards.clone();
                        joined.extend(steps(&branch)?.clone());
                        branch["steps"] = json!(joined);
                        result.push(branch);
                    }
                } else {
                    let mut joined = guards;
                    joined
                        .push(json!({"kind":"guard","expression":unmatched_case(&value.location)}));
                    result.push(json!({"name":name,"steps":joined}));
                }
                limited(result)
            }
            E::Unchanged { expression } if context.phase == Phase::Next => {
                let value = if self.version == IrVersion::V2 {
                    Self::target_json(&self.resolve_target(expression, &child)?)?
                } else {
                    self.expression(expression, &child)?
                };
                let values =
                    self.variable_reads(value, "generative UNCHANGED requires state variables")?;
                let mut steps = vec![];
                for value in values {
                    steps.extend(unchanged(value)?);
                }
                Ok(vec![json!({"name":name,"steps":steps})])
            }
            E::Apply { operator, args } if args.len() == 2 => {
                let equality = operator == "=" || operator == ":=" && self.extends("Apalache");
                if let Some(slot) = self.target(&args[0], context) {
                    if equality {
                        return Ok(vec![
                            json!({"name":name,"steps":[{"kind":"assign","slot":slot,"value":self.expression(&args[1],&child)?}]}),
                        ]);
                    }
                    if operator == "\\in" {
                        return Ok(vec![
                            json!({"name":name,"steps":[{"kind":"choose","slot":slot,"domain":self.domain(&args[1],&child,true)?}]}),
                        ]);
                    }
                }
                if equality && let Some(slot) = self.target(&args[1], context) {
                    return Ok(vec![
                        json!({"name":name,"steps":[{"kind":"assign","slot":slot,"value":self.expression(&args[0],&child)?}]}),
                    ]);
                }
                Ok(vec![
                    json!({"name":name,"steps":[{"kind":"guard","expression":self.expression(value,context)?}]}),
                ])
            }
            _ => Ok(vec![
                json!({"name":name,"steps":[{"kind":"guard","expression":self.expression(value,context)?}]}),
            ]),
        }
    }
    fn boxed_action(&mut self, value: &Expression, context: &Context) -> Result<bool> {
        match &value.node {
            E::Identifier { name } if context.locals.contains_key(name) => {
                if let Some((actual, caller, _)) = context
                    .binding_ids
                    .get(name)
                    .and_then(|binding| context.target_actuals.get(binding))
                    .cloned()
                {
                    self.boxed_action(&actual, &caller)
                } else {
                    Ok(false)
                }
            }

            E::Identifier { name }
                if self.has(context, name) && !context.locals.contains_key(name) =>
            {
                let (body, expanded) = self.call(name, &[], context)?;
                self.boxed_action(&body, &expanded)
            }
            E::Apply { operator, args: _ } if self.has(context, operator) => {
                let (body, expanded) = self.call(operator, &source_children(value), context)?;
                self.boxed_action(&body, &expanded)
            }
            E::BoxAction { .. } => Ok(true),
            _ => Ok(false),
        }
    }
    fn temporal(&mut self, value: &Expression, context: &Context) -> Result<Json> {
        if self.version == IrVersion::V2 {
            return Ok(json!(self.temporal_ir(value, context)?));
        }
        let child = context.nested()?;
        match &value.node {
            E::Forall { bounds, body } | E::Exists { bounds, body } => {
                let (bindings, context) = self.bind(bounds, &child)?;
                let formula = self.temporal(body, &context)?;
                if formula["kind"] == "atom" {
                    return Ok(atom(self.expression(value, &child)?));
                }
                self.lower_pending()?;
                if bindings
                    .iter()
                    .any(|binding| self.level(&binding["domain"]) > 0)
                {
                    return unsupported("temporal quantification requires constant finite domains");
                }
                Ok(
                    json!({"kind":if matches!(value.node, E::Forall { .. }) {"forall"} else {"exists"},"bounds":bindings,"formula":formula}),
                )
            }
            E::Identifier { name } if self.has(context, name) => {
                let (body, expanded) = self.call(name, &[], context)?;
                let formula = self.temporal(&body, &expanded)?;
                Ok(formula)
            }
            E::Apply { operator, args } if self.has(context, operator) => {
                let (body, expanded) = self.call(operator, args, context)?;
                let formula = self.temporal(&body, &expanded)?;
                Ok(formula)
            }
            E::Let { definitions, body } => {
                let expanded = self.let_context(definitions, context)?;
                self.temporal(body, &expanded)
            }
            E::Always { expression } | E::Eventually { expression } => {
                let formula = self.temporal(expression, &child)?;
                self.lower_pending()?;
                let always = matches!(value.node, E::Always { .. });
                if always
                    && atom_expression(&formula)
                        .is_some_and(|expression| self.level(&expression) > 1)
                    && !self.boxed_action(expression, &child)?
                {
                    return unsupported("[] action must have the form [A]_v");
                }
                Ok(json!({"kind":if always {"always"} else {"eventually"},"formula":formula}))
            }
            E::And { operands, .. } | E::Or { operands, .. } => {
                let and = matches!(value.node, E::And { .. });
                let mut result = atom(boolean(and));
                for operand in operands {
                    result = combine(
                        if and { "and" } else { "or" },
                        result,
                        self.temporal(operand, &child)?,
                    );
                }
                Ok(result)
            }
            E::Apply { operator, args }
                if args.len() == 1 && ["~", "\\lnot"].contains(&operator.as_str()) =>
            {
                Ok(negate(self.temporal(&args[0], &child)?))
            }
            E::Apply { operator, args }
                if args.len() == 2 && ["=>", "~>"].contains(&operator.as_str()) =>
            {
                let right = self.temporal(&args[1], &child)?;
                let result = combine(
                    "or",
                    negate(self.temporal(&args[0], &child)?),
                    if operator == "~>" {
                        json!({"kind":"eventually","formula":right})
                    } else {
                        right
                    },
                );
                Ok(if operator == "~>" {
                    json!({"kind":"always","formula":result})
                } else {
                    result
                })
            }
            E::WeakFairness { subscript, action } | E::StrongFairness { subscript, action } => {
                let subscript = self.expression(subscript, &child)?;
                let enabled = atom(
                    json!({"op":"enabled","branches":self.action(action,&child,"fairness")?,"subscript":subscript}),
                );
                let disabled = json!({"kind":"not","formula":enabled});
                let taken = atom(
                    json!({"op":"and","left":self.expression(action,&child)?,"right":{"op":"ne","left":subscript,"right":self.prime(subscript.clone())?}}),
                );
                Ok(if matches!(value.node, E::WeakFairness { .. }) {
                    json!({"kind":"always","formula":{"kind":"eventually","formula":{"kind":"or","left":disabled,"right":taken}}})
                } else {
                    json!({"kind":"or","left":{"kind":"eventually","formula":{"kind":"always","formula":disabled}},"right":{"kind":"always","formula":{"kind":"eventually","formula":taken}}})
                })
            }
            _ => Ok(atom(self.expression(value, context)?)),
        }
    }
    fn parts(
        &mut self,
        value: &Expression,
        context: &Context,
        initial: &mut Option<Vec<Json>>,
        next: &mut Option<Vec<Json>>,
        fairness: &mut Vec<Json>,
    ) -> Result<()> {
        if matches!(value.node, E::Forall { .. } | E::Exists { .. }) {
            let formula = self.temporal(value, context)?;
            if formula["kind"] != "atom" {
                fairness.push(formula);
                return Ok(());
            }
        }
        match &value.node {
            E::Identifier { name } if self.has(context, name) => {
                let (body, expanded) = self.call(name, &[], context)?;
                self.parts(&body, &expanded, initial, next, fairness)
            }
            E::Apply { operator, args } if self.has(context, operator) => {
                for argument in args {
                    if level(&self.expression(argument, context)?) > 0 {
                        return unsupported("SPECIFICATION operator arguments must be constant");
                    }
                }
                let (body, expanded) = self.call(operator, args, context)?;
                self.parts(&body, &expanded, initial, next, fairness)
            }
            E::Let { definitions, body } => {
                let expanded = self.let_context(definitions, context)?;
                self.parts(body, &expanded, initial, next, fairness)
            }
            E::And { operands, .. } => {
                for operand in operands {
                    self.parts(operand, &context.nested()?, initial, next, fairness)?;
                }
                Ok(())
            }
            E::Always { expression } if matches!(expression.node, E::BoxAction { .. }) => {
                if next.is_some() {
                    return unsupported("multiple NEXT conjuncts");
                }
                if let E::BoxAction {
                    action: _,
                    subscript: _,
                } = &expression.node
                {
                    let children = analysis::analysis_children(analysis::Node::Source(expression));
                    let mut action = None;
                    let mut subscript = None;
                    for edge in children {
                        if let analysis::Node::Source(child) = edge.node {
                            match edge.descriptor.role {
                                "action-body" => action = Some(child),
                                "subscript" => subscript = Some(child),
                                _ => {}
                            }
                        }
                    }
                    let action = action.ok_or_else(|| CompileError {
                        message: "SPECIFICATION action".into(),
                    })?;
                    let subscript = self.expression(
                        subscript.ok_or_else(|| CompileError {
                            message: "SPECIFICATION subscript".into(),
                        })?,
                        context,
                    )?;
                    let mut reads = HashSet::new();
                    for value in self.variable_reads(
                        subscript,
                        "SPECIFICATION subscript must list state variables",
                    )? {
                        read_slots(&value, &mut reads)?;
                    }
                    if reads.len() != self.slots.len() {
                        return unsupported(
                            "SPECIFICATION subscript must cover every state variable",
                        );
                    }
                    *next = Some(self.action(action, context, "Next")?);
                }
                Ok(())
            }
            E::WeakFairness { .. } | E::StrongFairness { .. } => {
                fairness.push(self.temporal(value, context)?);
                Ok(())
            }
            _ => {
                let additions = self.action(
                    value,
                    &Context {
                        phase: Phase::Initial,
                        ..context.clone()
                    },
                    "Init",
                )?;
                *initial = Some(if let Some(previous) = initial.take() {
                    let mut result = vec![];
                    for branch in &previous {
                        for addition in &additions {
                            let mut joined = steps(branch)?.clone();
                            joined.extend(steps(addition)?.clone());
                            result.push(json!({"name":addition["name"],"steps":joined}));
                            if result.len() > 4096 {
                                return unsupported("action expansion exceeds 4096 branches");
                            }
                        }
                    }
                    limited(result)?
                } else {
                    additions
                });
                Ok(())
            }
        }
    }
    fn operator(&mut self, name: &str, phase: Phase) -> Result<(Expression, Context)> {
        self.call(name, &[], &self.root(phase))
    }
    fn program(&mut self) -> Result<ir::SafetyProgram> {
        if self.version == IrVersion::V2 {
            return self.program_ir();
        }
        let mut initial = None;
        let mut next = None;
        let mut fairness = vec![];
        if let Some(name) = self.config.specification.clone() {
            let (body, context) = self.operator(&name, Phase::Next)?;
            self.parts(&body, &context, &mut initial, &mut next, &mut fairness)?;
        } else {
            for (phase, name) in [
                (Phase::Initial, self.config.init.clone()),
                (Phase::Next, self.config.next.clone()),
            ] {
                let name = name.ok_or_else(|| CompileError {
                    message: "INIT / NEXT".into(),
                })?;
                let (body, context) = self.operator(&name, phase)?;
                let branches = self.action(&body, &context, &name)?;
                if phase == Phase::Initial {
                    initial = Some(branches);
                } else {
                    next = Some(branches);
                }
            }
        }
        let initial = initial.ok_or_else(|| CompileError {
            message: "initial predicate required".into(),
        })?;
        let next = next.ok_or_else(|| CompileError {
            message: "NEXT required".into(),
        })?;
        self.lower_pending()?;
        if !next.iter().any(|branch| {
            steps(branch).is_ok_and(|steps| {
                steps
                    .iter()
                    .any(|step| step["kind"] != "guard" || self.level(&step["expression"]) > 0)
            })
        }) {
            return unsupported("NEXT cannot be constant");
        }
        let mut invariants = vec![];
        for name in self.config.invariants.clone() {
            let expression = if self.version == IrVersion::V2 {
                self.shared_call(&name, &[], &self.root(Phase::State), false)?
            } else {
                let (body, context) = self.operator(&name, Phase::State)?;
                self.expression(&body, &context)?
            };
            invariants.push(json!({"name":name,"expression":expression}));
        }
        let mut initial_properties = vec![];
        let mut action_properties = vec![];
        let mut properties = vec![];
        for name in self.config.properties.clone() {
            let (body, context) = self.operator(&name, Phase::Next)?;
            let formula = self.temporal(&body, &context)?;
            self.lower_pending()?;
            let initial_expression = atom_expression(&formula);
            let always_expression = always_expression(&formula);
            self.validate_temporal(&formula, &[], always_expression.is_some())?;
            if let Some(expression) = initial_expression {
                if self.level(&expression) > 1 {
                    return unsupported("initial PROPERTY must be a state predicate");
                }
                initial_properties.push(json!({"name":name,"expression":expression}));
            } else if let Some(expression) = always_expression {
                let target = if self.level(&expression) > 1 {
                    &mut action_properties
                } else {
                    &mut invariants
                };
                target.push(json!({"name":name,"expression":expression}));
            } else {
                properties.push(json!({"name":name,"formula":formula}));
            }
        }
        let mut assumptions = vec![];
        for (index, assumption) in self.module.assumptions.iter().enumerate() {
            assumptions.push(json!({"name":assumption.name.clone().unwrap_or_else(||format!("ASSUME {}",index+1)),"expression":self.expression(&assumption.expression,&self.root(Phase::Constant))?}));
        }
        let mut result = json!({"format":"modelt.safety-program","version":1,"name":self.module.name,"variables":self.module.variables.iter().map(|value|&value.name).collect::<Vec<_>>(),"initial":initial,"next":next,"invariants":invariants,"assumptions":assumptions,"checkDeadlock":self.config.check_deadlock});
        if let Some(symmetry) = self.node_symmetry()? {
            result["symmetry"] = symmetry;
        }
        for (key, names, phase) in [
            ("constraints", self.config.constraints.clone(), Phase::State),
            (
                "actionConstraints",
                self.config.action_constraints.clone(),
                Phase::Next,
            ),
        ] {
            if names.is_empty() {
                continue;
            }
            let mut predicates = vec![];
            for name in names {
                let expression = if self.version == IrVersion::V2 {
                    self.shared_call(&name, &[], &self.root(phase), false)?
                } else {
                    let (body, context) = self.operator(&name, phase)?;
                    self.expression(&body, &context)?
                };
                predicates.push(json!({"name":name,"expression":expression}));
            }
            result[key] = json!(predicates);
        }
        if !initial_properties.is_empty() {
            result["initialProperties"] = json!(initial_properties);
        }
        if !action_properties.is_empty() {
            result["actionProperties"] = json!(action_properties);
        }
        if !properties.is_empty() {
            result["properties"] = json!(properties);
            result["fairness"] = json!(fairness);
        }
        serde_json::from_value(result).map_err(|error| CompileError {
            message: format!("invalid generated IR: {error}"),
        })
    }
}
fn read_slots(value: &Json, slots: &mut HashSet<u64>) -> Result<()> {
    if value["op"] == "tuple" {
        for inner in value["values"].as_array().into_iter().flatten() {
            read_slots(inner, slots)?;
        }
        return Ok(());
    }
    if value["op"] != "read" || value["next"] != false {
        return unsupported("SPECIFICATION subscript must list state variables");
    }
    if let Some(slot) = value["slot"].as_u64() {
        slots.insert(slot);
    }
    Ok(())
}
fn validate_temporal(formula: &Json, path: &[String], direct: bool) -> Result<()> {
    match formula["kind"].as_str() {
        Some("atom") => {
            let tail = path
                .iter()
                .rev()
                .take(2)
                .map(String::as_str)
                .collect::<Vec<_>>();
            if level(&formula["expression"]) > 1
                && !(direct && path.len() == 1)
                && tail != ["eventually", "always"]
                && tail != ["always", "eventually"]
            {
                return unsupported("temporal actions require []<>A or <>[]A");
            }
            Ok(())
        }
        Some("and" | "or") => {
            validate_temporal(&formula["left"], path, direct)?;
            validate_temporal(&formula["right"], path, direct)
        }
        Some(kind) => {
            let mut path = path.to_vec();
            if !["not", "forall", "exists"].contains(&kind) {
                path.push(kind.into());
            }
            validate_temporal(&formula["formula"], &path, direct)
        }
        None => unsupported("invalid temporal formula"),
    }
}

fn expression_frame(frame: &Json, body: Json) -> Json {
    let mut frame = frame.clone();
    frame.as_object_mut().unwrap().remove("kind");
    frame.as_object_mut().unwrap().remove("formula");
    frame["op"] = json!("frame");
    frame["body"] = body;
    frame
}
fn atom_expression(formula: &Json) -> Option<Json> {
    match formula["kind"].as_str() {
        Some("atom") => Some(formula["expression"].clone()),
        Some("frame") => {
            atom_expression(&formula["formula"]).map(|body| expression_frame(formula, body))
        }
        _ => None,
    }
}
fn always_expression(formula: &Json) -> Option<Json> {
    match formula["kind"].as_str() {
        Some("always") => atom_expression(&formula["formula"]),
        Some("frame") => {
            always_expression(&formula["formula"]).map(|body| expression_frame(formula, body))
        }
        _ => None,
    }
}
fn free_old(value: &Expression) -> bool {
    let mut pending = vec![(analysis::Node::Source(value), false)];
    while let Some((node, old)) = pending.pop() {
        if matches!(
            node,
            analysis::Node::Source(Expression {
                node: E::ExceptAt,
                location: _
            })
        ) && !old
        {
            return true;
        }
        pending.extend(
            analysis::analysis_children(node)
                .into_iter()
                .map(|edge| (edge.node, old || edge.descriptor.scope == "old")),
        );
    }
    false
}
