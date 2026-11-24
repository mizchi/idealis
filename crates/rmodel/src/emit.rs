//! The two lowerings (plan 0091). They share only the checked syntax tree,
//! never each other's output.
//!
//! Alloy: every relation is a field of `one sig World`; `init` and facts
//! hold in the first state; each action is a pred whose unassigned vars are
//! unchanged; every step is an action or a stutter. Invariants are
//! `check { always f }`, reachability is `run { eventually f }`.
//!
//! TLA+: atoms are strings, sets are sets and binary relations are sets of
//! pairs. Static relations are variables chosen in Init from `SUBSET` and
//! never changed, so the explicit search enumerates the configurations.
//! Every relation is typed and has its multiplicity in every state.
use crate::Error;
use crate::syntax::{
    Arith, Check, CheckKind, Compare, Expr, Formula, INT, Int, IntCompare, Model, Mult, Quantifier,
    SetOp, Shape,
};
use std::collections::BTreeMap;

/// The Alloy command label of a check.
pub fn alloy_label(check: &str) -> String {
    format!("c_{check}")
}

/// Bound names: model name to emitted name.
type Scope = BTreeMap<String, String>;

pub fn to_alloy(model: &Model) -> String {
    let mut out = String::new();
    // A sum sort is an abstract signature its children extend (plan 0100).
    let extends = |sort: &str| {
        model
            .parent(sort)
            .map_or(String::new(), |sum| format!(" extends s_{sum}"))
    };
    for (name, _) in &model.sums {
        out.push_str(&format!("abstract sig s_{name} {{}}\n"));
    }
    for sig in &model.sigs {
        out.push_str(&format!("sig s_{sig}{} {{}}\n", extends(sig)));
    }
    for (name, members) in &model.enums {
        let members: Vec<String> = members.iter().map(|member| format!("e_{member}")).collect();
        out.push_str(&format!(
            "abstract sig s_{name}{} {{}}\none sig {} extends s_{name} {{}}\n",
            extends(name),
            members.join(", ")
        ));
    }
    let fields: Vec<String> = model
        .relations
        .iter()
        .map(|relation| {
            // A var's multiplicity is a formula, so breaking it is a
            // reachable violation rather than a declaration (plan 0099).
            let mult = |mult: Mult| if relation.var { Mult::Set } else { mult };
            let shape = match &relation.shape {
                Shape::Unary { sig, mult: m } => format!("{} s_{sig}", alloy_mult(mult(*m))),
                Shape::Binary { from, to, mult: m } => {
                    format!("s_{from} -> {} s_{to}", alloy_mult(mult(*m)))
                }
            };
            let var = if relation.var { "var " } else { "" };
            format!("  {var}r_{}: {shape}", relation.name)
        })
        .collect();
    out.push_str(&format!("one sig World {{\n{}\n}}\n", fields.join(",\n")));
    let empty = Scope::new();
    let declared = alloy_declarations(model);
    if let Some(declared) = &declared {
        out.push_str(&format!("fact {{ {declared} }}\n"));
    }
    for formula in model.facts.iter().chain(&model.init) {
        out.push_str(&format!(
            "fact {{ {} }}\n",
            alloy_formula(model, formula, &empty)
        ));
    }
    let frame = |assigned: &[&str]| -> Vec<String> {
        model
            .vars()
            .filter(|var| !assigned.contains(&var.name.as_str()))
            .map(|var| format!("(World.r_{0})' = (World.r_{0})", var.name))
            .collect()
    };
    let mut steps = Vec::new();
    for action in &model.actions {
        let scope: Scope = action
            .params
            .iter()
            .map(|(param, _)| (param.clone(), format!("p_{param}")))
            .collect();
        let mut body: Vec<String> = action
            .guards
            .iter()
            .map(|guard| alloy_formula(model, guard, &scope))
            .collect();
        for (var, value) in &action.assignments {
            let pair = matches!(
                model.relation(var).map(|relation| &relation.shape),
                Some(Shape::Binary { .. })
            );
            body.push(format!(
                "(World.r_{var})' = {}",
                alloy_as(model, value, &scope, pair)
            ));
        }
        let assigned: Vec<&str> = action
            .assignments
            .iter()
            .map(|(var, _)| var.as_str())
            .collect();
        body.extend(frame(&assigned));
        let declarations: Vec<String> = action
            .params
            .iter()
            .map(|(param, sig)| format!("p_{param}: s_{sig}"))
            .collect();
        if declarations.is_empty() {
            out.push_str(&format!(
                "pred a_{} {{\n  {}\n}}\n",
                action.name,
                body.join("\n  ")
            ));
            steps.push(format!("a_{}", action.name));
        } else {
            out.push_str(&format!(
                "pred a_{}[{}] {{\n  {}\n}}\n",
                action.name,
                declarations.join(", "),
                body.join("\n  ")
            ));
            let arguments: Vec<String> = action
                .params
                .iter()
                .map(|(param, _)| format!("p_{param}"))
                .collect();
            steps.push(format!(
                "(some {} | a_{}[{}])",
                declarations.join(", "),
                action.name,
                arguments.join(", ")
            ));
        }
    }
    let stutter = frame(&[]);
    steps.push(if stutter.is_empty() {
        "(no none)".to_owned()
    } else {
        format!("({})", stutter.join(" and "))
    });
    match &declared {
        None => out.push_str(&format!("fact {{ always {{ {} }} }}\n", steps.join(" or "))),
        // A state that breaks a multiplicity stays as it is.
        Some(declared) => out.push_str(&format!(
            "fact {{ always {{ ({declared} implies ({})) and ((not {declared}) implies {}) }} }}\n",
            steps.join(" or "),
            steps.last().expect("stutter")
        )),
    }
    let scope: Vec<String> = model
        .scope
        .sizes
        .iter()
        .map(|(sig, size)| format!("exactly {size} s_{sig}"))
        .chain(
            model
                .enums
                .iter()
                .map(|(name, members)| format!("exactly {} s_{name}", members.len())),
        )
        .chain(model.sums.iter().map(|(name, children)| {
            let size: usize = children
                .iter()
                .map(|child| {
                    model.scope.sizes.get(child).copied().unwrap_or_else(|| {
                        model
                            .enums
                            .iter()
                            .find(|(e, _)| e == child)
                            .map_or(0, |(_, members)| members.len())
                    })
                })
                .sum();
            format!("exactly {size} s_{name}")
        }))
        .chain([
            // Wide enough for any count of atoms in scope (review of #102).
            format!(
                "{} Int",
                // Only a model that counts needs more than 4 bits; wider
                // integers would crowd ralloy's 64-atom universe (issue #106).
                if !crate::syntax::uses_counts(model) || crate::syntax::atoms(model) <= 7 {
                    4
                } else {
                    5
                }
            ),
            format!("{} steps", model.scope.steps),
        ])
        .collect();
    for check in &model.checks {
        let formula = alloy_formula(model, &check.formula, &empty);
        let formula = match (&declared, check.kind) {
            (None, _) => formula,
            (Some(declared), CheckKind::Invariant) => format!("({declared} implies {formula})"),
            (Some(declared), CheckKind::Reachable) => format!("({declared} and {formula})"),
        };
        let (kind, temporal) = match check.kind {
            CheckKind::Invariant => ("check", "always"),
            CheckKind::Reachable => ("run", "eventually"),
        };
        out.push_str(&format!(
            "{}: {kind} {{ {temporal} {{ {formula} }} }} for {}\n",
            alloy_label(&check.name),
            scope.join(", ")
        ));
    }
    out.push_str(&format!(
        "{}: check {{ always {{ {} }} }} for {}\n",
        alloy_label(DECLARATIONS),
        declared.as_deref().unwrap_or("(no none)"),
        scope.join(", ")
    ));
    out
}

