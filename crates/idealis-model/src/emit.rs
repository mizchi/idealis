//! The two lowerings (plan 0091). They share only the checked syntax tree,
//! never each other's output.
//!
//! Alloy: every relation is a field of `one sig World`; `init` and facts
//! hold in the first state; each action is a pred whose unassigned vars are
//! unchanged; every step is an action or a stutter. Invariants are
//! `check { always f }`, reachability is `run { eventually f }`.
//!
//! TLA+: atoms are strings, sets are sets and binary relations are sets of
//! pairs. Static relations are variables initialized from a fixed fact or
//! a subset of their domain and stay unchanged thereafter.
//! Every relation is typed and has its multiplicity in every state.
use crate::reference::{Data, Machine};
use idealis_semantics::fixed_initial_values;
use idealis_syntax::{
    Action, Arith, Check, CheckKind, Compare, Expr, FairnessKind, Formula, INT, Int, IntCompare,
    Model, Mult, NAT, Phase, Quantifier, SetOp, Shape, TemporalCheck, TransitionCheck,
};
use idealis_syntax::{DECLARATIONS, Error, SCOPE, atom_count, validate_integer_bounds};
use std::collections::BTreeMap;

/// The Alloy command label of a check.
pub fn alloy_label(check: &str) -> String {
    format!("c_{check}")
}

#[derive(Clone, Default)]
struct AlloyScope {
    names: BTreeMap<String, String>,
    after: bool,
}

const ALLOY_MAX_STEPS: usize = 32;
const ALLOY_DEFAULT_STEPS: usize = 4;

/// The trace bound used by the Alloy backend.
pub fn alloy_steps(model: &Model) -> usize {
    model.scope.steps.unwrap_or(ALLOY_DEFAULT_STEPS)
}

fn alloy_bitwidth(model: &Model, features: CollectionFeatures<'_>) -> usize {
    if alloy_count_bound(model, features.counts) > 7
        || features
            .literals
            .iter()
            .any(|&value| !(-8..=7).contains(&value))
    {
        5
    } else {
        4
    }
}

