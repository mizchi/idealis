use crate::{CoreError, Value};
use serde::{Deserialize, Serialize};

pub const WIRE_INDEX_MAX: u64 = 4_294_967_295;

pub(crate) fn validate_wire_index(value: usize) -> Result<(), CoreError> {
    if value as u64 > WIRE_INDEX_MAX {
        return Err(CoreError::protocol("Invalid compiled program count"));
    }
    Ok(())
}

fn deserialize_wire_index<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<usize, D::Error> {
    let value = u64::deserialize(deserializer)?;
    if value > WIRE_INDEX_MAX {
        return Err(serde::de::Error::custom("Invalid compiled program count"));
    }
    usize::try_from(value).map_err(serde::de::Error::custom)
}

fn deserialize_optional_wire_index<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<usize>, D::Error> {
    deserialize_wire_index(deserializer).map(Some)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Expression {
    Frame {
        arguments: Vec<Expression>,
        captures: Vec<Expression>,
        #[serde(rename = "parameterCount")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        parameter_count: usize,
        #[serde(rename = "captureCount")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        capture_count: usize,
        #[serde(rename = "localScope")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        local_scope: usize,
        body: Box<Self>,
    },
    Call {
        #[serde(deserialize_with = "deserialize_wire_index")]
        definition: usize,
        arguments: Vec<Self>,
        captures: Vec<Self>,
    },
    Parameter {
        #[serde(deserialize_with = "deserialize_wire_index")]
        slot: usize,
    },
    Capture {
        #[serde(deserialize_with = "deserialize_wire_index")]
        slot: usize,
    },
    Prime {
        argument: Box<Self>,
    },
    Data {
        value: Value,
    },
    Bound {
        #[serde(deserialize_with = "deserialize_wire_index")]
        slot: usize,
    },
    Old,
    UnmatchedCase {
        location: String,
    },
    Set {
        values: Vec<Self>,
    },
    Tuple {
        values: Vec<Self>,
    },
    Record {
        fields: Vec<(String, Self)>,
    },
    RecordSet {
        fields: Vec<(String, Self)>,
    },
    At {
        target: Box<Self>,
        key: Box<Self>,
    },
    Update {
        target: Box<Self>,
        updates: Vec<Update>,
    },
    Forall {
        bounds: Vec<Binding>,
        body: Box<Self>,
    },
    Exists {
        bounds: Vec<Binding>,
        body: Box<Self>,
    },
    Choose {
        binding: Binding,
        body: Box<Self>,
    },
    Function {
        bounds: Vec<Binding>,
        body: Box<Self>,
    },
    Filter {
        bounds: Vec<Binding>,
        body: Box<Self>,
    },
    MapSet {
        bounds: Vec<Binding>,
        body: Box<Self>,
    },
    Fold {
        #[serde(deserialize_with = "deserialize_wire_index")]
        accumulator: usize,
        initial: Box<Self>,
        collection: Box<Self>,
        order: FoldOrder,
        body: Box<Self>,
    },
    SelectSeq {
        slot: usize,
        sequence: Box<Self>,
        body: Box<Self>,
    },
    SortSeq {
        slot: usize,
        sequence: Box<Self>,
        body: Box<Self>,
    },
    Assert {
        condition: Box<Self>,
        message: Box<Self>,
    },
    Domain {
        argument: Box<Self>,
    },
    VariantTag {
        argument: Box<Self>,
    },
    Cardinality {
        argument: Box<Self>,
    },
    SequenceSet {
        argument: Box<Self>,
    },
    Len {
        argument: Box<Self>,
    },
    Head {
        argument: Box<Self>,
    },
    Tail {
        argument: Box<Self>,
    },
    Powerset {
        argument: Box<Self>,
    },
    Union {
        argument: Box<Self>,
    },
    Cup {
        left: Box<Self>,
        right: Box<Self>,
    },
    Cap {
        left: Box<Self>,
        right: Box<Self>,
    },
    Difference {
        left: Box<Self>,
        right: Box<Self>,
    },
    SubsetEq {
        left: Box<Self>,
        right: Box<Self>,
    },
    Append {
        left: Box<Self>,
        right: Box<Self>,
    },
    Concat {
        left: Box<Self>,
        right: Box<Self>,
    },
    FunctionSet {
        left: Box<Self>,
        right: Box<Self>,
    },
    FunctionIn {
        left: Box<Self>,
        right: Box<Self>,
    },
    SequenceIn {
        left: Box<Self>,
        right: Box<Self>,
    },
    Subseq {
        sequence: Box<Self>,
        from: Box<Self>,
        to: Box<Self>,
    },
    Enabled {
        branches: Vec<Branch>,
        #[serde(default)]
        subscript: Option<Box<Self>>,
    },
    Literal {
        value: Scalar,
    },
    Read {
        #[serde(deserialize_with = "deserialize_wire_index")]
        slot: usize,
        next: bool,
    },
    Not {
        argument: Box<Self>,
    },
    Neg {
        argument: Box<Self>,
    },
    Add {
        left: Box<Self>,
        right: Box<Self>,
    },
    Sub {
        left: Box<Self>,
        right: Box<Self>,
    },
    Mul {
        left: Box<Self>,
        right: Box<Self>,
    },
    Div {
        left: Box<Self>,
        right: Box<Self>,
    },
    Mod {
        left: Box<Self>,
        right: Box<Self>,
    },
    Pow {
        left: Box<Self>,
        right: Box<Self>,
    },
    Eq {
        left: Box<Self>,
        right: Box<Self>,
    },
    Ne {
        left: Box<Self>,
        right: Box<Self>,
    },
    Lt {
        left: Box<Self>,
        right: Box<Self>,
    },
    Le {
        left: Box<Self>,
        right: Box<Self>,
    },
    Gt {
        left: Box<Self>,
        right: Box<Self>,
    },
    Ge {
        left: Box<Self>,
        right: Box<Self>,
    },
    And {
        left: Box<Self>,
        right: Box<Self>,
    },
    Or {
        left: Box<Self>,
        right: Box<Self>,
    },
    Implies {
        left: Box<Self>,
        right: Box<Self>,
    },
    If {
        condition: Box<Self>,
        consequent: Box<Self>,
        alternative: Box<Self>,
    },
    In {
        value: Box<Self>,
        domain: Domain,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum FoldOrder {
    SetMembers,
    SequenceLeft,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Domain {
    Value {
        value: Box<Expression>,
    },
    Range {
        lower: Box<Expression>,
        upper: Box<Expression>,
    },
    Set {
        values: Vec<Expression>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Step {
    Call {
        #[serde(deserialize_with = "deserialize_wire_index")]
        definition: usize,
        arguments: Vec<Expression>,
        captures: Vec<Expression>,
    },
    Frame {
        arguments: Vec<Expression>,
        captures: Vec<Expression>,
        #[serde(rename = "parameterCount")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        parameter_count: usize,
        #[serde(rename = "captureCount")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        capture_count: usize,
        #[serde(rename = "localScope")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        local_scope: usize,
        branches: Vec<Branch>,
    },
    Bind {
        #[serde(deserialize_with = "deserialize_wire_index")]
        slot: usize,
        domain: Domain,
        #[serde(rename = "tupleSize", default, skip_serializing_if = "Option::is_none")]
        #[serde(deserialize_with = "deserialize_optional_wire_index")]
        tuple_size: Option<usize>,
    },
    Guard {
        expression: Expression,
    },
    Assign {
        #[serde(deserialize_with = "deserialize_wire_index")]
        slot: usize,
        value: Expression,
    },
    Choose {
        #[serde(deserialize_with = "deserialize_wire_index")]
        slot: usize,
        domain: Domain,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    #[serde(rename = "tupleSize", default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_optional_wire_index")]
    pub tuple_size: Option<usize>,
    #[serde(deserialize_with = "deserialize_wire_index")]
    pub slot: usize,
    pub domain: Domain,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub path: Vec<Expression>,
    pub value: Expression,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
    pub name: String,
    pub expression: Expression,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SafetySymmetry {
    pub model_values: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SafetyOptions {
    pub workers: usize,
    pub symmetry_actions: bool,
    pub partial_order: bool,
}

impl Default for SafetyOptions {
    fn default() -> Self {
        Self {
            workers: 1,
            symmetry_actions: false,
            partial_order: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "DefinitionWire", rename_all = "camelCase")]
pub struct Definition {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recursive: bool,
    #[serde(deserialize_with = "deserialize_wire_index")]
    pub id: usize,
    #[serde(deserialize_with = "deserialize_wire_index")]
    pub parameter_count: usize,
    #[serde(deserialize_with = "deserialize_wire_index")]
    pub capture_count: usize,
    pub body_kind: BodyKind,
    #[serde(deserialize_with = "deserialize_wire_index")]
    pub local_scope: usize,
    pub body: DefinitionBody,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum DefinitionBody {
    Expression(Expression),
    Action(Vec<Branch>),
    Temporal(Box<TemporalFormula>),
}

impl From<Expression> for DefinitionBody {
    fn from(value: Expression) -> Self {
        Self::Expression(value)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum BodyKind {
    Expression,
    Action,
    Temporal,
}

#[derive(Deserialize)]
#[serde(
    tag = "bodyKind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DefinitionWire {
    Expression {
        #[serde(default)]
        recursive: bool,
        #[serde(deserialize_with = "deserialize_wire_index")]
        id: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        parameter_count: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        capture_count: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        local_scope: usize,
        body: Expression,
    },
    Action {
        #[serde(default)]
        recursive: bool,
        #[serde(deserialize_with = "deserialize_wire_index")]
        id: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        parameter_count: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        capture_count: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        local_scope: usize,
        body: Vec<Branch>,
    },
    Temporal {
        #[serde(default)]
        recursive: bool,
        #[serde(deserialize_with = "deserialize_wire_index")]
        id: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        parameter_count: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        capture_count: usize,
        #[serde(deserialize_with = "deserialize_wire_index")]
        local_scope: usize,
        body: Box<TemporalFormula>,
    },
}

impl From<DefinitionWire> for Definition {
    fn from(value: DefinitionWire) -> Self {
        match value {
            DefinitionWire::Expression {
                recursive,
                id,
                parameter_count,
                capture_count,
                local_scope,
                body,
            } => Self {
                recursive,
                id,
                parameter_count,
                capture_count,
                local_scope,
                body_kind: BodyKind::Expression,
                body: body.into(),
            },
            DefinitionWire::Action {
                recursive,
                id,
                parameter_count,
                capture_count,
                local_scope,
                body,
            } => Self {
                recursive,
                id,
                parameter_count,
                capture_count,
                local_scope,
                body_kind: BodyKind::Action,
                body: DefinitionBody::Action(body),
            },
            DefinitionWire::Temporal {
                recursive,
                id,
                parameter_count,
                capture_count,
                local_scope,
                body,
            } => Self {
                recursive,
                id,
                parameter_count,
                capture_count,
                local_scope,
                body_kind: BodyKind::Temporal,
                body: DefinitionBody::Temporal(body),
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "SafetyProgramWire", rename_all = "camelCase")]
pub struct SafetyProgram {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub variables: Vec<String>,
    pub initial: Vec<Branch>,
    pub next: Vec<Branch>,
    pub invariants: Vec<Predicate>,
    pub assumptions: Vec<Predicate>,
    pub check_deadlock: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symmetry: Option<SafetySymmetry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_properties: Vec<Predicate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_properties: Vec<Predicate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<TemporalProperty>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fairness: Vec<TemporalFormula>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Predicate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_constraints: Vec<Predicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definitions: Option<Vec<Definition>>,
}

#[derive(Default)]
enum DefinitionTable {
    #[default]
    Missing,
    Present(Vec<Definition>),
}

impl<'de> Deserialize<'de> for DefinitionTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<Definition>::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SafetyProgramWire {
    format: String,
    version: u32,
    name: String,
    variables: Vec<String>,
    initial: Vec<Branch>,
    next: Vec<Branch>,
    invariants: Vec<Predicate>,
    assumptions: Vec<Predicate>,
    check_deadlock: bool,
    #[serde(default)]
    symmetry: Option<SafetySymmetry>,
    #[serde(default)]
    initial_properties: Vec<Predicate>,
    #[serde(default)]
    action_properties: Vec<Predicate>,
    #[serde(default)]
    properties: Vec<TemporalProperty>,
    #[serde(default)]
    fairness: Vec<TemporalFormula>,
    #[serde(default)]
    constraints: Vec<Predicate>,
    #[serde(default)]
    action_constraints: Vec<Predicate>,
    #[serde(default)]
    definitions: DefinitionTable,
}

impl TryFrom<SafetyProgramWire> for SafetyProgram {
    type Error = CoreError;

    fn try_from(wire: SafetyProgramWire) -> Result<Self, Self::Error> {
        let definitions = match (wire.version, wire.definitions) {
            (1, DefinitionTable::Missing) => None,
            (2, DefinitionTable::Present(definitions)) => Some(definitions),
            _ => {
                return Err(CoreError::protocol(
                    "Invalid safety program version or definitions",
                ));
            }
        };
        let program = Self {
            format: wire.format,
            version: wire.version,
            name: wire.name,
            variables: wire.variables,
            initial: wire.initial,
            next: wire.next,
            invariants: wire.invariants,
            assumptions: wire.assumptions,
            check_deadlock: wire.check_deadlock,
            symmetry: wire.symmetry,
            initial_properties: wire.initial_properties,
            action_properties: wire.action_properties,
            properties: wire.properties,
            fairness: wire.fairness,
            constraints: wire.constraints,
            action_constraints: wire.action_constraints,
            definitions,
        };
        if program.version == 1 && program.has_shared_expressions() {
            return Err(CoreError::protocol(
                "Shared expressions require safety IR v2",
            ));
        }
        Ok(program)
    }
}

impl SafetyProgram {
    fn has_shared_expressions(&self) -> bool {
        fn branch<'a>(branches: &'a [Branch], pending: &mut Vec<&'a Expression>) -> bool {
            for branch in branches {
                for step in &branch.steps {
                    match step {
                        Step::Frame { .. } | Step::Call { .. } => return true,
                        Step::Guard { expression }
                        | Step::Assign {
                            value: expression, ..
                        } => pending.push(expression),
                        Step::Bind { domain, .. } | Step::Choose { domain, .. } => {
                            pending.extend(domain.children())
                        }
                    }
                }
            }
            false
        }
        let mut pending = vec![];
        if branch(&self.initial, &mut pending) || branch(&self.next, &mut pending) {
            return true;
        }
        for predicate in self
            .invariants
            .iter()
            .chain(&self.assumptions)
            .chain(&self.initial_properties)
            .chain(&self.action_properties)
            .chain(&self.constraints)
            .chain(&self.action_constraints)
        {
            pending.push(&predicate.expression);
        }
        let mut formulas: Vec<_> = self
            .properties
            .iter()
            .map(|property| &property.formula)
            .chain(&self.fairness)
            .collect();
        while let Some(formula) = formulas.pop() {
            match formula {
                TemporalFormula::Frame { .. } | TemporalFormula::Call { .. } => return true,
                TemporalFormula::Atom { expression } => pending.push(expression),
                TemporalFormula::Not { formula }
                | TemporalFormula::Always { formula }
                | TemporalFormula::Eventually { formula } => formulas.push(formula),
                TemporalFormula::And { left, right } | TemporalFormula::Or { left, right } => {
                    formulas.extend([left.as_ref(), right.as_ref()])
                }
                TemporalFormula::Forall { bounds, formula }
                | TemporalFormula::Exists { bounds, formula } => {
                    for binding in bounds {
                        pending.extend(binding.domain.children());
                    }
                    formulas.push(formula);
                }
            }
        }
        while let Some(expression) = pending.pop() {
            match expression {
                Expression::Frame { .. }
                | Expression::Call { .. }
                | Expression::Parameter { .. }
                | Expression::Capture { .. } => return true,
                Expression::Enabled {
                    branches,
                    subscript,
                } => {
                    if branch(branches, &mut pending) {
                        return true;
                    }
                    pending.extend(subscript.as_deref());
                }
                _ => pending.extend(expression.children()),
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SafetyBounds {
    pub max_depth: u32,
    pub max_states: u32,
    pub max_work: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_temporal_nodes: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyTraceStep {
    pub action: Option<String>,
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyViolation {
    pub name: String,
    pub trace: Vec<SafetyTraceStep>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "loopStart")]
    pub loop_start: Option<usize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "loopAction"
    )]
    pub loop_action: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyResult {
    pub status: String,
    pub reason: Option<String>,
    pub states: usize,
    pub generated: u32,
    pub depth: u32,
    pub work: u32,
    pub violation: Option<SafetyViolation>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SafetyActionCoverage {
    pub name: String,
    pub attempts: u32,
    pub enabled: u32,
    pub generated: u32,
    pub distinct: usize,
    pub work: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SafetyCoverage {
    pub initial: Vec<SafetyActionCoverage>,
    pub next: Vec<SafetyActionCoverage>,
    pub unattributed_work: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyCoverageReport {
    pub result: SafetyResult,
    pub coverage: SafetyCoverage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "Value")]
pub enum Scalar {
    Bool(bool),
    Int(i32),
    Ref(u32),
}

impl TryFrom<Value> for Scalar {
    type Error = CoreError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl Serialize for Scalar {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::Bool(value) => Value::Bool(*value),
            Self::Int(value) => Value::Int(value.to_string()),
            Self::Ref(_) => return Err(serde::ser::Error::custom("Internal value reference")),
        };
        value.serialize(serializer)
    }
}

impl Scalar {
    pub(crate) fn parse(value: &Value) -> Result<Self, CoreError> {
        match value {
            Value::Bool(value) => Ok(Self::Bool(*value)),
            Value::Int(value) => {
                let parsed = value.parse::<i32>().map_err(|_| {
                    CoreError::protocol("Safety integer exceeds signed 32 bit range")
                })?;
                if parsed.to_string() != *value {
                    return Err(CoreError::protocol("Noncanonical safety integer"));
                }
                Ok(Self::Int(parsed))
            }
            _ => Err(CoreError::protocol(
                "Safety values must be integers or booleans",
            )),
        }
    }
    pub(crate) fn integer(self) -> Result<i32, CoreError> {
        match self {
            Self::Int(value) => Ok(value),
            _ => Err(CoreError::protocol("Expected an integer")),
        }
    }
    pub(crate) fn boolean(self) -> Result<bool, CoreError> {
        match self {
            Self::Bool(value) => Ok(value),
            _ => Err(CoreError::protocol("Expected a boolean")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TemporalFormula {
    Call {
        #[serde(deserialize_with = "deserialize_wire_index")]
        definition: usize,
        arguments: Vec<Expression>,
        captures: Vec<Expression>,
    },
    Frame {
        arguments: Vec<Expression>,
        captures: Vec<Expression>,
        #[serde(rename = "parameterCount")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        parameter_count: usize,
        #[serde(rename = "captureCount")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        capture_count: usize,
        #[serde(rename = "localScope")]
        #[serde(deserialize_with = "deserialize_wire_index")]
        local_scope: usize,
        formula: Box<Self>,
    },
    Atom {
        expression: Expression,
    },
    Not {
        formula: Box<Self>,
    },
    Always {
        formula: Box<Self>,
    },
    Eventually {
        formula: Box<Self>,
    },
    And {
        left: Box<Self>,
        right: Box<Self>,
    },
    Or {
        left: Box<Self>,
        right: Box<Self>,
    },
    Forall {
        bounds: Vec<Binding>,
        formula: Box<Self>,
    },
    Exists {
        bounds: Vec<Binding>,
        formula: Box<Self>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalProperty {
    pub name: String,
    pub formula: TemporalFormula,
}