/// The name of the built-in check that no transition breaks a var's
/// multiplicity (plan 0099).
pub const DECLARATIONS: &str = "Declarations";
/// The built-in check that every state reached keeps its integers within
/// the scope's `Int = lo..hi` (plan 0103). When it fails, the search is not
/// complete and a check without a witness is unknown.
pub const SCOPE: &str = "Scope";

/// Every var's multiplicity as one Alloy formula, or `None` when no var has
/// one.
fn alloy_declarations(model: &Model) -> Option<String> {
    let parts: Vec<String> = model
        .vars()
        .filter_map(|relation| match &relation.shape {
            Shape::Unary {
                mult: Mult::Set, ..
            }
            | Shape::Binary {
                mult: Mult::Set, ..
            } => None,
            Shape::Unary { mult, .. } => Some(format!(
                "({} (World.r_{}))",
                alloy_mult(*mult),
                relation.name
            )),
            Shape::Binary { from, mult, .. } => Some(format!(
                "(all m_{0}: s_{from} | {1} (m_{0}.(World.r_{0})))",
                relation.name,
                alloy_mult(*mult)
            )),
        })
        .collect();
    (!parts.is_empty()).then(|| format!("({})", parts.join(" and ")))
}

fn alloy_mult(mult: Mult) -> &'static str {
    match mult {
        Mult::Set => "set",
        Mult::Lone => "lone",
        Mult::One => "one",
        Mult::Some => "some",
    }
}