/// Render Alloy for a model accepted by `alloy_supports`.
pub fn to_alloy(model: &Model) -> String {
    let mut out = String::new();
    let emit_transitions = alloy_transition_supports(model).is_ok();
    let mut bitwidth = alloy_bitwidth(model, collection_features(model, false));
    if emit_transitions {
        bitwidth = bitwidth.max(alloy_bitwidth(model, collection_features(model, true)));
    }
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
    let empty = AlloyScope::default();
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
        let scope = alloy_action_scope(action);
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
            body.push(alloy_assignment(model, var, value, &scope, pair));
        }
        let assigned: Vec<&str> = action
            .assignments
            .iter()
            .map(|(var, _)| var.as_str())
            .collect();
        body.extend(frame(&assigned));
        let declarations = alloy_parameters(action);
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
            steps.push(format!(
                "(some {} | {})",
                declarations.join(", "),
                alloy_call(action),
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
            format!("{bitwidth} Int"),
            format!("{} steps", alloy_steps(model)),
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
    let action_steps = &steps[..model.actions.len()];
    for check in model.transitions.iter().filter(|_| emit_transitions) {
        let formula = alloy_transition(model, check, declared.as_deref(), action_steps);
        out.push_str(&format!(
            "{}: check {{ always {{ {formula} }} }} for {}\n",
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

fn alloy_action_scope(action: &Action) -> AlloyScope {
    AlloyScope {
        names: action
            .params
            .iter()
            .map(|(param, _)| (param.clone(), format!("p_{param}")))
            .collect(),
        after: false,
    }
}

fn alloy_parameters(action: &Action) -> Vec<String> {
    action
        .params
        .iter()
        .map(|(param, sig)| format!("p_{param}: s_{sig}"))
        .collect()
}

fn alloy_call(action: &Action) -> String {
    if action.params.is_empty() {
        format!("a_{}", action.name)
    } else {
        let arguments: Vec<String> = action
            .params
            .iter()
            .map(|(param, _)| format!("p_{param}"))
            .collect();
        format!("a_{}[{}]", action.name, arguments.join(", "))
    }
}

fn alloy_transition(
    model: &Model,
    check: &TransitionCheck,
    declared: Option<&str>,
    action_steps: &[String],
) -> String {
    let implication = |step: String, scope: &AlloyScope| {
        let valid_step = match declared {
            Some(valid) => format!("({valid} and (after {{ {valid} }}) and {step})"),
            None => step,
        };
        let property = alloy_formula(model, &check.formula, scope);
        format!("({valid_step} implies {property})")
    };
    if let Some(name) = &check.action {
        // A pair may be produced by several calls. Each matching call must satisfy the property.
        let action = model
            .actions
            .iter()
            .find(|action| &action.name == name)
            .expect("checked action");
        let body = implication(alloy_call(action), &alloy_action_scope(action));
        let parameters = alloy_parameters(action);
        if parameters.is_empty() {
            body
        } else {
            format!("(all {} | {body})", parameters.join(", "))
        }
    } else if action_steps.is_empty() {
        "(no none)".to_owned()
    } else {
        // Exclude synthetic waiting while keeping explicit no-op actions.
        implication(
            format!("({})", action_steps.join(" or ")),
            &AlloyScope::default(),
        )
    }
}

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

fn alloy_sort_size(model: &Model, name: &str) -> usize {
    if let Some(size) = model.scope.sizes.get(name) {
        return *size;
    }
    if let Some((_, members)) = model.enums.iter().find(|(sort, _)| sort == name) {
        return members.len();
    }
    if let Some((_, children)) = model.sums.iter().find(|(sort, _)| sort == name) {
        return children
            .iter()
            .map(|child| alloy_sort_size(model, child))
            .sum();
    }
    atom_count(model)
}

fn alloy_collection_bound(model: &Model, value: &Expr) -> usize {
    let atoms = atom_count(model);
    let bound = if is_pair(model, value, &AlloyScope::default()) {
        atoms.saturating_mul(atoms)
    } else {
        atoms
    };
    let size = |inner| alloy_collection_bound(model, inner);
    match value {
        Expr::None => 0,
        Expr::Name(name) => {
            if let Some(relation) = model.relation(name) {
                match &relation.shape {
                    Shape::Binary { from, to, .. } => {
                        alloy_sort_size(model, from).saturating_mul(alloy_sort_size(model, to))
                    }
                    Shape::Unary { sig, .. } => alloy_sort_size(model, sig),
                }
            } else if model.member(name).is_some() {
                1
            } else {
                alloy_sort_size(model, name)
            }
        }
        Expr::At(_, inner) => size(inner),
        Expr::If(_, yes, no) => size(yes).max(size(no)),
        Expr::Select(_, sort, _) => alloy_sort_size(model, sort),
        Expr::Binary(SetOp::Union | SetOp::SetUnion | SetOp::Override, left, right) => {
            size(left).saturating_add(size(right)).min(bound)
        }
        Expr::Binary(SetOp::Difference | SetOp::SetDifference | SetOp::WithoutKeys, left, _) => {
            size(left)
        }
        Expr::Binary(SetOp::Intersection, left, right) => size(left).min(size(right)),
        Expr::Binary(SetOp::Product | SetOp::Join, left, right) => {
            size(left).saturating_mul(size(right)).min(bound)
        }
        _ => bound,
    }
}

fn alloy_count_bound(model: &Model, counts: Vec<&Expr>) -> usize {
    let scope = AlloyScope::default();
    counts
        .into_iter()
        .map(|value| {
            if is_pair(model, value, &scope) {
                alloy_collection_bound(model, value)
            } else {
                atom_count(model)
            }
        })
        .max()
        .unwrap_or(0)
}

/// Whether the Alloy lowering can express the model: Alloy integers wrap
/// at their bit width, and the value semantics of records has no lowering
/// yet (plan 0103).
pub fn alloy_supports(model: &Model) -> Result<(), String> {
    if !(1..=ALLOY_MAX_STEPS).contains(&alloy_steps(model)) {
        return Err(format!("Alloy steps must be in 1..={ALLOY_MAX_STEPS}"));
    }
    if model.scope.ints.is_some() {
        return Err("integer values: Alloy integers wrap at their bit width".into());
    }
    if model.has_structures() {
        return Err("sequences and payload variants have no Alloy lowering".into());
    }
    if !model.records.is_empty() {
        return Err("records: no Alloy lowering of value semantics".into());
    }
    let features = collection_features(model, false);
    if features.selection {
        return Err("set selection requires Alloy comprehension, unsupported by ralloy".into());
    }
    if features
        .literals
        .iter()
        .any(|value| !(-16..=15).contains(value))
    {
        return Err("integer literals exceed Alloy's supported range -16..=15".into());
    }
    let count = alloy_count_bound(model, features.counts);
    if count > 15 {
        return Err(format!(
            "counts exceed Alloy's maximum of 15 atoms or tuples: capacity {count}"
        ));
    }
    Ok(())
}

/// Whether the Alloy lowering can also emit the model's transition checks.
pub fn alloy_transition_supports(model: &Model) -> Result<(), String> {
    alloy_supports(model)?;
    let features = collection_features(model, true);
    if features.selection {
        return Err(
            "transition selection requires Alloy comprehension, unsupported by ralloy".into(),
        );
    }
    if features
        .literals
        .iter()
        .any(|value| !(-16..=15).contains(value))
    {
        return Err("transition integer literals exceed Alloy's supported range -16..=15".into());
    }
    let count = alloy_count_bound(model, features.counts);
    if count > 15 {
        return Err(format!(
            "transition counts exceed Alloy's maximum of 15 atoms or tuples: capacity {count}"
        ));
    }
    Ok(())
}

fn alloy_assignment(
    model: &Model,
    var: &str,
    value: &Expr,
    scope: &AlloyScope,
    pair: bool,
) -> String {
    if let Expr::If(test, yes, no) = value {
        let test = alloy_formula(model, test, scope);
        let yes = alloy_assignment(model, var, yes, scope, pair);
        let no = alloy_assignment(model, var, no, scope, pair);
        format!("(({test} implies ({yes})) and ((not {test}) implies ({no})))")
    } else {
        format!("(World.r_{var})' = {}", alloy_as(model, value, scope, pair))
    }
}

const VALUES: &str = "alloy_supports rejects models with values";

/// Whether an expression denotes a binary relation.
fn is_pair(model: &Model, expression: &Expr, scope: &AlloyScope) -> bool {
    match expression {
        Expr::None => false,
        Expr::At(_, expr) => is_pair(model, expr, scope),
        Expr::If(_, yes, no) => is_pair(model, yes, scope) || is_pair(model, no, scope),
        Expr::Name(name) => {
            !scope.names.contains_key(name)
                && matches!(
                    model.relation(name).map(|relation| &relation.shape),
                    Some(Shape::Binary { .. })
                )
        }
        Expr::Binary(SetOp::Product | SetOp::WithoutKeys, _, _) | Expr::Closure(_) => true,
        Expr::Binary(SetOp::Join, left, right) => {
            is_pair(model, left, scope) && is_pair(model, right, scope)
        }
        Expr::Binary(_, left, right) => is_pair(model, left, scope) || is_pair(model, right, scope),
        Expr::Literal(_)
        | Expr::Count(_)
        | Expr::Arith(..)
        | Expr::Record(..)
        | Expr::Field(..)
        | Expr::Sequence(..)
        | Expr::Append(..)
        | Expr::Tail(_)
        | Expr::Take(..)
        | Expr::RemoveAt(..)
        | Expr::RemoveAll(..)
        | Expr::ReachableFrom(..)
        | Expr::Select(..)
        | Expr::Keys(..)
        | Expr::Length(_)
        | Expr::Elements(_)
        | Expr::Index(..)
        | Expr::Variant(..)
        | Expr::Payload(..) => false,
    }
}

/// `none` as the empty relation of the arity it stands for: Alloy's `none`
/// is unary (review of #102).
fn alloy_as(model: &Model, expression: &Expr, scope: &AlloyScope, pair: bool) -> String {
    match expression {
        Expr::At(phase, inner) => {
            let mut scope = scope.clone();
            scope.after = *phase == Phase::After;
            alloy_as(model, inner, &scope, pair)
        }
        Expr::If(test, yes, no) => {
            let pair = pair || is_pair(model, expression, scope);
            let test = alloy_formula(model, test, scope);
            let yes = alloy_as(model, yes, scope, pair);
            let no = alloy_as(model, no, scope, pair);
            format!("({test} implies {yes} else {no})")
        }
        Expr::None if pair => "(none -> none)".into(),
        Expr::Binary(SetOp::Override, left, right) => {
            let left = alloy_as(model, left, scope, true);
            let right = alloy_as(model, right, scope, true);
            format!("(({left} - (({right}.univ) -> univ)) + {right})")
        }
        Expr::Binary(SetOp::WithoutKeys, relation, keys) => {
            let relation = alloy_as(model, relation, scope, true);
            let keys = alloy_as(model, keys, scope, false);
            format!("({relation} - ({keys} -> univ))")
        }
        // The binary context reaches the operands of a set operation, so
        // `none + none` is binary too (issue #106).
        Expr::Binary(
            operator @ (SetOp::Union
            | SetOp::SetUnion
            | SetOp::Difference
            | SetOp::SetDifference
            | SetOp::Intersection),
            left,
            right,
        ) if pair => {
            let symbol = match operator {
                SetOp::Union | SetOp::SetUnion => "+",
                SetOp::Difference | SetOp::SetDifference => "-",
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

fn alloy_expression(model: &Model, expression: &Expr, scope: &AlloyScope) -> String {
    match expression {
        Expr::Select(variable, sort, predicate) => {
            let mut inner = scope.clone();
            inner
                .names
                .insert(variable.clone(), format!("v_{variable}"));
            format!(
                "{{v_{variable}: s_{sort} | {}}}",
                alloy_formula(model, predicate, &inner)
            )
        }
        Expr::Closure(edges) => format!("^({})", alloy_as(model, edges, scope, true)),
        Expr::Keys(relation) => format!("({}.univ)", alloy_as(model, relation, scope, true)),
        Expr::ReachableFrom(roots, edges) => {
            let roots = alloy_as(model, roots, scope, false);
            let edges = alloy_as(model, edges, scope, true);
            format!("({roots} + (({roots}).^({edges})))")
        }
        Expr::At(..) => alloy_as(model, expression, scope, is_pair(model, expression, scope)),
        Expr::If(..) => alloy_as(model, expression, scope, false),
        Expr::None => "none".into(),
        Expr::Name(name) => {
            if let Some(bound) = scope.names.get(name) {
                bound.clone()
            } else if let Some(relation) = model.relation(name) {
                let prime = if scope.after && relation.var { "'" } else { "" };
                format!("(World.r_{name}){prime}")
            } else if model.member(name).is_some() {
                format!("e_{name}")
            } else {
                format!("s_{name}")
            }
        }
        Expr::Binary(SetOp::Override | SetOp::WithoutKeys, ..) => {
            alloy_as(model, expression, scope, true)
        }
        Expr::Binary(operator, left, right) => {
            let symbol = match operator {
                SetOp::Union | SetOp::SetUnion => "+",
                SetOp::Difference | SetOp::SetDifference => "-",
                SetOp::Intersection => "&",
                SetOp::Product => "->",
                SetOp::Join => ".",
                SetOp::Override | SetOp::WithoutKeys | SetOp::Multiply => {
                    unreachable!("handled separately")
                }
            };
            let pair = matches!(
                operator,
                SetOp::Union
                    | SetOp::SetUnion
                    | SetOp::Difference
                    | SetOp::SetDifference
                    | SetOp::Intersection
            ) && (is_pair(model, left, scope) || is_pair(model, right, scope));
            format!(
                "({} {symbol} {})",
                alloy_as(model, left, scope, pair),
                alloy_as(model, right, scope, pair)
            )
        }
        Expr::Count(counted) => format!("(#{})", alloy_expression(model, counted, scope)),
        Expr::Literal(_)
        | Expr::Arith(..)
        | Expr::Record(..)
        | Expr::Field(..)
        | Expr::Sequence(..)
        | Expr::Append(..)
        | Expr::Tail(_)
        | Expr::Take(..)
        | Expr::RemoveAt(..)
        | Expr::RemoveAll(..)
        | Expr::Length(_)
        | Expr::Elements(_)
        | Expr::Index(..)
        | Expr::Variant(..)
        | Expr::Payload(..) => {
            unreachable!("{VALUES}")
        }
    }
}

fn alloy_formula(model: &Model, formula: &Formula, scope: &AlloyScope) -> String {
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
                Int::Value(_) | Int::LocatedValue(..) => unreachable!("{VALUES}"),
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
        Formula::HasTag(..) => unreachable!("{VALUES}"),
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
            inner
                .names
                .insert(variable.clone(), format!("v_{variable}"));
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

/// TLC needs portable integers and materializable structured domains.
fn tla_refinement(clause: &crate::Refinement) -> String {
    let operand = |value: &crate::RefinementOperand| match value {
        crate::RefinementOperand::Field(path) => path
            .iter()
            .fold("v".to_owned(), |value, field| format!("{value}.f_{field}")),
        crate::RefinementOperand::Literal(n) => n.to_string(),
    };
    let op = match clause.symbol() {
        "<=" => "=<",
        "!=" => "#",
        op => op,
    };
    format!("{} {op} {}", operand(&clause.left), operand(&clause.right))
}

pub fn tla_supports(model: &Model) -> Result<(), String> {
    let limit = i64::from(i32::MAX);
    validate_integer_bounds(model, -limit..=limit).map_err(|error| error.message)?;
    for record in &model.records {
        for clause in &record.refinements {
            for operand in [&clause.left, &clause.right] {
                if let crate::RefinementOperand::Literal(n) = operand
                    && !(-limit..=limit).contains(n)
                {
                    return Err(format!(
                        "{}: where literal {n} exceeds the TLA+ integer range",
                        record.name
                    ));
                }
            }
        }
    }
    if (model.has_structures() || model.has_refinements())
        && let Some(domain) = model
            .value_domains()
            .map_err(|error| error.message)?
            .into_iter()
            .find(|domain| !domain.enumerable())
    {
        return Err(format!(
            "{} exceeds the TLA+ value materialization limit",
            domain.kind.name()
        ));
    }
    Ok(())
}

/// A TLA+ module and TLC configuration checking one check of the model.
pub fn to_tla(model: &Model, check: &str) -> Result<(String, String), Error> {
    lower_tla(model, check, None)
}

/// One transition label and the exact source command it represents.
pub struct GroundAction {
    pub label: String,
    pub action: String,
    pub args: Vec<(String, Data)>,
}

/// A temporal lowering that retains the identities of concrete action instances.
pub struct TemporalWitnessTla {
    pub source: String,
    pub config: String,
    pub actions: Vec<GroundAction>,
}

pub fn to_tla_temporal_witness(model: &Model, check: &str) -> Result<TemporalWitnessTla, Error> {
    tla_supports(model).map_err(Error::kind)?;
    if !model.temporal.iter().any(|property| property.name == check) {
        return Err(Error::kind(format!("no temporal property {check}")));
    }
    let machine = Machine::new(model).map_err(Error::kind)?;
    let mut actions = Vec::new();
    // The explicit compiler accepts 4096 branches. The generated Next has
    // three additional branches for unchanged and invalid states.
    const MAX_ACTIONS: usize = 4093;
    for (index, action) in model.actions.iter().enumerate() {
        let arguments = machine
            .action_arguments(action, MAX_ACTIONS - actions.len())
            .ok_or_else(|| {
                Error::kind("temporal witness exceeds the explicit action branch limit")
            })?;
        for (instance, args) in arguments.into_iter().enumerate() {
            let label = if args.is_empty() {
                format!("a_{}", action.name)
            } else {
                format!("Witness_{index}_{instance}")
            };
            actions.push(GroundAction {
                label,
                action: action.name.clone(),
                args,
            });
        }
    }
    let (source, config) = lower_tla(model, check, Some(&actions))?;
    Ok(TemporalWitnessTla {
        source,
        config,
        actions,
    })
}

fn lower_tla(
    model: &Model,
    check: &str,
    grounded: Option<&[GroundAction]>,
) -> Result<(String, String), Error> {
    tla_supports(model).map_err(Error::kind)?;
    let target = if check == DECLARATIONS {
        Target::Declarations
    } else if check == SCOPE {
        Target::Scope
    } else if let Some(property) = model.transitions.iter().find(|p| p.name == check) {
        Target::Transition(property)
    } else if let Some(obligation) = model.temporal.iter().find(|p| p.name == check) {
        Target::Temporal(obligation)
    } else {
        Target::Check(
            model
                .checks
                .iter()
                .find(|candidate| candidate.name == check)
                .ok_or_else(|| Error::kind(format!("no check {check}")))?,
        )
    };
    let config = if matches!(target, Target::Temporal(_) | Target::Transition(_)) {
        format!("SPECIFICATION Spec\nPROPERTY {}\n", alloy_label(check))
    } else {
        format!("INIT Init\nNEXT Next\nINVARIANT {}\n", alloy_label(check))
    };
    let mut emitter = Tla {
        model,
        fresh: 0,
        after: false,
    };
    Ok((emitter.module(target, grounded), config))
}

enum Target<'a> {
    Declarations,
    Scope,
    Check(&'a Check),
    Temporal(&'a TemporalCheck),
    Transition(&'a TransitionCheck),
}

struct Tla<'a> {
    model: &'a Model,
    fresh: usize,
    after: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arity {
    Any,
    Set,
    Pair,
}

impl Tla<'_> {
    fn module(&mut self, check: Target<'_>, grounded: Option<&[GroundAction]>) -> String {
        let model = self.model;
        let mut out = format!(
            "---- MODULE {} ----\nEXTENDS Integers, FiniteSets{}\n",
            model.name,
            if model.sequences.is_empty() {
                ""
            } else {
                ", Sequences"
            }
        );
        let mut variables: Vec<String> = model
            .relations
            .iter()
            .map(|relation| format!("r_{}", relation.name))
            .collect();
        let observes_step = matches!(check, Target::Transition(_));
        if observes_step {
            variables.push("idealis_tick".into());
        }
        // Represent the sole state of a relation-free model in TLA+.
        let unit_state = variables.is_empty();
        if unit_state {
            variables.push("idealis_unit".into());
        }
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
            if model.uses_nat() {
                out.push_str(&format!("s_{NAT} == {}..{high}\n", low.max(0)));
            }
        }
        use idealis_syntax::ValueType;
        // s_ sorts constrain enumeration and declarations; u_ sorts retain
        // just the integer scope for diagnostics on the attempted successor.
        let mut refined_sorts = std::collections::BTreeSet::new();
        for value in model.value_domains().expect("checked value types") {
            let ty = value.kind;
            let children = ty.children();
            let own = match ty {
                ValueType::Record(record) => record.refinements.as_slice(),
                _ => &[],
            };
            let mut domain = |raw| {
                let sort_name = |sort| {
                    let prefix = if raw && refined_sorts.contains(sort) {
                        "u"
                    } else {
                        "s"
                    };
                    format!("{prefix}_{sort}")
                };
                match ty {
                    ValueType::Record(record) => {
                        let fields = record
                            .fields
                            .iter()
                            .map(|(field, sort)| {
                                format!("f_{field} : {}", sort_name(sort.as_str()))
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("[{fields}]")
                    }
                    ValueType::Sequence(seq) => (0..=seq.max)
                        .map(|len| self.tuples(&vec![sort_name(seq.element.as_str()); len]))
                        .collect::<Vec<_>>()
                        .join(" \\cup "),
                    ValueType::Variant(variant) => variant
                        .cases
                        .iter()
                        .map(|case| self.tagged(&case.tag, case.payload.as_deref().map(sort_name)))
                        .collect::<Vec<_>>()
                        .join(" \\cup "),
                }
            };
            let filtered = domain(false);
            let filtered = if own.is_empty() {
                filtered
            } else {
                let predicate = own
                    .iter()
                    .map(tla_refinement)
                    .collect::<Vec<_>>()
                    .join(" /\\ ");
                format!("{{v \\in {filtered} : {predicate}}}")
            };
            let name = ty.name();
            out.push_str(&format!("s_{name} == {filtered}\n"));
            if !own.is_empty() || children.iter().any(|sort| refined_sorts.contains(sort)) {
                out.push_str(&format!("u_{name} == {}\n", domain(true)));
                refined_sorts.insert(name);
            }
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
                let name = &relation.name;
                if refined_sorts.contains(target.as_str()) {
                    let domain = match &relation.shape {
                        Shape::Unary { .. } => format!("u_{target}"),
                        Shape::Binary { from, .. } => {
                            product(&format!("s_{from}"), &format!("u_{target}"), 0)
                        }
                    };
                    out.push_str(&format!("U_{name} == {domain}\n"));
                    out.push_str(&format!(
                        "T_{name}(V) == V \\subseteq D_{name} /\\ {shape}\n"
                    ));
                } else {
                    out.push_str(&format!("T_{name}(V) == {shape}\n"));
                }
            } else {
                out.push_str(&format!(
                    "T_{}(V) == V \\subseteq D_{} /\\ {shape}\n",
                    relation.name, relation.name
                ));
            }
        }
        let empty = BTreeMap::new();
        let fixed = fixed_initial_values(model);
        let mut init: Vec<String> = model
            .relations
            .iter()
            .map(|relation| {
                if let Some(value) = fixed.get(relation.name.as_str()) {
                    let (value, _) = self.expression(value, &empty);
                    // Keep the domain restriction even when T_r only checks multiplicity.
                    let name = &relation.name;
                    format!("r_{name} = {value} /\\ r_{name} \\subseteq D_{name}")
                } else if let Shape::Unary {
                    sig,
                    mult: Mult::One | Mult::Lone,
                } = &relation.shape
                    && model.has_structures()
                    && model.is_value(sig)
                {
                    let empty = if matches!(
                        relation.shape,
                        Shape::Unary {
                            mult: Mult::Lone,
                            ..
                        }
                    ) {
                        " \\cup {{}}"
                    } else {
                        ""
                    };
                    format!(
                        "r_{0} \\in ({{{{v}} : v \\in D_{0}}}{empty})",
                        relation.name
                    )
                } else {
                    format!("r_{0} \\in SUBSET D_{0}", relation.name)
                }
            })
            .collect();
        if observes_step {
            init.push("idealis_tick = FALSE".into());
        }
        if unit_state {
            init.push("idealis_unit = FALSE".into());
        }
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
            let body = self.action_body(action, &scope, unit_state, observes_step);
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
            if let Some(choices) = grounded
                && !params.is_empty()
            {
                for choice in choices.iter().filter(|choice| choice.action == action.name) {
                    let scope = choice
                        .args
                        .iter()
                        .map(|(name, value)| (name.clone(), format!("{{{}}}", self.literal(value))))
                        .collect();
                    let body = self.action_body(action, &scope, unit_state, observes_step);
                    out.push_str(&format!("{} == {}\n", choice.label, body.join(" /\\ ")));
                    steps.push(choice.label.clone());
                }
                continue;
            }
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
        let declared = if unit_state {
            "idealis_unit = FALSE".to_owned()
        } else if declared.is_empty() {
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
                .map(|relation| {
                    let sort = match &relation.shape {
                        Shape::Unary { sig, .. } => sig,
                        Shape::Binary { to, .. } => to,
                    };
                    let prefix = if refined_sorts.contains(sort.as_str()) {
                        "U"
                    } else {
                        "D"
                    };
                    format!("r_{0} \\subseteq {prefix}_{0}", relation.name)
                })
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
        if let Target::Temporal(obligation) = &check {
            let state = format!("<<{}>>", variables.join(", "));
            // Keep Init opaque to the compiler's SPECIFICATION splitter.
            let mut spec = format!("(Init \\/ FALSE) /\\ [][Next]_{state}");
            for fairness in obligation.fairness.as_ref().unwrap_or(&model.fairness) {
                let action = model
                    .actions
                    .iter()
                    .find(|a| a.name == fairness.action)
                    .expect("checked action");
                let args = action
                    .params
                    .iter()
                    .map(|(param, _)| format!("p_{param}"))
                    .collect::<Vec<_>>();
                let mut choice = if args.is_empty() {
                    format!("a_{}", action.name)
                } else {
                    format!("a_{}({})", action.name, args.join(", "))
                };
                let fair = match fairness.kind {
                    FairnessKind::Weak => "WF",
                    FairnessKind::Strong => "SF",
                };
                if fairness.each && !args.is_empty() {
                    let name = format!("Fair_{}({})", action.name, args.join(", "));
                    out.push_str(&format!("{name} == {valid} /\\ {choice}\n"));
                    let mut rule = format!("{fair}_{state}({name})");
                    for (param, sort) in action.params.iter().rev() {
                        rule = format!("(\\A p_{param} \\in s_{sort} : {rule})");
                    }
                    spec.push_str(&format!(" /\\ {rule}"));
                } else {
                    for (param, sort) in action.params.iter().rev() {
                        choice = format!("(\\E p_{param} \\in s_{sort} : {choice})");
                    }
                    out.push_str(&format!("Fair_{} == {valid} /\\ {choice}\n", action.name));
                    spec.push_str(&format!(" /\\ {fair}_{state}(Fair_{})", action.name));
                }
            }
            out.push_str(&format!("Spec == {spec}\n"));
        }
        if observes_step {
            out.push_str(&format!(
                "Spec == (Init \\/ FALSE) /\\ [][Next]_<<{}>>\n",
                variables.join(", ")
            ));
        }
        let (name, body) = match check {
            Target::Declarations if model.has_values() => {
                (DECLARATIONS, "(InScope => TypeOK)".to_owned())
            }
            Target::Declarations => (DECLARATIONS, "TypeOK".to_owned()),
            Target::Scope if model.has_values() => (SCOPE, "InScope".to_owned()),
            Target::Scope => (SCOPE, "TRUE".to_owned()),
            Target::Transition(check) => {
                let state = variables.join(", ");
                let mut condition = format!("({valid}) /\\ ({valid})'");
                let action = check.action.as_ref().map(|name| {
                    model
                        .actions
                        .iter()
                        .find(|action| &action.name == name)
                        .expect("checked action")
                });
                let scope = if let Some(action) = action {
                    let params: Vec<_> = action
                        .params
                        .iter()
                        .map(|(param, _)| format!("p_{param}"))
                        .collect();
                    if params.is_empty() {
                        condition.push_str(&format!(" /\\ a_{}", action.name));
                    } else {
                        condition.push_str(&format!(
                            " /\\ a_{}({})",
                            action.name,
                            params.join(", ")
                        ));
                    }
                    action
                        .params
                        .iter()
                        .zip(params)
                        .map(|((param, _), tla)| (param.clone(), format!("{{{tla}}}")))
                        .collect()
                } else {
                    BTreeMap::new()
                };
                let formula = self.formula(&check.formula, &scope);
                let mut assertion = format!("({condition} => {formula})");
                if let Some(action) = action {
                    for (param, sort) in action.params.iter().rev() {
                        assertion = format!("(\\A p_{param} \\in s_{sort} : {assertion})");
                    }
                }
                (check.name.as_str(), format!("[] [{assertion}]_<<{state}>>"))
            }
            Target::Temporal(check) => {
                let scope = check
                    .params
                    .iter()
                    .map(|(param, _)| (param.clone(), format!("{{p_{param}}}")))
                    .collect();
                let mut body = format!("<>({})", self.formula(&check.formula, &scope));
                if let Some(trigger) = &check.trigger {
                    let condition = self.formula(trigger, &scope);
                    body = format!("[](({condition}) => {body})");
                }
                for (param, sort) in check.params.iter().rev() {
                    body = format!("(\\A p_{param} \\in s_{sort} : {body})");
                }
                (check.name.as_str(), body)
            }
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
    fn tuples(&mut self, domains: &[String]) -> String {
        if domains.is_empty() {
            return "{<<>>}".into();
        }
        let id = self.next();
        let items: Vec<_> = (0..domains.len()).map(|i| format!("q{id}_{i}")).collect();
        let binds = items
            .iter()
            .zip(domains)
            .map(|(name, domain)| format!("{name} \\in {domain}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{{<<{}>> : {binds}}}", items.join(", "))
    }

    fn action_body(
        &mut self,
        action: &Action,
        scope: &BTreeMap<String, String>,
        unit_state: bool,
        observes_step: bool,
    ) -> Vec<String> {
        let mut body = Vec::new();
        // A guard may establish that a value read by a later expression exists.
        let guards_first = action.guards.iter().any(tests_presence);
        if guards_first {
            body.extend(action.guards.iter().map(|guard| self.formula(guard, scope)));
        }
        // Make an explicitly enabled no-op distinguishable from an implicit wait.
        if observes_step {
            body.push("idealis_tick' = ~idealis_tick".into());
        }
        if unit_state {
            body.push("UNCHANGED idealis_unit".into());
        }
        for (var, value) in &action.assignments {
            body.push(format!("r_{var}' = {}", self.expression(value, scope).0));
        }
        let unchanged: Vec<String> = self
            .model
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
        if !guards_first {
            body.extend(action.guards.iter().map(|guard| self.formula(guard, scope)));
        }
        body
    }

    fn literal(&self, value: &Data) -> String {
        match value {
            Data::Atom(name) => {
                let atom = self
                    .model
                    .enums
                    .iter()
                    .find(|(_, members)| members.contains(name))
                    .map_or_else(|| name.clone(), |(sort, _)| format!("{sort}.{name}"));
                format!("\"{atom}\"")
            }
            Data::Int(value) => value.to_string(),
            Data::Sequence(items) => format!(
                "<<{}>>",
                items
                    .iter()
                    .map(|item| self.literal(item))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Data::Record(_, fields) => format!(
                "[{}]",
                fields
                    .iter()
                    .map(|(name, value)| format!("f_{name} |-> {}", self.literal(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Data::Variant(tag, payload) => {
                let value = payload
                    .as_ref()
                    .map_or_else(|| "\"\"".into(), |value| self.literal(value));
                format!("[tag |-> \"{tag}\", value |-> {value}]")
            }
        }
    }

    fn tagged(&mut self, tag: &str, payload: Option<String>) -> String {
        match payload {
            Some(set) => {
                let id = self.next();
                format!("{{[tag |-> \"{tag}\", value |-> v{id}] : v{id} \\in {set}}}")
            }
            None => format!("{{[tag |-> \"{tag}\", value |-> \"\"]}}"),
        }
    }

    fn expression(
        &mut self,
        expression: &Expr,
        scope: &BTreeMap<String, String>,
    ) -> (String, Arity) {
        match expression {
            Expr::Closure(edges) => {
                let (edges, _) = self.expression(edges, scope);
                let id = self.next();
                let mut definitions = format!("(LET rows{id}_0 == {edges}");
                // A shortest positive path uses at most one edge per atom,
                // including paths from an atom back to itself along a cycle.
                let doublings = atom_count(self.model)
                    .max(1)
                    .next_power_of_two()
                    .trailing_zeros();
                for step in 1..=doublings {
                    let previous = format!("rows{id}_{}", step - 1);
                    let joined = compose(&previous, &previous, self.next());
                    definitions.push_str(&format!(
                        "\n  rows{id}_{step} == {previous} \\cup ({joined})"
                    ));
                }
                (
                    format!("{definitions}\n  IN rows{id}_{doublings})"),
                    Arity::Pair,
                )
            }
            Expr::Keys(relation) => {
                let (relation, _) = self.expression(relation, scope);
                let id = self.next();
                (
                    format!("{{row{id}[1] : row{id} \\in {relation}}}"),
                    Arity::Set,
                )
            }
            Expr::ReachableFrom(roots, edges) => {
                let (roots, _) = self.expression(roots, scope);
                let (edges, _) = self.expression(edges, scope);
                let id = self.next();
                let mut definitions =
                    format!("(LET roots{id}_0 == {roots}\n  edges{id} == {edges}");
                let bound = atom_count(self.model);
                for step in 1..=bound {
                    let prev = step - 1;
                    definitions.push_str(&format!(
                        "\n  roots{id}_{step} == roots{id}_{prev} \\cup {{edge{id}[2] : edge{id} \\in {{pair{id} \\in edges{id} : pair{id}[1] \\in roots{id}_{prev}}}}}"
                    ));
                }
                (
                    format!("{definitions}\n  IN roots{id}_{bound})"),
                    Arity::Set,
                )
            }
            Expr::If(condition, yes, no) => {
                let test = self.formula(condition, scope);
                let (yes, left) = self.expression(yes, scope);
                let (no, right) = self.expression(no, scope);
                (
                    format!("(IF {test} THEN {yes} ELSE {no})"),
                    pick(left, right),
                )
            }
            Expr::Select(variable, sort, predicate) => {
                let mut inner = scope.clone();
                inner.insert(variable.clone(), format!("{{v_{variable}}}"));
                (
                    format!(
                        "{{v_{variable} \\in s_{sort} : {}}}",
                        self.formula(predicate, &inner)
                    ),
                    Arity::Set,
                )
            }
            Expr::At(phase, expr) => {
                let after = std::mem::replace(&mut self.after, *phase == crate::Phase::After);
                let lowered = self.expression(expr, scope);
                self.after = after;
                lowered
            }
            Expr::None => ("{}".into(), Arity::Any),
            Expr::Name(name) => {
                if let Some(bound) = scope.get(name) {
                    (bound.clone(), Arity::Set)
                } else if let Some(relation) = self.model.relation(name) {
                    let arity = match relation.shape {
                        Shape::Unary { .. } => Arity::Set,
                        Shape::Binary { .. } => Arity::Pair,
                    };
                    let phase = if self.after && relation.var { "'" } else { "" };
                    (format!("r_{name}{phase}"), arity)
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
            Expr::Sequence(_, items) => {
                let domains = items
                    .iter()
                    .map(|v| self.expression(v, scope).0)
                    .collect::<Vec<_>>();
                (self.tuples(&domains), Arity::Set)
            }
            Expr::Append(seq, item) => {
                let id = self.next();
                let seq = self.expression(seq, scope).0;
                let item = self.expression(item, scope).0;
                (
                    pairwise(&format!("Append(x{id}, y{id})"), &seq, &item, id),
                    Arity::Set,
                )
            }
            Expr::Tail(seq) => {
                let id = self.next();
                let set = self.expression(seq, scope).0;
                (
                    format!("{{IF Len(v{id}) = 0 THEN <<>> ELSE Tail(v{id}) : v{id} \\in {set}}}"),
                    Arity::Set,
                )
            }
            Expr::Take(seq, count) => {
                let id = self.next();
                let seq = self.expression(seq, scope).0;
                let count = self.expression(count, scope).0;
                (
                    pairwise(
                        &format!(
                            "IF y{id} <= 0 THEN <<>> ELSE IF y{id} >= Len(x{id}) THEN x{id} ELSE SubSeq(x{id}, 1, y{id})"
                        ),
                        &seq,
                        &count,
                        id,
                    ),
                    Arity::Set,
                )
            }
            Expr::RemoveAt(seq, index) => {
                let id = self.next();
                let seq = self.expression(seq, scope).0;
                let index = self.expression(index, scope).0;
                (
                    pairwise(
                        &format!(
                            "IF y{id} < 0 \\/ Len(x{id}) <= y{id} THEN x{id} ELSE (SubSeq(x{id}, 1, y{id}) \\o SubSeq(x{id}, y{id} + 2, Len(x{id})))"
                        ),
                        &seq,
                        &index,
                        id,
                    ),
                    Arity::Set,
                )
            }
            Expr::RemoveAll(seq, excluded) => {
                let id = self.next();
                let seq = self.expression(seq, scope).0;
                let excluded = self.expression(excluded, scope).0;
                (
                    format!(
                        "(LET Keep{id}(v{id}) == v{id} \\notin {excluded}
                          IN {{SelectSeq(s{id}, Keep{id}) : s{id} \\in {seq}}})"
                    ),
                    Arity::Set,
                )
            }
            Expr::Elements(seq) => {
                let id = self.next();
                let set = self.expression(seq, scope).0;
                (
                    format!(
                        "UNION {{{{v{id}[i{id}] : i{id} \\in 1..Len(v{id})}} : v{id} \\in {set}}}"
                    ),
                    Arity::Set,
                )
            }
            Expr::Length(seq) => {
                let id = self.next();
                let set = self.expression(seq, scope).0;
                (format!("{{Len(v{id}) : v{id} \\in {set}}}"), Arity::Set)
            }
            Expr::Index(seq, index) => {
                let id = self.next();
                let seq = self.expression(seq, scope).0;
                let index = self.expression(index, scope).0;
                (
                    format!(
                        "UNION {{UNION {{IF 0 <= i{id} /\\ i{id} < Len(v{id}) THEN {{v{id}[i{id} + 1]}} ELSE {{}} : i{id} \\in {index}}} : v{id} \\in {seq}}}"
                    ),
                    Arity::Set,
                )
            }
            Expr::Variant(tag, payload) => {
                let set = payload.as_ref().map(|expr| self.expression(expr, scope).0);
                (self.tagged(tag, set), Arity::Set)
            }
            Expr::Payload(expr, tag) => {
                let id = self.next();
                let set = self.expression(expr, scope).0;
                (
                    format!(
                        "{{v{id}.value : v{id} \\in {{w{id} \\in {set} : w{id}.tag = \"{tag}\"}}}}"
                    ),
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
            Expr::Field(receiver, field) => {
                let id = self.next();
                let (receiver, arity) = self.expression(receiver, scope);
                let value = match arity {
                    Arity::Pair => format!("<<t{id}[1], t{id}[2].f_{field}>>"),
                    _ => format!("t{id}.f_{field}"),
                };
                (format!("{{{value} : t{id} \\in {receiver}}}"), arity)
            }
            Expr::Binary(operator, left, right) => {
                let (left, left_arity) = self.expression(left, scope);
                let (right, right_arity) = self.expression(right, scope);
                match operator {
                    SetOp::WithoutKeys => {
                        let id = self.next();
                        (
                            format!("{{row{id} \\in {left} : row{id}[1] \\notin ({right})}}"),
                            Arity::Pair,
                        )
                    }
                    SetOp::Override => {
                        let id = self.next();
                        (
                            format!(
                                "({{t{id} \\in {left} : \\A p{id} \\in {right} : p{id}[1] # t{id}[1]}} \\cup ({right}))"
                            ),
                            Arity::Pair,
                        )
                    }
                    SetOp::Union | SetOp::SetUnion => {
                        // SANY rejects an unparenthesized prefix UNION before \\cup.
                        let left = if left.starts_with("UNION ") {
                            format!("({left})")
                        } else {
                            left
                        };
                        (
                            format!("({left} \\cup {right})"),
                            pick(left_arity, right_arity),
                        )
                    }
                    SetOp::Difference | SetOp::SetDifference => (
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
                            (Arity::Pair, Arity::Pair) => (compose(&left, &right, id), Arity::Pair),
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
                    Int::Value(value) | Int::LocatedValue(value, _) => {
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
            Formula::HasTag(expr, tag) => {
                let id = self.next();
                let set = self.expression(expr, scope).0;
                format!("(\\E v{id} \\in {set} : v{id}.tag = \"{tag}\")")
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
                let domain = if sig == NAT {
                    let (low, high) = self.model.scope.ints.expect("validated Nat quantifier");
                    format!("({}..{high})", low.max(0))
                } else {
                    format!("s_{sig}")
                };
                format!(
                    "({symbol} v_{variable} \\in {domain} : {})",
                    self.formula(body, &inner)
                )
            }
        }
    }
}

fn pick(left: Arity, right: Arity) -> Arity {
    if left == Arity::Any { right } else { left }
}

fn compose(left: &str, right: &str, id: usize) -> String {
    format!(
        "UNION {{{{<<t{id}[1], u{id}[2]>> : u{id} \\in {{w{id} \\in {right} : w{id}[1] = t{id}[2]}}}} : t{id} \\in {left}}}"
    )
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

fn tests_presence(formula: &Formula) -> bool {
    match formula {
        Formula::Mult(Mult::Some, _) => true,
        Formula::And(parts) => parts.iter().any(tests_presence),
        _ => false,
    }
}

/// Operands of counts in the finite atom fragment used by the Alloy lowering.
#[derive(Default)]
struct CollectionFeatures<'a> {
    pub counts: Vec<&'a Expr>,
    pub literals: Vec<i64>,
    pub selection: bool,
}

fn collection_features(model: &Model, transitions_only: bool) -> CollectionFeatures<'_> {
    fn formula<'a>(value: &'a Formula, features: &mut CollectionFeatures<'a>) {
        match value {
            Formula::Ints(_, left, right) => {
                for side in [left, right] {
                    match side {
                        Int::Count(counted) => {
                            features.counts.push(counted);
                            expression(counted, features);
                        }
                        Int::Value(value) | Int::LocatedValue(value, _) => {
                            expression(value, features)
                        }
                        Int::Literal(value) => features.literals.push(*value),
                    }
                }
            }
            Formula::Compare(_, left, right) => {
                expression(left, features);
                expression(right, features);
            }
            Formula::Or(left, right) | Formula::Implies(left, right) => {
                formula(left, features);
                formula(right, features);
            }
            Formula::Mult(_, value) | Formula::Empty(value) => expression(value, features),
            Formula::Not(inner) | Formula::Quantified(_, _, _, inner) => formula(inner, features),
            Formula::And(parts) => parts.iter().for_each(|part| formula(part, features)),
            Formula::HasTag(..) => {}
        }
    }
    fn expression<'a>(value: &'a Expr, features: &mut CollectionFeatures<'a>) {
        match value {
            Expr::Count(inner) => {
                features.counts.push(inner);
                expression(inner, features);
            }
            Expr::If(condition, yes, no) => {
                formula(condition, features);
                expression(yes, features);
                expression(no, features);
            }
            Expr::Select(_, _, predicate) => {
                features.selection = true;
                formula(predicate, features);
            }
            Expr::At(_, inner) | Expr::Keys(inner) | Expr::Closure(inner) => {
                expression(inner, features)
            }
            Expr::Binary(_, left, right) | Expr::ReachableFrom(left, right) => {
                expression(left, features);
                expression(right, features);
            }
            _ => {}
        }
    }
    let mut features = CollectionFeatures::default();
    if transitions_only {
        for check in &model.transitions {
            formula(&check.formula, &mut features);
        }
    } else {
        for condition in model
            .facts
            .iter()
            .chain(&model.init)
            .chain(model.checks.iter().map(|check| &check.formula))
            .chain(model.actions.iter().flat_map(|action| &action.guards))
        {
            formula(condition, &mut features);
        }
        for (_, value) in model.actions.iter().flat_map(|action| &action.assignments) {
            expression(value, &mut features);
        }
    }
    features
}
