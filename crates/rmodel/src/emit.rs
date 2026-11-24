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
    Check, CheckKind, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Quantifier, SetOp,
    Shape,
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
    for sig in &model.sigs {
        out.push_str(&format!("sig s_{sig} {{}}\n"));
    }
    for (name, members) in &model.enums {
        let members: Vec<String> = members.iter().map(|member| format!("e_{member}")).collect();
        out.push_str(&format!(
            "abstract sig s_{name} {{}}\none sig {} extends s_{name} {{}}\n",
            members.join(", ")
        ));
    }
    let fields: Vec<String> = model
        .relations
        .iter()
        .map(|relation| {
            let shape = match &relation.shape {
                Shape::Unary { sig, mult } => format!("{} s_{sig}", alloy_mult(*mult)),
                Shape::Binary { from, to, mult } => {
                    format!("s_{from} -> {} s_{to}", alloy_mult(*mult))
                }
            };
            let var = if relation.var { "var " } else { "" };
            format!("  {var}r_{}: {shape}", relation.name)
        })
        .collect();
    out.push_str(&format!("one sig World {{\n{}\n}}\n", fields.join(",\n")));
    let empty = Scope::new();
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
            body.push(format!(
                "(World.r_{var})' = {}",
                alloy_expression(model, value, &scope)
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
    out.push_str(&format!("fact {{ always {{ {} }} }}\n", steps.join(" or ")));
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
        .chain(["4 Int".to_owned(), format!("{} steps", model.scope.steps)])
        .collect();
    for check in &model.checks {
        let formula = alloy_formula(model, &check.formula, &empty);
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
    out
}

fn alloy_mult(mult: Mult) -> &'static str {
    match mult {
        Mult::Set => "set",
        Mult::Lone => "lone",
        Mult::One => "one",
        Mult::Some => "some",
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
            };
            format!(
                "({} {symbol} {})",
                alloy_expression(model, left, scope),
                alloy_expression(model, right, scope)
            )
        }
    }
}

fn alloy_formula(model: &Model, formula: &Formula, scope: &Scope) -> String {
    let expression = |expression: &Expr| alloy_expression(model, expression, scope);
    match formula {
        Formula::Compare(operator, left, right) => {
            let (left, right) = (expression(left), expression(right));
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
    let target = model
        .checks
        .iter()
        .find(|candidate| candidate.name == check)
        .ok_or_else(|| Error::kind(format!("no check {check}")))?;
    let mut emitter = Tla { model, fresh: 0 };
    Ok((
        emitter.module(target),
        format!("INIT Init\nNEXT Next\nINVARIANT {}\n", alloy_label(check)),
    ))
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
    fn module(&mut self, check: &Check) -> String {
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
            let atoms: Vec<String> = members
                .iter()
                .map(|member| format!("\"{member}\""))
                .collect();
            out.push_str(&format!("s_{name} == {{{}}}\n", atoms.join(", ")));
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
            out.push_str(&format!(
                "T_{}(V) == V \\subseteq D_{} /\\ {shape}\n",
                relation.name, relation.name
            ));
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
            for (var, _) in &action.assignments {
                body.push(format!("T_{var}(r_{var}')"));
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
        out.push_str(&format!("Next == {}\n", steps.join(" \\/ ")));
        let formula = self.formula(&check.formula, &empty);
        let body = match check.kind {
            CheckKind::Invariant => formula,
            CheckKind::Reachable => format!("~{formula}"),
        };
        out.push_str(&format!("{} == {body}\n====\n", alloy_label(&check.name)));
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
                } else if self.model.member(name).is_some() {
                    (format!("{{\"{name}\"}}"), Arity::Set)
                } else {
                    (format!("s_{name}"), Arity::Set)
                }
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
fn product(left: &str, right: &str, id: usize) -> String {
    format!("{{<<x{id}, y{id}>> : x{id} \\in {left}, y{id} \\in {right}}}")
}