/// Whether the Alloy lowering can express the model: Alloy integers wrap
/// at their bit width, and the value semantics of records has no lowering
/// yet (plan 0103).
pub fn alloy_supports(model: &Model) -> Result<(), String> {
    if model.scope.ints.is_some() {
        return Err("integer values: Alloy integers wrap at their bit width".into());
    }
    if !model.records.is_empty() {
        return Err("records: no Alloy lowering of value semantics".into());
    }
    Ok(())
}

const VALUES: &str = "alloy_supports rejects models with values";

/// Whether an expression denotes a binary relation.
fn is_pair(model: &Model, expression: &Expr, scope: &Scope) -> bool {
    match expression {
        Expr::None => false,
        Expr::Name(name) => {
            !scope.contains_key(name)
                && matches!(
                    model.relation(name).map(|relation| &relation.shape),
                    Some(Shape::Binary { .. })
                )
        }
        Expr::Binary(SetOp::Product, _, _) => true,
        Expr::Binary(SetOp::Join, left, right) => {
            is_pair(model, left, scope) && is_pair(model, right, scope)
        }
        Expr::Binary(_, left, right) => is_pair(model, left, scope) || is_pair(model, right, scope),
        Expr::Literal(_) | Expr::Count(_) | Expr::Arith(..) | Expr::Record(..) => false,
    }
}

/// `none` as the empty relation of the arity it stands for: Alloy's `none`
/// is unary (review of #102).
fn alloy_as(model: &Model, expression: &Expr, scope: &Scope, pair: bool) -> String {
    match expression {
        Expr::None if pair => "(none -> none)".into(),
        // The binary context reaches the operands of a set operation, so
        // `none + none` is binary too (issue #106).
        Expr::Binary(
            operator @ (SetOp::Union | SetOp::Difference | SetOp::Intersection),
            left,
            right,
        ) if pair => {
            let symbol = match operator {
                SetOp::Union => "+",
                SetOp::Difference => "-",
                _ => "&",
            };
            format!(
                "({} {symbol} {})",
                alloy_as(model, left, scope, true),
                alloy_as(model, right, scope, true)
            )
        }
        _ => alloy_expression(model, expression, scope),
    }
}

fn alloy_expression(model: &Model, expression: &Expr, scope: &Scope) -> String {
    match expression {
        Expr::None => "none".into(),
        Expr::Name(name) => {
            if let Some(bound) = scope.get(name) {
                bound.clone()
            } else if model.relation(name).is_some() {
                format!("(World.r_{name})")
            } else if model.member(name).is_some() {
                format!("e_{name}")
            } else {
                format!("s_{name}")
            }
        }
        Expr::Binary(operator, left, right) => {
            let symbol = match operator {
                SetOp::Union => "+",
                SetOp::Difference => "-",
                SetOp::Intersection => "&",
                SetOp::Product => "->",
                SetOp::Join => ".",
                SetOp::Multiply => unreachable!("{VALUES}"),
            };
            let pair = matches!(
                operator,
                SetOp::Union | SetOp::Difference | SetOp::Intersection
            ) && (is_pair(model, left, scope) || is_pair(model, right, scope));
            format!(
                "({} {symbol} {})",
                alloy_as(model, left, scope, pair),
                alloy_as(model, right, scope, pair)
            )
        }
        Expr::Literal(_) | Expr::Count(_) | Expr::Arith(..) | Expr::Record(..) => {
            unreachable!("{VALUES}")
        }
    }
}

