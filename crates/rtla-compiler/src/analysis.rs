#![allow(dead_code)]

use rtla::ast::{self, ExpressionKind as S};
use rtlc::ir::{self, Expression as I};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(crate) enum Node<'a> {
    Source(&'a ast::Expression),
    SourceCall(&'a ast::Expression),
    SourceDefinition(&'a ast::Definition),
    SourceBound(&'a ast::Bound),
    SourceArm(&'a ast::CaseArm),
    SourceField(&'a ast::RecordField),
    SourceUpdate(&'a ast::ExceptUpdate),
    SourcePath(&'a ast::ExceptPath),
    Expression(&'a I),
    Domain(&'a ir::Domain),
    Binding(&'a ir::Binding),
    Branch(&'a ir::Branch),
    Step(&'a ir::Step),
    Definition(&'a ir::Definition),
    Body(&'a ir::DefinitionBody),
    Temporal(&'a ir::TemporalFormula),
    Program(&'a ir::SafetyProgram),
    Call(&'a usize),
}

#[derive(Clone)]
pub(crate) struct Descriptor {
    pub space: &'static str,
    pub constructor: &'static str,
    pub operator: String,
    pub position: String,
    pub role: &'static str,
    pub scope: &'static str,
    pub action: &'static str,
    pub membership: &'static str,
}

pub(crate) struct Edge<'a> {
    pub node: Node<'a>,
    pub descriptor: Descriptor,
    pub depth: usize,
}

fn descriptor(
    space: &'static str,
    constructor: &'static str,
    operator: &str,
    position: &str,
) -> Descriptor {
    let mut result = Descriptor {
        space,
        constructor,
        operator: operator.into(),
        position: position.into(),
        role: "value",
        scope: "caller",
        action: "stop",
        membership: "stop",
    };
    let p = position;
    let source = space == "source";
    if p.is_empty() {
        result.role = "leaf";
    }
    if source {
        match constructor {
            "And" | "Or" => {
                result.role = "action-body";
                result.action = "inherit";
            }
            "Always" | "Eventually" => result.role = "temporal-body",
            "Prime" => result.role = "prime-argument",
            "Unchanged" => {
                result.role = "generative-target";
                result.action = "target-in-action";
            }
            "Enabled" => {
                result.role = "action-body";
                result.action = "start";
            }
            "If" | "Case" => {
                if p.ends_with("condition") {
                    result.role = "condition";
                } else {
                    result.role = "result";
                    result.action = "inherit";
                    result.membership = "inherit";
                }
            }
            "Let" => {
                if p.starts_with("definitions") {
                    result.role = "definition-body";
                    result.scope = "declaration";
                } else {
                    result.role = "result";
                    result.action = "inherit";
                    result.membership = "inherit";
                }
            }
            "Label" => {
                result.role = "result";
                result.action = "inherit";
                result.membership = "inherit";
            }
            "Forall" | "Exists" | "FunctionConstruction" | "Choose" | "SetFilter" | "SetMap" => {
                if p.ends_with(".set") || p == "set" {
                    result.role = "binder-domain";
                } else {
                    result.role = "binder-body";
                    result.scope = "binder";
                    if constructor == "Exists" {
                        result.action = "inherit";
                    }
                }
            }
            "FunctionSet" if p == "range" => {
                result.role = "membership-result";
                result.membership = "inherit";
            }
            "RecordSet" => {
                result.role = "membership-result";
                result.membership = "inherit";
            }
            "Except" => {
                if p == "function" {
                    result.role = "EXCEPT-base";
                } else if p.ends_with(".value") {
                    result.role = "EXCEPT-replacement";
                    result.scope = "old";
                } else {
                    result.role = "EXCEPT-key";
                }
            }
            "FunctionApplication" if p.starts_with("args") => result.role = "call-actual",
            "BoxAction" | "AngleAction" | "WeakFairness" | "StrongFairness" => {
                if p == "action" {
                    result.role = "action-body";
                    result.action = "inherit";
                } else {
                    result.role = "subscript";
                    result.action = if matches!(constructor, "WeakFairness" | "StrongFairness") {
                        "target"
                    } else {
                        "target-in-action"
                    };
                }
            }
            "Apply" => {
                let op = operator.rsplit_once('/').map_or(operator, |(op, _)| op);
                let rhs = p == "args[1]";
                match op {
                    "\\in" | "∈" | "\\notin" | "∉" => {
                        result.role = if rhs {
                            "membership-result"
                        } else {
                            "membership-operand"
                        };
                        if rhs {
                            result.membership = "start";
                        } else if matches!(op, "\\in" | "∈") {
                            result.action = "target-if-identifier";
                        }
                    }
                    "Seq" => {
                        result.role = "membership-result";
                        result.membership = "inherit";
                    }
                    "=" => {
                        result.role = "assignment-target";
                        result.action = "target-if-identifier";
                    }
                    "~>" => result.role = "temporal-body",
                    "ApaFoldSet" | "ApaFoldSeqLeft" if p == "args[0]" => {
                        result.role = "operator-reference";
                        result.scope = "declaration";
                    }
                    "user call representative" => {
                        result.role = "call-actual";
                        result.action = "summary";
                        result.membership = "summary";
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    } else {
        match space {
            "source-elaboration" => {
                result.role = "elaboration-input";
                result.scope = "lexical";
            }
            "parser-only" => {
                result.role = if constructor == "Fairness" {
                    "annotation"
                } else {
                    "parser-or-context-refusal"
                };
                result.scope = if constructor == "Fairness" {
                    "annotation"
                } else {
                    "syntax"
                };
            }
            "source-wrapper" => {
                if constructor.ends_with("Definition") {
                    result.role = "definition-body";
                    result.scope = "declaration";
                } else if constructor.starts_with("ExceptPath") {
                    result.role = "EXCEPT-key";
                }
            }
            "ir-expression" => match constructor {
                "select-seq" | "sort-seq" if p == "body" => {
                    result.role = "binder-body";
                    result.scope = "binder";
                }
                "parameter" | "capture" | "bound" => result.scope = "frame",
                "record-set" | "sequence-set" => {
                    result.role = "membership-result";
                    result.membership = "inherit";
                }
                "function-set" if p == "right" => {
                    result.role = "membership-result";
                    result.membership = "inherit";
                }
                "update" => {
                    if p == "target" {
                        result.role = "EXCEPT-base";
                    } else if p.ends_with(".value") {
                        result.role = "EXCEPT-replacement";
                        result.scope = "old";
                    } else {
                        result.role = "EXCEPT-key";
                    }
                }
                "forall" | "exists" | "function" | "filter" | "map-set" | "choose" => {
                    if p == "body" {
                        result.role = "binder-body";
                        result.scope = "binder";
                        if constructor == "exists" {
                            result.action = "inherit";
                        }
                    } else {
                        result.role = "binder-domain";
                    }
                }
                "fold" if p == "body" => {
                    result.role = "binder-body";
                    result.scope = "binder";
                }
                "prime" => result.role = "prime-argument",
                "enabled" => {
                    if p.starts_with("branches") {
                        result.role = "action-branch";
                        result.action = "start";
                    } else {
                        result.role = "subscript";
                        result.action = "target";
                    }
                }
                "if" => {
                    if p == "condition" {
                        result.role = "condition";
                    } else {
                        result.role = "result";
                        result.action = "inherit";
                        result.membership = "inherit";
                    }
                }
                "in" => {
                    if p == "value" {
                        result.role = "membership-operand";
                    } else {
                        result.role = "membership-result";
                        result.membership = "start";
                    }
                }
                "frame" | "call" => {
                    if p.starts_with("arguments") {
                        result.role = if constructor == "frame" {
                            "frame-actual"
                        } else {
                            "call-actual"
                        };
                    } else if p.starts_with("captures") {
                        result.role = if constructor == "frame" {
                            "frame-capture"
                        } else {
                            "call-capture"
                        };
                    } else if p == "definition" {
                        result.role = "call-edge";
                        result.scope = "definition";
                    } else {
                        result.role = "frame-body";
                        result.scope = "frame";
                    }
                }
                _ => {}
            },
            "ir-domain" | "ir-binding" => result.role = "binder-domain",
            "ir-branch" => {
                result.role = "action-step";
                result.action = "inherit";
            }
            "ir-definition" => {
                result.role = "definition-body";
                result.scope = "frame";
            }
            "ir-step" | "ir-temporal" if constructor == "call" => {
                if p.starts_with("arguments") {
                    result.role = "call-actual";
                } else if p.starts_with("captures") {
                    result.role = "call-capture";
                } else {
                    result.role = "call-edge";
                    result.scope = "definition";
                }
            }
            "ir-step" if constructor == "frame" => {
                if p.starts_with("arguments") {
                    result.role = "frame-actual";
                } else if p.starts_with("captures") {
                    result.role = "frame-capture";
                } else {
                    result.role = "action-frame-body";
                    result.scope = "frame";
                    result.action = "inherit";
                }
            }
            "ir-temporal" => {
                if constructor == "frame" {
                    if p.starts_with("arguments") {
                        result.role = "frame-actual";
                    } else if p.starts_with("captures") {
                        result.role = "frame-capture";
                    } else {
                        result.role = "frame-body";
                        result.scope = "frame";
                    }
                } else if p.starts_with("bounds") {
                    result.role = "binder-domain";
                } else if constructor != "atom" {
                    result.role = "temporal-body";
                    if matches!(constructor, "forall" | "exists") {
                        result.scope = "binder";
                    }
                }
            }
            "ir-program" => {
                if p.starts_with("initial[") || p.starts_with("next[") {
                    result.role = "action-branch";
                    result.action = "start";
                } else if p.starts_with("definitions") {
                    result.role = "definition-body";
                    result.scope = "declaration";
                } else if p.starts_with("properties") || p.starts_with("fairness") {
                    result.role = "temporal-body";
                }
            }
            _ => {}
        }
    }
    result
}

fn edge<'a>(
    node: Node<'a>,
    space: &'static str,
    constructor: &'static str,
    operator: &str,
    position: impl AsRef<str>,
) -> Edge<'a> {
    Edge {
        node,
        descriptor: descriptor(space, constructor, operator, position.as_ref()),
        depth: usize::from(
            space == "ir-expression"
                || (space == "ir-step" || space == "ir-temporal") && constructor == "frame",
        ),
    }
}

macro_rules! expression_edges_body {
    ($value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let value = $value;
        let mut out = vec![];
        let mut add = |ctor, pos, node| out.push($edge(node, "ir-expression", ctor, "", pos));
        match value {
            I::Parameter { slot: _ } => {}
            I::Capture { slot: _ } => {}
            I::Bound { slot: _ } => {}
            I::Old => {}
            I::UnmatchedCase { location: _ } => {}
            I::Data { value: _ } | I::Literal { value: _ } => {}
            I::Read { slot: _, next: _ } => {}
            I::SelectSeq {
                slot: _,
                sequence,
                body,
            } => {
                add("select-seq", "sequence", $node::Expression(sequence));
                add("select-seq", "body", $node::Expression(body));
            }
            I::SortSeq {
                slot: _,
                sequence,
                body,
            } => {
                add("sort-seq", "sequence", $node::Expression(sequence));
                add("sort-seq", "body", $node::Expression(body));
            }
            I::Assert { condition, message } => {
                add("assert", "condition", $node::Expression(condition));
                add("assert", "message", $node::Expression(message));
            }
            I::Domain { argument } => add("domain", "argument", $node::Expression(argument)),
            I::VariantTag { argument } => {
                add("variant-tag", "argument", $node::Expression(argument))
            }
            I::Cardinality { argument } => {
                add("cardinality", "argument", $node::Expression(argument))
            }
            I::SequenceSet { argument } => {
                add("sequence-set", "argument", $node::Expression(argument))
            }
            I::Len { argument } => add("len", "argument", $node::Expression(argument)),
            I::Head { argument } => add("head", "argument", $node::Expression(argument)),
            I::Tail { argument } => add("tail", "argument", $node::Expression(argument)),
            I::Powerset { argument } => add("powerset", "argument", $node::Expression(argument)),
            I::Union { argument } => add("union", "argument", $node::Expression(argument)),
            I::Not { argument } => add("not", "argument", $node::Expression(argument)),
            I::Neg { argument } => add("neg", "argument", $node::Expression(argument)),
            I::Prime { argument } => add("prime", "argument", $node::Expression(argument)),
            I::Cup { left, right } => {
                add("cup", "left", $node::Expression(left));
                add("cup", "right", $node::Expression(right));
            }
            I::Cap { left, right } => {
                add("cap", "left", $node::Expression(left));
                add("cap", "right", $node::Expression(right));
            }
            I::Difference { left, right } => {
                add("difference", "left", $node::Expression(left));
                add("difference", "right", $node::Expression(right));
            }
            I::SubsetEq { left, right } => {
                add("subset-eq", "left", $node::Expression(left));
                add("subset-eq", "right", $node::Expression(right));
            }
            I::Append { left, right } => {
                add("append", "left", $node::Expression(left));
                add("append", "right", $node::Expression(right));
            }
            I::Concat { left, right } => {
                add("concat", "left", $node::Expression(left));
                add("concat", "right", $node::Expression(right));
            }
            I::FunctionSet { left, right } => {
                add("function-set", "left", $node::Expression(left));
                add("function-set", "right", $node::Expression(right));
            }
            I::FunctionIn { left, right } => {
                add("function-in", "left", $node::Expression(left));
                add("function-in", "right", $node::Expression(right));
            }
            I::SequenceIn { left, right } => {
                add("sequence-in", "left", $node::Expression(left));
                add("sequence-in", "right", $node::Expression(right));
            }
            I::Add { left, right } => {
                add("add", "left", $node::Expression(left));
                add("add", "right", $node::Expression(right));
            }
            I::Sub { left, right } => {
                add("sub", "left", $node::Expression(left));
                add("sub", "right", $node::Expression(right));
            }
            I::Mul { left, right } => {
                add("mul", "left", $node::Expression(left));
                add("mul", "right", $node::Expression(right));
            }
            I::Div { left, right } => {
                add("div", "left", $node::Expression(left));
                add("div", "right", $node::Expression(right));
            }
            I::Mod { left, right } => {
                add("mod", "left", $node::Expression(left));
                add("mod", "right", $node::Expression(right));
            }
            I::Pow { left, right } => {
                add("pow", "left", $node::Expression(left));
                add("pow", "right", $node::Expression(right));
            }
            I::Eq { left, right } => {
                add("eq", "left", $node::Expression(left));
                add("eq", "right", $node::Expression(right));
            }
            I::Ne { left, right } => {
                add("ne", "left", $node::Expression(left));
                add("ne", "right", $node::Expression(right));
            }
            I::Lt { left, right } => {
                add("lt", "left", $node::Expression(left));
                add("lt", "right", $node::Expression(right));
            }
            I::Le { left, right } => {
                add("le", "left", $node::Expression(left));
                add("le", "right", $node::Expression(right));
            }
            I::Gt { left, right } => {
                add("gt", "left", $node::Expression(left));
                add("gt", "right", $node::Expression(right));
            }
            I::Ge { left, right } => {
                add("ge", "left", $node::Expression(left));
                add("ge", "right", $node::Expression(right));
            }
            I::And { left, right } => {
                add("and", "left", $node::Expression(left));
                add("and", "right", $node::Expression(right));
            }
            I::Or { left, right } => {
                add("or", "left", $node::Expression(left));
                add("or", "right", $node::Expression(right));
            }
            I::Implies { left, right } => {
                add("implies", "left", $node::Expression(left));
                add("implies", "right", $node::Expression(right));
            }
            I::Set { values } => {
                for v in values {
                    add("set", "values[*]", $node::Expression(v));
                }
            }
            I::Tuple { values } => {
                for v in values {
                    add("tuple", "values[*]", $node::Expression(v));
                }
            }
            I::Record { fields } => {
                for (_, v) in fields {
                    add("record", "fields[*][1]", $node::Expression(v));
                }
            }
            I::RecordSet { fields } => {
                for (_, v) in fields {
                    add("record-set", "fields[*][1]", $node::Expression(v));
                }
            }
            I::Forall { bounds, body } => {
                for v in bounds {
                    add("forall", "bounds[*]", $node::Binding(v));
                }
                add("forall", "body", $node::Expression(body));
            }
            I::Exists { bounds, body } => {
                for v in bounds {
                    add("exists", "bounds[*]", $node::Binding(v));
                }
                add("exists", "body", $node::Expression(body));
            }
            I::Function { bounds, body } => {
                for v in bounds {
                    add("function", "bounds[*]", $node::Binding(v));
                }
                add("function", "body", $node::Expression(body));
            }
            I::Filter { bounds, body } => {
                for v in bounds {
                    add("filter", "bounds[*]", $node::Binding(v));
                }
                add("filter", "body", $node::Expression(body));
            }
            I::MapSet { bounds, body } => {
                for v in bounds {
                    add("map-set", "bounds[*]", $node::Binding(v));
                }
                add("map-set", "body", $node::Expression(body));
            }
            I::Choose { binding, body } => {
                add("choose", "binding", $node::Binding(binding));
                add("choose", "body", $node::Expression(body));
            }
            I::At { target, key } => {
                add("at", "target", $node::Expression(target));
                add("at", "key", $node::Expression(key));
            }
            I::Update { target, updates } => {
                add("update", "target", $node::Expression(target));
                for ir::Update { path, value } in updates {
                    for v in path {
                        add("update", "updates[*].path[*]", $node::Expression(v));
                    }
                    add("update", "updates[*].value", $node::Expression(value));
                }
            }
            I::Fold {
                accumulator: _,
                initial,
                collection,
                order: _,
                body,
            } => {
                add("fold", "initial", $node::Expression(initial));
                add("fold", "collection", $node::Expression(collection));
                add("fold", "body", $node::Expression(body));
            }
            I::Subseq { sequence, from, to } => {
                add("subseq", "sequence", $node::Expression(sequence));
                add("subseq", "from", $node::Expression(from));
                add("subseq", "to", $node::Expression(to));
            }
            I::Enabled {
                branches,
                subscript,
            } => {
                for v in branches {
                    add("enabled", "branches[*]", $node::Branch(v));
                }
                if let Some(v) = subscript {
                    add("enabled", "subscript", $node::Expression(v));
                }
            }
            I::If {
                condition,
                consequent,
                alternative,
            } => {
                add("if", "condition", $node::Expression(condition));
                add("if", "consequent", $node::Expression(consequent));
                add("if", "alternative", $node::Expression(alternative));
            }
            I::In { value, domain } => {
                add("in", "value", $node::Expression(value));
                add("in", "domain", $node::Domain(domain));
            }
            I::Call {
                definition,
                arguments,
                captures,
            } => {
                add("call", "definition", $node::Call(definition));
                for v in arguments {
                    add("call", "arguments[*]", $node::Expression(v));
                }
                for v in captures {
                    add("call", "captures[*]", $node::Expression(v));
                }
            }
            I::Frame {
                arguments,
                captures,
                parameter_count: _,
                capture_count: _,
                local_scope: _,
                body,
            } => {
                for v in arguments {
                    add("frame", "arguments[*]", $node::Expression(v));
                }
                for v in captures {
                    add("frame", "captures[*]", $node::Expression(v));
                }
                add("frame", "body", $node::Expression(body));
            }
        }
        out
    }};
}

macro_rules! step_edges_body {
    ($value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let value = $value;
        match value {
            ir::Step::Call {
                definition,
                arguments,
                captures,
            } => arguments
                .$iter()
                .map(|value| {
                    $edge(
                        $node::Expression(value),
                        "ir-step",
                        "call",
                        "",
                        "arguments[*]",
                    )
                })
                .chain(captures.$iter().map(|value| {
                    $edge(
                        $node::Expression(value),
                        "ir-step",
                        "call",
                        "",
                        "captures[*]",
                    )
                }))
                .chain([$edge(
                    $node::Call(definition),
                    "ir-step",
                    "call",
                    "",
                    "definition",
                )])
                .collect(),
            ir::Step::Bind {
                slot: _,
                domain,
                tuple_size: _,
            } => vec![$edge(
                $node::Domain(domain),
                "ir-step",
                "bind",
                "",
                "domain",
            )],
            ir::Step::Guard { expression } => vec![$edge(
                $node::Expression(expression),
                "ir-step",
                "guard",
                "",
                "expression",
            )],
            ir::Step::Assign { slot: _, value } => vec![$edge(
                $node::Expression(value),
                "ir-step",
                "assign",
                "",
                "value",
            )],
            ir::Step::Choose { slot: _, domain } => vec![$edge(
                $node::Domain(domain),
                "ir-step",
                "choose",
                "",
                "domain",
            )],
            ir::Step::Frame {
                arguments,
                captures,
                parameter_count: _,
                capture_count: _,
                local_scope: _,
                branches,
            } => {
                arguments
                    .$iter()
                    .map(|v| $edge($node::Expression(v), "ir-step", "frame", "", "arguments[*]"))
                    .chain(captures.$iter().map(|v| {
                        $edge($node::Expression(v), "ir-step", "frame", "", "captures[*]")
                    }))
                    .chain(
                        branches.$iter().map(|v| {
                            $edge($node::Branch(v), "ir-step", "frame", "", "branches[*]")
                        }),
                    )
                    .collect()
            }
        }
    }};
}

macro_rules! temporal_edges_body {
    ($value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let value = $value;
        use ir::TemporalFormula as T;
        match value {
            T::Call {
                definition,
                arguments,
                captures,
            } => arguments
                .$iter()
                .map(|value| {
                    $edge(
                        $node::Expression(value),
                        "ir-temporal",
                        "call",
                        "",
                        "arguments[*]",
                    )
                })
                .chain(captures.$iter().map(|value| {
                    $edge(
                        $node::Expression(value),
                        "ir-temporal",
                        "call",
                        "",
                        "captures[*]",
                    )
                }))
                .chain([$edge(
                    $node::Call(definition),
                    "ir-temporal",
                    "call",
                    "",
                    "definition",
                )])
                .collect(),
            T::Frame {
                arguments,
                captures,
                parameter_count: _,
                capture_count: _,
                local_scope: _,
                formula,
            } => arguments
                .$iter()
                .map(|v| {
                    $edge(
                        $node::Expression(v),
                        "ir-temporal",
                        "frame",
                        "",
                        "arguments[*]",
                    )
                })
                .chain(captures.$iter().map(|v| {
                    $edge(
                        $node::Expression(v),
                        "ir-temporal",
                        "frame",
                        "",
                        "captures[*]",
                    )
                }))
                .chain([$edge(
                    $node::Temporal(formula),
                    "ir-temporal",
                    "frame",
                    "",
                    "formula",
                )])
                .collect(),
            T::Atom { expression } => vec![$edge(
                $node::Expression(expression),
                "ir-temporal",
                "atom",
                "",
                "expression",
            )],
            T::Not { formula } => vec![$edge(
                $node::Temporal(formula),
                "ir-temporal",
                "not",
                "",
                "formula",
            )],
            T::Always { formula } => vec![$edge(
                $node::Temporal(formula),
                "ir-temporal",
                "always",
                "",
                "formula",
            )],
            T::Eventually { formula } => vec![$edge(
                $node::Temporal(formula),
                "ir-temporal",
                "eventually",
                "",
                "formula",
            )],
            T::And { left, right } => vec![
                $edge($node::Temporal(left), "ir-temporal", "and", "", "left"),
                $edge($node::Temporal(right), "ir-temporal", "and", "", "right"),
            ],
            T::Or { left, right } => vec![
                $edge($node::Temporal(left), "ir-temporal", "or", "", "left"),
                $edge($node::Temporal(right), "ir-temporal", "or", "", "right"),
            ],
            T::Forall { bounds, formula } => bounds
                .$iter()
                .map(|v| $edge($node::Binding(v), "ir-temporal", "forall", "", "bounds[*]"))
                .chain([$edge(
                    $node::Temporal(formula),
                    "ir-temporal",
                    "forall",
                    "",
                    "formula",
                )])
                .collect(),
            T::Exists { bounds, formula } => bounds
                .$iter()
                .map(|v| $edge($node::Binding(v), "ir-temporal", "exists", "", "bounds[*]"))
                .chain([$edge(
                    $node::Temporal(formula),
                    "ir-temporal",
                    "exists",
                    "",
                    "formula",
                )])
                .collect(),
        }
    }};
}

macro_rules! program_edges_body {
    ($value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let value = $value;
        let ir::SafetyProgram {
            format: _,
            version: _,
            name: _,
            variables: _,
            initial,
            next,
            invariants,
            assumptions,
            check_deadlock: _,
            symmetry: _,
            initial_properties,
            action_properties,
            properties,
            fairness,
            constraints,
            action_constraints,
            definitions,
        } = value;
        let mut out = vec![];
        for (position, branches) in [("initial[*]", initial), ("next[*]", next)] {
            for v in branches {
                out.push($edge(
                    $node::Branch(v),
                    "ir-program",
                    "program",
                    "",
                    position,
                ));
            }
        }
        for (position, predicates) in [
            ("invariants[*].expression", invariants),
            ("assumptions[*].expression", assumptions),
            ("initialProperties[*].expression", initial_properties),
            ("actionProperties[*].expression", action_properties),
            ("constraints[*].expression", constraints),
            ("actionConstraints[*].expression", action_constraints),
        ] {
            for ir::Predicate {
                name: _,
                expression,
            } in predicates
            {
                out.push($edge(
                    $node::Expression(expression),
                    "ir-program",
                    "program",
                    "",
                    position,
                ));
            }
        }
        for ir::TemporalProperty { name: _, formula } in properties {
            out.push($edge(
                $node::Temporal(formula),
                "ir-program",
                "program",
                "",
                "properties[*].formula",
            ));
        }
        for v in fairness {
            out.push($edge(
                $node::Temporal(v),
                "ir-program",
                "program",
                "",
                "fairness[*]",
            ));
        }
        if let Some(values) = definitions {
            for v in values {
                out.push($edge(
                    $node::Definition(v),
                    "ir-program",
                    "program",
                    "",
                    "definitions[*]",
                ));
            }
        }
        out
    }};
}

macro_rules! ir_wrapper_edges {
    (Domain, $value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        match $value {
            ir::Domain::Value { value } => vec![$edge(
                $node::Expression(value),
                "ir-domain",
                "value",
                "",
                "value",
            )],
            ir::Domain::Range { lower, upper } => vec![
                $edge($node::Expression(lower), "ir-domain", "range", "", "lower"),
                $edge($node::Expression(upper), "ir-domain", "range", "", "upper"),
            ],
            ir::Domain::Set { values } => values
                .$iter()
                .map(|v| $edge($node::Expression(v), "ir-domain", "set", "", "values[*]"))
                .collect(),
        }
    }};
    (Binding, $value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let ir::Binding {
            tuple_size: _,
            slot: _,
            domain,
        } = $value;
        vec![$edge(
            $node::Domain(domain),
            "ir-binding",
            "binding",
            "",
            "domain",
        )]
    }};
    (Branch, $value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let ir::Branch { name: _, steps } = $value;
        steps
            .$iter()
            .map(|v| $edge($node::Step(v), "ir-branch", "branch", "", "steps[*]"))
            .collect()
    }};
    (Definition, $value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let ir::Definition {
            recursive: _,
            id: _,
            parameter_count: _,
            capture_count: _,
            body_kind: _,
            local_scope: _,
            body,
        } = $value;
        match body {
            ir::DefinitionBody::Temporal(body) => vec![$edge(
                $node::Temporal(body),
                "ir-definition",
                "definition",
                "",
                "body",
            )],
            ir::DefinitionBody::Expression(body) => vec![$edge(
                $node::Expression(body),
                "ir-definition",
                "definition",
                "",
                "body",
            )],
            ir::DefinitionBody::Action(branches) => branches
                .$iter()
                .map(|branch| {
                    $edge(
                        $node::Branch(branch),
                        "ir-definition",
                        "definition",
                        "",
                        "body[*]",
                    )
                })
                .collect(),
        }
    }};
    (Call, $value:ident, $node:ident, $edge:ident, $iter:ident) => {{
        let _ = $value;
        vec![]
    }};
}

pub(crate) fn analysis_children(node: Node<'_>) -> Vec<Edge<'_>> {
    match node {
        Node::Source(value) => source_edges(value, false),
        Node::SourceCall(value) => source_edges(value, true),
        Node::SourceDefinition(value) => {
            let ast::Definition {
                name: _,
                head,
                body,
                is_local: _,
                is_recursive: _,
                location: _,
            } = value;
            match head {
                ast::DefinitionKind::Operator { params: _ } => vec![edge(
                    Node::Source(body),
                    "source-wrapper",
                    "OperatorDefinition",
                    "",
                    "body",
                )],
                ast::DefinitionKind::Function { bounds } => bounds
                    .iter()
                    .map(|ast::Bound { pattern: _, set }| {
                        edge(
                            Node::Source(set),
                            "source-wrapper",
                            "FunctionDefinition",
                            "",
                            "bounds[*].set",
                        )
                    })
                    .chain([edge(
                        Node::Source(body),
                        "source-wrapper",
                        "FunctionDefinition",
                        "",
                        "body",
                    )])
                    .collect(),
            }
        }
        Node::SourceBound(value) => {
            let ast::Bound { pattern: _, set } = value;
            vec![edge(
                Node::Source(set),
                "source-wrapper",
                "Bound",
                "",
                "set",
            )]
        }
        Node::SourceArm(value) => {
            let ast::CaseArm { condition, body } = value;
            vec![
                edge(
                    Node::Source(condition),
                    "source-wrapper",
                    "CaseArm",
                    "",
                    "condition",
                ),
                edge(Node::Source(body), "source-wrapper", "CaseArm", "", "body"),
            ]
        }
        Node::SourceField(value) => {
            let ast::RecordField {
                name: _,
                value,
                location: _,
            } = value;
            vec![edge(
                Node::Source(value),
                "source-wrapper",
                "RecordField",
                "",
                "value",
            )]
        }
        Node::SourceUpdate(value) => {
            let ast::ExceptUpdate { path, value } = value;
            path.iter()
                .map(|p| {
                    edge(
                        Node::SourcePath(p),
                        "source-wrapper",
                        "ExceptUpdate",
                        "",
                        "path[*]",
                    )
                })
                .chain([edge(
                    Node::Source(value),
                    "source-wrapper",
                    "ExceptUpdate",
                    "",
                    "value",
                )])
                .collect()
        }
        Node::SourcePath(value) => match value {
            ast::ExceptPath::Field { name: _ } => vec![],
            ast::ExceptPath::Index { args } => args
                .iter()
                .map(|v| {
                    edge(
                        Node::Source(v),
                        "source-wrapper",
                        "ExceptPath.Index",
                        "",
                        "args[*]",
                    )
                })
                .collect(),
        },
        Node::Expression(value) => expression_edges(value),
        Node::Domain(value) => ir_wrapper_edges!(Domain, value, Node, edge, iter),
        Node::Binding(value) => ir_wrapper_edges!(Binding, value, Node, edge, iter),
        Node::Branch(value) => ir_wrapper_edges!(Branch, value, Node, edge, iter),
        Node::Step(value) => step_edges(value),
        Node::Body(body) => match body {
            ir::DefinitionBody::Temporal(body) => vec![edge(
                Node::Temporal(body),
                "ir-body",
                "temporal",
                "",
                "body",
            )],
            ir::DefinitionBody::Expression(body) => vec![edge(
                Node::Expression(body),
                "ir-body",
                "expression",
                "",
                "body",
            )],
            ir::DefinitionBody::Action(branches) => branches
                .iter()
                .map(|branch| edge(Node::Branch(branch), "ir-body", "action", "", "body[*]"))
                .collect(),
        },
        Node::Definition(value) => ir_wrapper_edges!(Definition, value, Node, edge, iter),
        Node::Temporal(value) => temporal_edges(value),
        Node::Program(value) => program_edges(value),
        Node::Call(_) => vec![],
    }
}

fn source_edges(value: &ast::Expression, call: bool) -> Vec<Edge<'_>> {
    let ast::Expression { node, location: _ } = value;
    let mut out = Vec::new();
    let mut add = |constructor, operator: &str, position: String, value| {
        out.push(edge(value, "source", constructor, operator, position))
    };
    match node {
        S::Number { value: _ }
        | S::Integer { text: _ }
        | S::Decimal { text: _ }
        | S::String { value: _ }
        | S::Identifier { name: _ }
        | S::ExceptAt => {}
        S::Apply { operator, args } => {
            let spelling = if operator == "-." { "-" } else { operator };
            let known = apply_spellings()
                .iter()
                .any(|(op, arity)| *op == spelling && *arity == args.len());
            let class = if known && !call {
                format!("{spelling}/{}", args.len())
            } else {
                format!("user call representative/{}", args.len())
            };
            for (i, v) in args.iter().enumerate() {
                add("Apply", &class, format!("args[{i}]"), Node::Source(v));
            }
        }
        S::Lambda { params: _, body } => out.push(edge(
            Node::Source(body),
            "source-elaboration",
            "Lambda",
            "",
            "body",
        )),
        S::AssumeProve {
            assumptions, prove, ..
        } => {
            for (index, assumption) in assumptions.iter().enumerate() {
                let expression = match assumption {
                    ast::ProofAssumption::Expression { expression } => Some(expression),
                    ast::ProofAssumption::New { set, .. } => set.as_deref(),
                };
                if let Some(expression) = expression {
                    out.push(edge(
                        Node::Source(expression),
                        "parser-only",
                        "AssumeProve",
                        "",
                        format!("assumptions[{index}]"),
                    ));
                }
            }
            out.push(edge(
                Node::Source(prove),
                "parser-only",
                "AssumeProve",
                "",
                "prove",
            ));
        }
        S::LetModule {
            definitions,
            instances,
            recursive: _,
            body,
        } => {
            for (index, definition) in definitions.iter().enumerate() {
                out.push(edge(
                    Node::SourceDefinition(definition),
                    "source-elaboration",
                    "LetModule",
                    "",
                    format!("definitions[{index}]"),
                ));
            }
            for (index, instance) in instances.iter().enumerate() {
                for (argument, entry) in instance.substitutions.iter().enumerate() {
                    out.push(edge(
                        Node::Source(&entry.expression),
                        "source-elaboration",
                        "LetModule",
                        "",
                        format!("instances[{index}].substitutions[{argument}]"),
                    ));
                }
            }
            out.push(edge(
                Node::Source(body),
                "source-elaboration",
                "LetModule",
                "",
                "body",
            ));
        }
        S::Selection {
            expression,
            selectors,
        } => {
            out.push(edge(
                Node::Source(expression),
                "source-elaboration",
                "Selection",
                "",
                "expression",
            ));
            for (index, selector) in selectors.iter().enumerate() {
                if let ast::Selector::Name { args, .. } | ast::Selector::Arguments { args } =
                    selector
                {
                    for (argument, value) in args.iter().enumerate() {
                        out.push(edge(
                            Node::Source(value),
                            "source-elaboration",
                            "Selection",
                            "",
                            format!("selectors[{index}].args[{argument}]"),
                        ));
                    }
                }
            }
        }
        S::And {
            operands,
            is_bulleted: _,
        }
        | S::Or {
            operands,
            is_bulleted: _,
        } => {
            let (ctor, op) = if matches!(
                node,
                S::And {
                    operands: _,
                    is_bulleted: _
                }
            ) {
                ("And", "/\\")
            } else {
                ("Or", "\\/")
            };
            for (i, v) in operands.iter().enumerate() {
                add(ctor, op, format!("operands[{i}]"), Node::Source(v));
            }
        }
        S::Prime { expression } => add("Prime", "'", "expression".into(), Node::Source(expression)),
        S::Unchanged { expression } => add(
            "Unchanged",
            "UNCHANGED",
            "expression".into(),
            Node::Source(expression),
        ),
        S::Enabled { expression } => add(
            "Enabled",
            "ENABLED",
            "expression".into(),
            Node::Source(expression),
        ),
        S::Always { expression } => add(
            "Always",
            "[]",
            "expression".into(),
            Node::Source(expression),
        ),
        S::Eventually { expression } => add(
            "Eventually",
            "<>",
            "expression".into(),
            Node::Source(expression),
        ),
        S::If {
            condition,
            consequent,
            alternative,
        } => {
            add("If", "", "condition".into(), Node::Source(condition));
            add("If", "", "consequent".into(), Node::Source(consequent));
            add("If", "", "alternative".into(), Node::Source(alternative));
        }
        S::Case { arms, other } => {
            for (i, ast::CaseArm { condition, body }) in arms.iter().enumerate() {
                add(
                    "Case",
                    "",
                    format!("arms[{i}].condition"),
                    Node::Source(condition),
                );
                add("Case", "", format!("arms[{i}].body"), Node::Source(body));
            }
            if let Some(v) = other {
                add("Case", "", "other".into(), Node::Source(v));
            }
        }
        S::Let { definitions, body } => {
            for (i, v) in definitions.iter().enumerate() {
                add(
                    "Let",
                    "",
                    format!("definitions[{i}]"),
                    Node::SourceDefinition(v),
                );
            }
            add("Let", "", "body".into(), Node::Source(body));
        }
        S::Forall { bounds, body } => {
            for (i, ast::Bound { pattern: _, set }) in bounds.iter().enumerate() {
                add(
                    "Forall",
                    "\\A",
                    format!("bounds[{i}].set"),
                    Node::Source(set),
                );
            }
            add("Forall", "\\A", "body".into(), Node::Source(body));
        }
        S::Exists { bounds, body } => {
            for (i, ast::Bound { pattern: _, set }) in bounds.iter().enumerate() {
                add(
                    "Exists",
                    "\\E",
                    format!("bounds[{i}].set"),
                    Node::Source(set),
                );
            }
            add("Exists", "\\E", "body".into(), Node::Source(body));
        }
        S::FunctionConstruction { bounds, body } => {
            for (i, ast::Bound { pattern: _, set }) in bounds.iter().enumerate() {
                add(
                    "FunctionConstruction",
                    "",
                    format!("bounds[{i}].set"),
                    Node::Source(set),
                );
            }
            add(
                "FunctionConstruction",
                "",
                "body".into(),
                Node::Source(body),
            );
        }
        S::Choose {
            pattern: _,
            set,
            body,
        } => {
            if let Some(v) = set {
                add("Choose", "CHOOSE", "set".into(), Node::Source(v));
            }
            add("Choose", "CHOOSE", "body".into(), Node::Source(body));
        }
        S::SetEnumeration { elements } | S::Tuple { elements } => {
            let (ctor, op) = if matches!(node, S::Tuple { elements: _ }) {
                ("Tuple", "<<...>>")
            } else {
                ("SetEnumeration", "")
            };
            for (i, v) in elements.iter().enumerate() {
                add(ctor, op, format!("elements[{i}]"), Node::Source(v));
            }
        }
        S::SetFilter { bound, predicate } => {
            let ast::Bound { pattern: _, set } = bound.as_ref();
            add("SetFilter", "", "bound.set".into(), Node::Source(set));
            add("SetFilter", "", "predicate".into(), Node::Source(predicate));
        }
        S::SetMap { expression, bounds } => {
            for (i, ast::Bound { pattern: _, set }) in bounds.iter().enumerate() {
                add("SetMap", "", format!("bounds[{i}].set"), Node::Source(set));
            }
            add("SetMap", "", "expression".into(), Node::Source(expression));
        }
        S::FunctionSet { domain, range } => {
            add("FunctionSet", "", "domain".into(), Node::Source(domain));
            add("FunctionSet", "", "range".into(), Node::Source(range));
        }
        S::Record { fields } | S::RecordSet { fields } => {
            let ctor = if matches!(node, S::Record { fields: _ }) {
                "Record"
            } else {
                "RecordSet"
            };
            for (
                i,
                ast::RecordField {
                    name: _,
                    value,
                    location: _,
                },
            ) in fields.iter().enumerate()
            {
                add(ctor, "", format!("fields[{i}].value"), Node::Source(value));
            }
        }
        S::Except { function, updates } => {
            add("Except", "", "function".into(), Node::Source(function));
            for (i, ast::ExceptUpdate { path, value }) in updates.iter().enumerate() {
                for (j, p) in path.iter().enumerate() {
                    match p {
                        ast::ExceptPath::Field { name: _ } => {}
                        ast::ExceptPath::Index { args } => {
                            for (k, v) in args.iter().enumerate() {
                                add(
                                    "Except",
                                    "",
                                    format!("updates[{i}].path[{j}].Index.args[{k}]"),
                                    Node::Source(v),
                                );
                            }
                        }
                    }
                }
                add(
                    "Except",
                    "",
                    format!("updates[{i}].value"),
                    Node::Source(value),
                );
            }
        }
        S::FunctionApplication { function, args } => {
            add(
                "FunctionApplication",
                "",
                "function".into(),
                Node::Source(function),
            );
            for (i, v) in args.iter().enumerate() {
                add(
                    "FunctionApplication",
                    "",
                    format!("args[{i}]"),
                    Node::Source(v),
                );
            }
        }
        S::FieldAccess { record, field: _ } => {
            add("FieldAccess", "", "record".into(), Node::Source(record))
        }
        S::BoxAction { action, subscript } | S::AngleAction { action, subscript } => {
            let (ctor, op) = if matches!(
                node,
                S::BoxAction {
                    action: _,
                    subscript: _
                }
            ) {
                ("BoxAction", "[A]_v")
            } else {
                ("AngleAction", "<<A>>_v")
            };
            add(ctor, op, "action".into(), Node::Source(action));
            add(ctor, op, "subscript".into(), Node::Source(subscript));
        }
        S::WeakFairness { subscript, action } | S::StrongFairness { subscript, action } => {
            let (ctor, op) = if matches!(
                node,
                S::WeakFairness {
                    subscript: _,
                    action: _
                }
            ) {
                ("WeakFairness", "WF_")
            } else {
                ("StrongFairness", "SF_")
            };
            add(ctor, op, "subscript".into(), Node::Source(subscript));
            add(ctor, op, "action".into(), Node::Source(action));
        }
        S::Label {
            name: _,
            params: _,
            body,
        } => add("Label", "::", "body".into(), Node::Source(body)),
        S::UnboundedQuantifier {
            quantifier: _,
            names: _,
            is_temporal: _,
            body,
        } => out.push(edge(
            Node::Source(body),
            "parser-only",
            "UnboundedQuantifier",
            "",
            "body",
        )),
        S::Bracket { args } => {
            for (i, v) in args.iter().enumerate() {
                out.push(edge(
                    Node::Source(v),
                    "parser-only",
                    "Bracket",
                    "",
                    format!("args[{i}]"),
                ));
            }
        }
        S::Product { factors } => {
            for (i, v) in factors.iter().enumerate() {
                out.push(edge(
                    Node::Source(v),
                    "parser-only",
                    "Product",
                    "",
                    format!("factors[{i}]"),
                ));
            }
        }
    }
    out
}

fn expression_edges(value: &I) -> Vec<Edge<'_>> {
    expression_edges_body!(value, Node, edge, iter)
}

fn step_edges(value: &ir::Step) -> Vec<Edge<'_>> {
    step_edges_body!(value, Node, edge, iter)
}

fn temporal_edges(value: &ir::TemporalFormula) -> Vec<Edge<'_>> {
    temporal_edges_body!(value, Node, edge, iter)
}

fn program_edges(value: &ir::SafetyProgram) -> Vec<Edge<'_>> {
    program_edges_body!(value, Node, edge, iter)
}

fn apply_spellings() -> Vec<(&'static str, usize)> {
    vec![
        ("~", 1),
        ("\\lnot", 1),
        ("\\neg", 1),
        ("¬", 1),
        ("-", 1),
        ("DOMAIN", 1),
        ("Cardinality", 1),
        ("Len", 1),
        ("Head", 1),
        ("Tail", 1),
        ("SUBSET", 1),
        ("UNION", 1),
        ("Seq", 1),
        ("+", 2),
        ("-", 2),
        ("*", 2),
        ("#", 2),
        ("/=", 2),
        ("≠", 2),
        ("<", 2),
        ("<=", 2),
        ("\\leq", 2),
        ("=<", 2),
        ("≤", 2),
        (">", 2),
        (">=", 2),
        ("\\geq", 2),
        ("≥", 2),
        ("=>", 2),
        ("⇒", 2),
        ("\\cup", 2),
        ("\\union", 2),
        ("∪", 2),
        ("\\cap", 2),
        ("\\intersect", 2),
        ("∩", 2),
        ("\\", 2),
        ("\\subseteq", 2),
        ("⊆", 2),
        ("\\o", 2),
        ("\\circ", 2),
        ("<=>", 2),
        ("\\equiv", 2),
        ("⇔", 2),
        ("~>", 2),
        ("..", 2),
        ("Append", 2),
        ("=", 2),
        ("\\in", 2),
        ("∈", 2),
        ("\\notin", 2),
        ("∉", 2),
        ("user call representative", 2),
        ("SubSeq", 3),
        ("ApaFoldSet", 3),
        ("ApaFoldSeqLeft", 3),
    ]
}

fn descriptors() -> Vec<Descriptor> {
    let mut values = Vec::new();
    let mut collect = |node| {
        values.extend(
            analysis_children(node)
                .into_iter()
                .map(|edge| edge.descriptor),
        );
    };
    let sources = source_samples();
    for value in &sources {
        collect(Node::Source(value));
    }
    let expressions = expression_samples();
    for value in &expressions {
        collect(Node::Expression(value));
    }
    let domain = ir::Domain::Set {
        values: vec![I::Old],
    };
    let domains = [
        domain.clone(),
        ir::Domain::Value {
            value: Box::new(I::Old),
        },
        ir::Domain::Range {
            lower: Box::new(I::Old),
            upper: Box::new(I::Old),
        },
    ];
    for value in &domains {
        collect(Node::Domain(value));
    }
    let binding = ir::Binding {
        tuple_size: None,
        slot: 0,
        domain,
    };
    collect(Node::Binding(&binding));
    let steps = step_samples(&binding);
    for value in &steps {
        collect(Node::Step(value));
    }
    let branch = ir::Branch {
        name: String::new(),
        steps: steps.clone(),
    };
    collect(Node::Branch(&branch));
    let definition = ir::Definition {
        recursive: false,
        id: 0,
        parameter_count: 0,
        capture_count: 0,
        body_kind: ir::BodyKind::Expression,
        local_scope: 0,
        body: I::Old.into(),
    };
    collect(Node::Definition(&definition));
    let action_definition = ir::Definition {
        recursive: false,
        id: 0,
        parameter_count: 0,
        capture_count: 0,
        body_kind: ir::BodyKind::Action,
        local_scope: 0,
        body: ir::DefinitionBody::Action(vec![branch.clone()]),
    };
    collect(Node::Definition(&action_definition));
    let temporal = temporal_samples(&binding);
    for value in &temporal {
        collect(Node::Temporal(value));
    }
    let predicate = || {
        vec![ir::Predicate {
            name: String::new(),
            expression: I::Old,
        }]
    };
    let program = ir::SafetyProgram {
        format: String::new(),
        version: 2,
        name: String::new(),
        variables: vec![],
        initial: vec![branch.clone()],
        next: vec![branch.clone()],
        invariants: predicate(),
        assumptions: predicate(),
        check_deadlock: false,
        symmetry: None,
        initial_properties: predicate(),
        action_properties: predicate(),
        constraints: predicate(),
        action_constraints: predicate(),
        properties: vec![ir::TemporalProperty {
            name: String::new(),
            formula: ir::TemporalFormula::Atom { expression: I::Old },
        }],
        fairness: temporal.clone(),
        definitions: Some(vec![definition.clone()]),
    };
    collect(Node::Program(&program));
    let leaf = source_leaf();
    let bound = ast::Bound {
        pattern: ast::Pattern::Name {
            name: String::new(),
        },
        set: leaf.clone(),
    };
    let definitions = [
        ast::Definition {
            name: String::new(),
            head: ast::DefinitionKind::Operator { params: vec![] },
            body: leaf.clone(),
            is_local: false,
            is_recursive: false,
            location: leaf.location,
        },
        ast::Definition {
            name: String::new(),
            head: ast::DefinitionKind::Function {
                bounds: vec![bound.clone()],
            },
            body: leaf.clone(),
            is_local: false,
            is_recursive: false,
            location: leaf.location,
        },
    ];
    for value in &definitions {
        collect(Node::SourceDefinition(value));
    }
    collect(Node::SourceBound(&bound));
    let arm = ast::CaseArm {
        condition: leaf.clone(),
        body: leaf.clone(),
    };
    collect(Node::SourceArm(&arm));
    let field = ast::RecordField {
        name: String::new(),
        value: leaf.clone(),
        location: leaf.location,
    };
    collect(Node::SourceField(&field));
    let path = ast::ExceptPath::Index {
        args: vec![leaf.clone()],
    };
    collect(Node::SourcePath(&path));
    let update = ast::ExceptUpdate {
        path: vec![path.clone()],
        value: leaf,
    };
    collect(Node::SourceUpdate(&update));
    for (space, constructor) in [
        ("source", "Number"),
        ("source", "String"),
        ("source", "Identifier"),
        ("source", "ExceptAt"),
        ("parser-only", "Decimal"),
        ("parser-only", "Integer"),
        ("parser-only", "OperatorReference"),
        ("source-wrapper", "ExceptPath.Field"),
        ("ir-expression", "data"),
        ("ir-expression", "bound"),
        ("ir-expression", "old"),
        ("ir-expression", "unmatched-case"),
        ("ir-expression", "literal"),
        ("ir-expression", "read"),
        ("ir-expression", "parameter"),
        ("ir-expression", "capture"),
    ] {
        values.push(descriptor(space, constructor, "", ""));
    }
    for (constructor, position) in [
        ("Lambda", "body"),
        ("Subexpression", "selectors[*].args[*]"),
        ("LetInstance", "instances[*]"),
        ("InstanceSubstitution", "expression"),
        ("Fairness", "enabled"),
    ] {
        values.push(descriptor("parser-only", constructor, "", position));
    }
    let originals = values.clone();
    for value in originals {
        if value.space != "source" {
            continue;
        }
        let aliases: &[&str] = match value.constructor {
            "And" => &["\\land", "∧"],
            "Or" => &["\\lor", "∨"],
            "Always" => &["□"],
            "Eventually" => &["◇"],
            "Forall" => &["\\forall", "∀"],
            "Exists" => &["\\exists", "∃"],
            "Tuple" => &["⟨...⟩"],
            "AngleAction" => &["⟨A>>_v"],
            _ => &[],
        };
        for alias in aliases {
            let mut alias_value = value.clone();
            alias_value.operator = (*alias).into();
            values.push(alias_value);
        }
    }
    values.sort_by(|a, b| {
        (
            a.space,
            a.constructor,
            a.operator.as_str(),
            a.position.as_str(),
        )
            .cmp(&(
                b.space,
                b.constructor,
                b.operator.as_str(),
                b.position.as_str(),
            ))
    });
    values.dedup_by(|a, b| {
        (a.space, a.constructor, &a.operator, &a.position)
            == (b.space, b.constructor, &b.operator, &b.position)
    });
    values
}

fn source_leaf() -> ast::Expression {
    ast::Expression {
        node: S::Number { value: 0 },
        location: ast::Location {
            start: 0,
            end: 0,
            line: 0,
            column: 0,
        },
    }
}

fn source_samples() -> Vec<ast::Expression> {
    let leaf = source_leaf();
    let one = || Box::new(leaf.clone());
    let many = || vec![leaf.clone(), leaf.clone()];
    let pattern = || ast::Pattern::Name {
        name: String::new(),
    };
    let bound = || ast::Bound {
        pattern: pattern(),
        set: leaf.clone(),
    };
    let bounds = || vec![bound(), bound()];
    let field = || ast::RecordField {
        name: String::new(),
        value: leaf.clone(),
        location: leaf.location,
    };
    let definition = || ast::Definition {
        name: String::new(),
        head: ast::DefinitionKind::Operator { params: vec![] },
        body: leaf.clone(),
        is_local: false,
        is_recursive: false,
        location: leaf.location,
    };
    let arm = || ast::CaseArm {
        condition: leaf.clone(),
        body: leaf.clone(),
    };
    let update = || ast::ExceptUpdate {
        path: vec![
            ast::ExceptPath::Index { args: many() },
            ast::ExceptPath::Index { args: many() },
        ],
        value: leaf.clone(),
    };
    let mut nodes = vec![
        S::Number { value: 0 },
        S::Integer {
            text: "9007199254740992".into(),
        },
        S::LetModule {
            definitions: vec![definition(), definition()],
            instances: vec![ast::Instance {
                name: None,
                params: vec![],
                module: String::new(),
                substitutions: vec![ast::InstanceSubstitution {
                    name: String::new(),
                    expression: leaf.clone(),
                }],
                is_local: false,
                location: leaf.location,
            }],
            recursive: vec![],
            body: one(),
        },
        S::AssumeProve {
            assumptions: vec![
                ast::ProofAssumption::Expression {
                    expression: leaf.clone(),
                },
                ast::ProofAssumption::New {
                    declaration: ast::Declaration {
                        name: String::new(),
                        arity: 0,
                        location: leaf.location,
                    },
                    level: ast::DeclarationLevel::Constant,
                    is_new: true,
                    set: Some(one()),
                },
            ],
            prove: one(),
            is_boxed_assume: false,
            is_boxed_prove: false,
        },
        S::String {
            value: String::new(),
        },
        S::Identifier {
            name: String::new(),
        },
        S::ExceptAt,
        S::Decimal {
            text: String::new(),
        },
        S::And {
            operands: many(),
            is_bulleted: false,
        },
        S::Or {
            operands: many(),
            is_bulleted: false,
        },
        S::Prime { expression: one() },
        S::Unchanged { expression: one() },
        S::Enabled { expression: one() },
        S::Always { expression: one() },
        S::Eventually { expression: one() },
        S::If {
            condition: one(),
            consequent: one(),
            alternative: one(),
        },
        S::Case {
            arms: vec![arm(), arm()],
            other: Some(one()),
        },
        S::Let {
            definitions: vec![definition(), definition()],
            body: one(),
        },
        S::Forall {
            bounds: bounds(),
            body: one(),
        },
        S::Exists {
            bounds: bounds(),
            body: one(),
        },
        S::UnboundedQuantifier {
            quantifier: String::new(),
            names: vec![],
            is_temporal: false,
            body: one(),
        },
        S::Choose {
            pattern: pattern(),
            set: Some(one()),
            body: one(),
        },
        S::SetEnumeration { elements: many() },
        S::SetFilter {
            bound: Box::new(bound()),
            predicate: one(),
        },
        S::SetMap {
            expression: one(),
            bounds: bounds(),
        },
        S::Tuple { elements: many() },
        S::FunctionConstruction {
            bounds: bounds(),
            body: one(),
        },
        S::FunctionSet {
            domain: one(),
            range: one(),
        },
        S::Record {
            fields: vec![field(), field()],
        },
        S::RecordSet {
            fields: vec![field(), field()],
        },
        S::Except {
            function: one(),
            updates: vec![update(), update()],
        },
        S::FunctionApplication {
            function: one(),
            args: many(),
        },
        S::Bracket { args: many() },
        S::FieldAccess {
            record: one(),
            field: String::new(),
        },
        S::Product { factors: many() },
        S::BoxAction {
            action: one(),
            subscript: one(),
        },
        S::AngleAction {
            action: one(),
            subscript: one(),
        },
        S::WeakFairness {
            subscript: one(),
            action: one(),
        },
        S::StrongFairness {
            subscript: one(),
            action: one(),
        },
        S::Label {
            name: String::new(),
            params: vec![],
            body: one(),
        },
        S::Lambda {
            params: vec![],
            body: one(),
        },
        S::Selection {
            expression: one(),
            selectors: vec![
                ast::Selector::Name {
                    name: String::new(),
                    args: many(),
                },
                ast::Selector::Arguments { args: many() },
            ],
        },
    ];
    nodes.extend(
        apply_spellings()
            .into_iter()
            .map(|(operator, arity)| S::Apply {
                operator: operator.into(),
                args: vec![leaf.clone(); arity],
            }),
    );
    nodes
        .into_iter()
        .map(|node| ast::Expression {
            node,
            location: leaf.location,
        })
        .collect()
}

fn expression_samples() -> Vec<I> {
    let one = || Box::new(I::Old);
    let many = || vec![I::Old];
    let domain = || ir::Domain::Set { values: many() };
    let binding = || ir::Binding {
        tuple_size: None,
        slot: 0,
        domain: domain(),
    };
    let bounds = || vec![binding()];
    vec![
        I::Domain { argument: one() },
        I::VariantTag { argument: one() },
        I::Cardinality { argument: one() },
        I::SequenceSet { argument: one() },
        I::Len { argument: one() },
        I::Head { argument: one() },
        I::Tail { argument: one() },
        I::SelectSeq {
            slot: 0,
            sequence: one(),
            body: one(),
        },
        I::SortSeq {
            slot: 0,
            sequence: one(),
            body: one(),
        },
        I::Assert {
            condition: one(),
            message: one(),
        },
        I::Powerset { argument: one() },
        I::Union { argument: one() },
        I::Not { argument: one() },
        I::Neg { argument: one() },
        I::Prime { argument: one() },
        I::Cup {
            left: one(),
            right: one(),
        },
        I::Cap {
            left: one(),
            right: one(),
        },
        I::Difference {
            left: one(),
            right: one(),
        },
        I::SubsetEq {
            left: one(),
            right: one(),
        },
        I::Append {
            left: one(),
            right: one(),
        },
        I::Concat {
            left: one(),
            right: one(),
        },
        I::FunctionSet {
            left: one(),
            right: one(),
        },
        I::FunctionIn {
            left: one(),
            right: one(),
        },
        I::SequenceIn {
            left: one(),
            right: one(),
        },
        I::Add {
            left: one(),
            right: one(),
        },
        I::Sub {
            left: one(),
            right: one(),
        },
        I::Mul {
            left: one(),
            right: one(),
        },
        I::Div {
            left: one(),
            right: one(),
        },
        I::Mod {
            left: one(),
            right: one(),
        },
        I::Pow {
            left: one(),
            right: one(),
        },
        I::Eq {
            left: one(),
            right: one(),
        },
        I::Ne {
            left: one(),
            right: one(),
        },
        I::Lt {
            left: one(),
            right: one(),
        },
        I::Le {
            left: one(),
            right: one(),
        },
        I::Gt {
            left: one(),
            right: one(),
        },
        I::Ge {
            left: one(),
            right: one(),
        },
        I::And {
            left: one(),
            right: one(),
        },
        I::Or {
            left: one(),
            right: one(),
        },
        I::Implies {
            left: one(),
            right: one(),
        },
        I::Forall {
            bounds: bounds(),
            body: one(),
        },
        I::Exists {
            bounds: bounds(),
            body: one(),
        },
        I::Function {
            bounds: bounds(),
            body: one(),
        },
        I::Filter {
            bounds: bounds(),
            body: one(),
        },
        I::MapSet {
            bounds: bounds(),
            body: one(),
        },
        I::Choose {
            binding: binding(),
            body: one(),
        },
        I::Set { values: many() },
        I::Tuple { values: many() },
        I::Record {
            fields: vec![(String::new(), I::Old)],
        },
        I::RecordSet {
            fields: vec![(String::new(), I::Old)],
        },
        I::At {
            target: one(),
            key: one(),
        },
        I::Update {
            target: one(),
            updates: vec![ir::Update {
                path: many(),
                value: I::Old,
            }],
        },
        I::Fold {
            accumulator: 0,
            initial: one(),
            collection: one(),
            order: ir::FoldOrder::SetMembers,
            body: one(),
        },
        I::Subseq {
            sequence: one(),
            from: one(),
            to: one(),
        },
        I::Enabled {
            branches: vec![ir::Branch {
                name: String::new(),
                steps: vec![],
            }],
            subscript: Some(one()),
        },
        I::If {
            condition: one(),
            consequent: one(),
            alternative: one(),
        },
        I::In {
            value: one(),
            domain: domain(),
        },
        I::Call {
            definition: 0,
            arguments: many(),
            captures: many(),
        },
        I::Frame {
            arguments: many(),
            captures: many(),
            parameter_count: 1,
            capture_count: 1,
            local_scope: 0,
            body: one(),
        },
    ]
}

fn step_samples(binding: &ir::Binding) -> Vec<ir::Step> {
    vec![
        ir::Step::Call {
            definition: 0,
            arguments: vec![I::Old],
            captures: vec![I::Old],
        },
        ir::Step::Bind {
            slot: 0,
            domain: binding.domain.clone(),
            tuple_size: None,
        },
        ir::Step::Guard { expression: I::Old },
        ir::Step::Assign {
            slot: 0,
            value: I::Old,
        },
        ir::Step::Choose {
            slot: 0,
            domain: binding.domain.clone(),
        },
        ir::Step::Frame {
            arguments: vec![I::Old],
            captures: vec![I::Old],
            parameter_count: 1,
            capture_count: 1,
            local_scope: 0,
            branches: vec![ir::Branch {
                name: String::new(),
                steps: vec![],
            }],
        },
    ]
}

fn temporal_samples(binding: &ir::Binding) -> Vec<ir::TemporalFormula> {
    use ir::TemporalFormula as T;
    let one = || Box::new(T::Atom { expression: I::Old });
    vec![
        T::Call {
            definition: 0,
            arguments: vec![I::Old],
            captures: vec![I::Old],
        },
        T::Atom { expression: I::Old },
        T::Forall {
            bounds: vec![binding.clone()],
            formula: one(),
        },
        T::Exists {
            bounds: vec![binding.clone()],
            formula: one(),
        },
        T::Not { formula: one() },
        T::Always { formula: one() },
        T::Eventually { formula: one() },
        T::And {
            left: one(),
            right: one(),
        },
        T::Or {
            left: one(),
            right: one(),
        },
        T::Frame {
            arguments: vec![I::Old],
            captures: vec![I::Old],
            parameter_count: 1,
            capture_count: 1,
            local_scope: 0,
            formula: one(),
        },
    ]
}

pub fn analysis_roles() -> Value {
    Value::Array(descriptors().into_iter().map(|d|json!({"space":d.space,"constructor":d.constructor,"operatorClass":d.operator,"position":d.position,"role":d.role,"scope":d.scope,"forwarding":{"action":d.action,"membership":d.membership}})).collect())
}

pub(crate) enum NodeMut<'a> {
    Expression(&'a mut I),
    Domain(&'a mut ir::Domain),
    Binding(&'a mut ir::Binding),
    Branch(&'a mut ir::Branch),
    Step(&'a mut ir::Step),
    Definition(&'a mut ir::Definition),
    Body(&'a mut ir::DefinitionBody),
    Temporal(&'a mut ir::TemporalFormula),
    Program(&'a mut ir::SafetyProgram),
    Call(&'a mut usize),
}

impl NodeMut<'_> {
    pub(crate) fn view(&self) -> Node<'_> {
        match self {
            Self::Expression(value) => Node::Expression(value),
            Self::Domain(value) => Node::Domain(value),
            Self::Binding(value) => Node::Binding(value),
            Self::Branch(value) => Node::Branch(value),
            Self::Step(value) => Node::Step(value),
            Self::Definition(value) => Node::Definition(value),
            Self::Body(value) => Node::Body(value),
            Self::Temporal(value) => Node::Temporal(value),
            Self::Program(value) => Node::Program(value),
            Self::Call(value) => Node::Call(value),
        }
    }
    pub(crate) fn reborrow(&mut self) -> NodeMut<'_> {
        match self {
            Self::Expression(value) => NodeMut::Expression(value),
            Self::Domain(value) => NodeMut::Domain(value),
            Self::Binding(value) => NodeMut::Binding(value),
            Self::Branch(value) => NodeMut::Branch(value),
            Self::Step(value) => NodeMut::Step(value),
            Self::Definition(value) => NodeMut::Definition(value),
            Self::Body(value) => NodeMut::Body(value),
            Self::Temporal(value) => NodeMut::Temporal(value),
            Self::Program(value) => NodeMut::Program(value),
            Self::Call(value) => NodeMut::Call(value),
        }
    }
}

pub(crate) struct EdgeMut<'a> {
    pub node: NodeMut<'a>,
    pub descriptor: Descriptor,
}
fn edge_mut<'a>(
    node: NodeMut<'a>,
    space: &'static str,
    constructor: &'static str,
    operator: &str,
    position: impl AsRef<str>,
) -> EdgeMut<'a> {
    EdgeMut {
        node,
        descriptor: descriptor(space, constructor, operator, position.as_ref()),
    }
}
pub(crate) fn analysis_children_mut(node: NodeMut<'_>) -> Vec<EdgeMut<'_>> {
    match node {
        NodeMut::Expression(value) => expression_edges_body!(value, NodeMut, edge_mut, iter_mut),
        NodeMut::Step(value) => step_edges_body!(value, NodeMut, edge_mut, iter_mut),
        NodeMut::Temporal(value) => temporal_edges_body!(value, NodeMut, edge_mut, iter_mut),
        NodeMut::Program(value) => program_edges_body!(value, NodeMut, edge_mut, iter_mut),
        NodeMut::Domain(value) => ir_wrapper_edges!(Domain, value, NodeMut, edge_mut, iter_mut),
        NodeMut::Binding(value) => ir_wrapper_edges!(Binding, value, NodeMut, edge_mut, iter_mut),
        NodeMut::Branch(value) => ir_wrapper_edges!(Branch, value, NodeMut, edge_mut, iter_mut),
        NodeMut::Body(body) => match body {
            ir::DefinitionBody::Temporal(body) => vec![edge_mut(
                NodeMut::Temporal(body),
                "ir-body",
                "temporal",
                "",
                "body",
            )],
            ir::DefinitionBody::Expression(body) => vec![edge_mut(
                NodeMut::Expression(body),
                "ir-body",
                "expression",
                "",
                "body",
            )],
            ir::DefinitionBody::Action(branches) => branches
                .iter_mut()
                .map(|branch| edge_mut(NodeMut::Branch(branch), "ir-body", "action", "", "body[*]"))
                .collect(),
        },
        NodeMut::Definition(value) => {
            ir_wrapper_edges!(Definition, value, NodeMut, edge_mut, iter_mut)
        }
        NodeMut::Call(value) => ir_wrapper_edges!(Call, value, NodeMut, edge_mut, iter_mut),
    }
}

pub(crate) fn walk_mut(
    mut node: NodeMut<'_>,
    scoped: bool,
    transform: &mut impl FnMut(NodeMut<'_>),
) {
    for edge in analysis_children_mut(node.reborrow()) {
        if !scoped || edge.descriptor.scope != "frame" {
            walk_mut(edge.node, scoped, transform);
        }
    }
    transform(node);
}