fn alloy_formula(model: &Model, formula: &Formula, scope: &Scope) -> String {
    let expression = |expression: &Expr| alloy_expression(model, expression, scope);
    match formula {
        Formula::Compare(operator, left, right) => {
            let pair = is_pair(model, left, scope) || is_pair(model, right, scope);
            let (left, right) = (
                alloy_as(model, left, scope, pair),
                alloy_as(model, right, scope, pair),
            );
            match operator {
                Compare::In => format!("({left} in {right})"),
                Compare::NotIn => format!("(not ({left} in {right}))"),
                Compare::Equal => format!("({left} = {right})"),
                Compare::NotEqual => format!("(not ({left} = {right}))"),
            }
        }
        Formula::Ints(operator, left, right) => {
            let int = |int: &Int| match int {
                Int::Count(counted) => format!("#{}", expression(counted)),
                Int::Literal(value) => value.to_string(),
                Int::Value(_) => unreachable!("{VALUES}"),
            };
            let symbol = match operator {
                IntCompare::Less => "<",
                IntCompare::LessEqual => "<=",
                IntCompare::Greater => ">",
                IntCompare::GreaterEqual => ">=",
                IntCompare::Equal => "=",
                IntCompare::NotEqual => "!=",
            };
            format!("({} {symbol} {})", int(left), int(right))
        }
        Formula::Mult(mult, counted) => format!("({} {})", alloy_mult(*mult), expression(counted)),
        Formula::Empty(counted) => format!("(no {})", expression(counted)),
        Formula::Not(inner) => format!("(not {})", alloy_formula(model, inner, scope)),
        Formula::And(parts) if parts.is_empty() => "(no none)".into(),
        Formula::And(parts) => format!(
            "({})",
            parts
                .iter()
                .map(|part| alloy_formula(model, part, scope))
                .collect::<Vec<_>>()
                .join(" and ")
        ),
        Formula::Or(left, right) => format!(
            "({} or {})",
            alloy_formula(model, left, scope),
            alloy_formula(model, right, scope)
        ),
        Formula::Implies(left, right) => format!(
            "({} implies {})",
            alloy_formula(model, left, scope),
            alloy_formula(model, right, scope)
        ),
        Formula::Quantified(quantifier, variable, sig, body) => {
            let mut inner = scope.clone();
            inner.insert(variable.clone(), format!("v_{variable}"));
            let word = match quantifier {
                Quantifier::All => "all",
                Quantifier::Some => "some",
            };
            format!(
                "({word} v_{variable}: s_{sig} | {})",
                alloy_formula(model, body, &inner)
            )
        }
    }
}

/// A TLA+ module and TLC configuration checking one check of the model.
pub fn to_tla(model: &Model, check: &str) -> Result<(String, String), Error> {
    let target = if check == DECLARATIONS {
        Target::Declarations
    } else if check == SCOPE {
        Target::Scope
    } else {
        Target::Check(
            model
                .checks
                .iter()
                .find(|candidate| candidate.name == check)
                .ok_or_else(|| Error::kind(format!("no check {check}")))?,
        )
    };
    let mut emitter = Tla { model, fresh: 0 };
    Ok((
        emitter.module(target),
        format!("INIT Init\nNEXT Next\nINVARIANT {}\n", alloy_label(check)),
    ))
}

enum Target<'a> {
    Declarations,
    Scope,
    Check(&'a Check),
}

struct Tla<'a> {
    model: &'a Model,
    fresh: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arity {
    Any,
    Set,
    Pair,
}
impl Tla<'_> {
    fn module(&mut self, check: Target<'_>) -> String {
        let model = self.model;
        let mut out = format!(
            "---- MODULE {} ----\nEXTENDS Integers, FiniteSets\n",
            model.name
        );
        let variables: Vec<String> = model
            .relations
            .iter()
            .map(|relation| format!("r_{}", relation.name))
            .collect();
        out.push_str(&format!("VARIABLES {}\n", variables.join(", ")));
        for sig in &model.sigs {
            let atoms: Vec<String> = (0..model.scope.sizes[sig])
                .map(|index| format!("\"{sig}{index}\""))
                .collect();
            out.push_str(&format!("s_{sig} == {{{}}}\n", atoms.join(", ")));
        }
        for (name, members) in &model.enums {
            // "Enum.Member": generated sig atoms have no dot, so an enum
            // member never equals one (review of #103).
            let atoms: Vec<String> = members
                .iter()
                .map(|member| format!("\"{name}.{member}\""))
                .collect();
            out.push_str(&format!("s_{name} == {{{}}}\n", atoms.join(", ")));
        }
        for (name, children) in &model.sums {
            let parts: Vec<String> = children.iter().map(|child| format!("s_{child}")).collect();
            out.push_str(&format!("s_{name} == {}\n", parts.join(" \\cup ")));
        }
        // Values (plan 0103): the scope's integers, and records over their
        // fields' sorts. Both bound only the enumeration; arithmetic leaves
        // them, which InScope detects.
        if let Some((low, high)) = model.scope.ints {
            out.push_str(&format!("s_{INT} == {low}..{high}\n"));
        }
        for record in &model.records {
            let fields: Vec<String> = record
                .fields
                .iter()
                .map(|(field, sort)| format!("f_{field} : s_{sort}"))
                .collect();
            out.push_str(&format!("s_{} == [{}]\n", record.name, fields.join(", ")));
        }
        for relation in &model.relations {
            let (domain, shape) = match &relation.shape {
                Shape::Unary { sig, mult } => (
                    format!("s_{sig}"),
                    match mult {
                        Mult::Set => "TRUE".to_owned(),
                        Mult::Lone => "Cardinality(V) =< 1".to_owned(),
                        Mult::One => "Cardinality(V) = 1".to_owned(),
                        Mult::Some => "Cardinality(V) >= 1".to_owned(),
                    },
                ),
                Shape::Binary { from, to, mult } => {
                    let count = "Cardinality({t \\in V : t[1] = a})";
                    let bound = match mult {
                        Mult::Set => None,
                        Mult::Lone => Some(format!("{count} =< 1")),
                        Mult::One => Some(format!("{count} = 1")),
                        Mult::Some => Some(format!("{count} >= 1")),
                    };
                    (
                        product(&format!("s_{from}"), &format!("s_{to}"), 0),
                        bound.map_or("TRUE".to_owned(), |bound| {
                            format!("\\A a \\in s_{from} : {bound}")
                        }),
                    )
                }
            };
            out.push_str(&format!("D_{} == {domain}\n", relation.name));
            // Values are typed statically; their range is InScope's.
            let target = match &relation.shape {
                Shape::Unary { sig, .. } => sig,
                Shape::Binary { to, .. } => to,
            };
            if model.is_value(target) {
                out.push_str(&format!("T_{}(V) == {shape}\n", relation.name));
            } else {
                out.push_str(&format!(
                    "T_{}(V) == V \\subseteq D_{} /\\ {shape}\n",
                    relation.name, relation.name
                ));
            }
        }
        let empty = BTreeMap::new();
        let mut init: Vec<String> = model
            .relations
            .iter()
            .map(|relation| format!("r_{0} \\in SUBSET D_{0}", relation.name))
            .collect();
        init.extend(
            model
                .relations
                .iter()
                .map(|relation| format!("T_{0}(r_{0})", relation.name)),
        );
        for formula in model.facts.iter().chain(&model.init) {
            init.push(self.formula(formula, &empty));
        }
        out.push_str(&format!("Init == {}\n", init.join(" /\\ ")));
        let mut steps = Vec::new();
        for action in &model.actions {
            let scope: BTreeMap<String, String> = action
                .params
                .iter()
                .map(|(param, _)| (param.clone(), format!("{{p_{param}}}")))
                .collect();
            let mut body = Vec::new();
            for (var, value) in &action.assignments {
                body.push(format!("r_{var}' = {}", self.expression(value, &scope).0));
            }
            let unchanged: Vec<String> = model
                .relations
                .iter()
                .filter(|relation| {
                    !action
                        .assignments
                        .iter()
                        .any(|(var, _)| *var == relation.name)
                })
                .map(|relation| format!("r_{}", relation.name))
                .collect();
            if !unchanged.is_empty() {
                body.push(format!("UNCHANGED <<{}>>", unchanged.join(", ")));
            }
            for guard in &action.guards {
                body.push(self.formula(guard, &scope));
            }

            let params: Vec<String> = action
                .params
                .iter()
                .map(|(param, _)| format!("p_{param}"))
                .collect();
            let head = if params.is_empty() {
                format!("a_{}", action.name)
            } else {
                format!("a_{}({})", action.name, params.join(", "))
            };
            out.push_str(&format!("{head} == {}\n", body.join(" /\\ ")));
            let mut step = head;
            for (param, sig) in action.params.iter().rev() {
                step = format!("(\\E p_{param} \\in s_{sig} : {step})");
            }
            steps.push(step);
        }
        steps.push(format!("UNCHANGED <<{}>>", variables.join(", ")));
        // Only a state within the var multiplicities steps; checks look at
        // those states, and Declarations at the multiplicities themselves
        // (plan 0099).
        let declared: Vec<String> = model
            .vars()
            .map(|relation| format!("T_{0}(r_{0})", relation.name))
            .collect();
        let declared = if declared.is_empty() {
            "TRUE".to_owned()
        } else {
            declared.join(" /\\ ")
        };
        out.push_str(&format!("TypeOK == {declared}\n"));
        // A state with an integer outside the scope is not explored further
        // (plan 0103). Models without values keep their earlier output.
        let valid = if model.has_values() {
            let ranged: Vec<String> = model
                .vars()
                .filter(|relation| {
                    let target = match &relation.shape {
                        Shape::Unary { sig, .. } => sig,
                        Shape::Binary { to, .. } => to,
                    };
                    model.is_value(target)
                })
                .map(|relation| format!("r_{0} \\subseteq D_{0}", relation.name))
                .collect();
            let ranged = if ranged.is_empty() {
                "TRUE".to_owned()
            } else {
                ranged.join(" /\\ ")
            };
            out.push_str(&format!("InScope == {ranged}\n"));
            "(TypeOK /\\ InScope)"
        } else {
            "TypeOK"
        };
        // A state that breaks a multiplicity stays as it is, like the Alloy
        // lowering, rather than deadlocking.
        out.push_str(&format!(
            "Next == ({valid} /\\ ({})) \\/ (~{valid} /\\ UNCHANGED <<{}>>)\n",
            steps.join(" \\/ "),
            variables.join(", ")
        ));
        let (name, body) = match check {
            Target::Declarations if model.has_values() => {
                (DECLARATIONS, "(InScope => TypeOK)".to_owned())
            }
            Target::Declarations => (DECLARATIONS, "TypeOK".to_owned()),
            Target::Scope if model.has_values() => (SCOPE, "InScope".to_owned()),
            Target::Scope => (SCOPE, "TRUE".to_owned()),
            Target::Check(check) => {
                let formula = self.formula(&check.formula, &empty);
                let body = match check.kind {
                    CheckKind::Invariant => format!("({valid} => {formula})"),
                    CheckKind::Reachable => format!("~({valid} /\\ {formula})"),
                };
                (check.name.as_str(), body)
            }
        };
        out.push_str(&format!("{} == {body}\n====\n", alloy_label(name)));
        out
    }
    fn next(&mut self) -> usize {
        self.fresh += 1;
        self.fresh
    }
    fn expression(
        &mut self,
        expression: &Expr,
        scope: &BTreeMap<String, String>,
    ) -> (String, Arity) {
        match expression {
            Expr::None => ("{}".into(), Arity::Any),
            Expr::Name(name) => {
                if let Some(bound) = scope.get(name) {
                    (bound.clone(), Arity::Set)
                } else if let Some(relation) = self.model.relation(name) {
                    let arity = match relation.shape {
                        Shape::Unary { .. } => Arity::Set,
                        Shape::Binary { .. } => Arity::Pair,
                    };
                    (format!("r_{name}"), arity)
                } else if let Some(enumeration) = self.model.member(name) {
                    (format!("{{\"{enumeration}.{name}\"}}"), Arity::Set)
                } else {
                    (format!("s_{name}"), Arity::Set)
                }
            }
            // Values (plan 0103): sets of one integer or record.
            Expr::Literal(value) => (format!("{{{value}}}"), Arity::Set),
            Expr::Count(counted) => (
                format!("{{Cardinality({})}}", self.expression(counted, scope).0),
                Arity::Set,
            ),
            Expr::Arith(operator, left, right) => {
                let id = self.next();
                let (left, _) = self.expression(left, scope);
                let (right, _) = self.expression(right, scope);
                let symbol = match operator {
                    Arith::Add => "+",
                    Arith::Sub => "-",
                    Arith::Mul => "*",
                };
                (
                    pairwise(&format!("x{id} {symbol} y{id}"), &left, &right, id),
                    Arity::Set,
                )
            }
            Expr::Record(name, values) => {
                let id = self.next();
                let record = self.model.record(name).expect("a declared record");
                let mut fields = Vec::new();
                let mut ranges = Vec::new();
                for (index, ((field, _), value)) in record.fields.iter().zip(values).enumerate() {
                    let (value, _) = self.expression(value, scope);
                    fields.push(format!("f_{field} |-> x{id}_{index}"));
                    ranges.push((format!("x{id}_{index}"), value));
                }
                // Nested maps of one binder each, innermost the last field.
                let mut set = format!("{{[{}]}}", fields.join(", "));
                for (index, (binder, range)) in ranges.iter().enumerate().rev() {
                    set = if index == 0 && ranges.len() == 1 {
                        format!("{{[{}] : {binder} \\in {range}}}", fields.join(", "))
                    } else {
                        format!("UNION {{{set} : {binder} \\in {range}}}")
                    };
                }
                (set, Arity::Set)
            }
            Expr::Binary(SetOp::Join, left, right)
                if matches!(right.as_ref(), Expr::Name(name)
                    if !scope.contains_key(name) && self.model.field(name).is_some()) =>
            {
                let Expr::Name(field) = right.as_ref() else {
                    unreachable!()
                };
                let id = self.next();
                let (left, _) = self.expression(left, scope);
                (
                    format!("{{t{id}.f_{field} : t{id} \\in {left}}}"),
                    Arity::Set,
                )
            }
            Expr::Binary(operator, left, right) => {
                let (left, left_arity) = self.expression(left, scope);
                let (right, right_arity) = self.expression(right, scope);
                match operator {
                    SetOp::Union => (
                        format!("({left} \\cup {right})"),
                        pick(left_arity, right_arity),
                    ),
                    SetOp::Difference => (
                        format!("({left} \\ {right})"),
                        pick(left_arity, right_arity),
                    ),
                    SetOp::Intersection => (
                        format!("({left} \\cap {right})"),
                        pick(left_arity, right_arity),
                    ),
                    SetOp::Product => (product(&left, &right, self.next()), Arity::Pair),
                    SetOp::Multiply => unreachable!("the checker rewrites * to arithmetic"),
                    SetOp::Join => {
                        let id = self.next();
                        let (t, u) = (format!("t{id}"), format!("u{id}"));
                        match (left_arity, right_arity) {
                            (Arity::Pair, Arity::Pair) => (
                                format!(
                                    "UNION {{{{<<{t}[1], {u}[2]>> : {u} \\in {{w{id} \\in {right} : w{id}[1] = {t}[2]}}}} : {t} \\in {left}}}"
                                ),
                                Arity::Pair,
                            ),
                            (Arity::Pair, _) => (
                                format!(
                                    "{{{t}[1] : {t} \\in {{{u} \\in {left} : {u}[2] \\in {right}}}}}"
                                ),
                                Arity::Set,
                            ),
                            _ => (
                                format!(
                                    "{{{t}[2] : {t} \\in {{{u} \\in {right} : {u}[1] \\in {left}}}}}"
                                ),
                                Arity::Set,
                            ),
                        }
                    }
                }
            }
        }
    }
    fn formula(&mut self, formula: &Formula, scope: &BTreeMap<String, String>) -> String {
        match formula {
            Formula::Compare(operator, left, right) => {
                let (left, _) = self.expression(left, scope);
                let (right, _) = self.expression(right, scope);
                match operator {
                    Compare::In => format!("({left} \\subseteq {right})"),
                    Compare::NotIn => format!("(~({left} \\subseteq {right}))"),
                    Compare::Equal => format!("({left} = {right})"),
                    Compare::NotEqual => format!("({left} # {right})"),
                }
            }
            Formula::Ints(operator, left, right) => {
                let mut int = |int: &Int| match int {
                    Int::Count(counted) => {
                        format!("Cardinality({})", self.expression(counted, scope).0)
                    }
                    Int::Literal(value) => value.to_string(),
                    // One integer, by the checker (plan 0103).
                    Int::Value(value) => {
                        format!("(CHOOSE x \\in {} : TRUE)", self.expression(value, scope).0)
                    }
                };
                let (left, right) = (int(left), int(right));
                let symbol = match operator {
                    IntCompare::Less => "<",
                    IntCompare::LessEqual => "=<",
                    IntCompare::Greater => ">",
                    IntCompare::GreaterEqual => ">=",
                    IntCompare::Equal => "=",
                    IntCompare::NotEqual => "#",
                };
                format!("({left} {symbol} {right})")
            }
            Formula::Mult(mult, counted) => {
                let (set, _) = self.expression(counted, scope);
                match mult {
                    Mult::Some => format!("({set} # {{}})"),
                    Mult::One => format!("(Cardinality({set}) = 1)"),
                    Mult::Lone => format!("(Cardinality({set}) =< 1)"),
                    Mult::Set => "TRUE".into(),
                }
            }
            Formula::Empty(counted) => format!("({} = {{}})", self.expression(counted, scope).0),
            Formula::Not(inner) => format!("(~{})", self.formula(inner, scope)),
            Formula::And(parts) if parts.is_empty() => "TRUE".into(),
            Formula::And(parts) => {
                let parts: Vec<String> =
                    parts.iter().map(|part| self.formula(part, scope)).collect();
                format!("({})", parts.join(" /\\ "))
            }
            Formula::Or(left, right) => {
                format!(
                    "({} \\/ {})",
                    self.formula(left, scope),
                    self.formula(right, scope)
                )
            }
            Formula::Implies(left, right) => {
                format!(
                    "({} => {})",
                    self.formula(left, scope),
                    self.formula(right, scope)
                )
            }
            Formula::Quantified(quantifier, variable, sig, body) => {
                let mut inner = scope.clone();
                inner.insert(variable.clone(), format!("{{v_{variable}}}"));
                let symbol = match quantifier {
                    Quantifier::All => "\\A",
                    Quantifier::Some => "\\E",
                };
                format!(
                    "({symbol} v_{variable} \\in s_{sig} : {})",
                    self.formula(body, &inner)
                )
            }
        }
    }
}

fn pick(left: Arity, right: Arity) -> Arity {
    if left == Arity::Any { right } else { left }
}

/// `{<<x, y>> : x \in A, y \in B}`; rtlc has no `\X`.
/// One binder per set map: rtlc rejects a map with two binders whose
/// domain holds another comprehension ("Invalid safety variable read",
/// "Invalid bound variable slot", issue #112; plan 0103).
fn product(left: &str, right: &str, id: usize) -> String {
    pairwise(&format!("<<x{id}, y{id}>>"), left, right, id)
}

/// `{f(x, y) : x \in left, y \in right}` with one binder per map.
fn pairwise(element: &str, left: &str, right: &str, id: usize) -> String {
    format!("UNION {{{{{element} : y{id} \\in {right}}} : x{id} \\in {left}}}")
}
