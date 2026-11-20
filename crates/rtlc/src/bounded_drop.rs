use crate::Value;
use crate::ir::{Binding, Branch, Domain, Expression, Step, TemporalFormula, Update};
use std::mem;

const DEPTH_BUDGET: usize = 32;

enum Detached {
    Expression(Expression),
    Branch(Branch),
    Formula(TemporalFormula),
    Value(Value),
}

type Worklist = Vec<Detached>;

#[inline(always)]
fn is_leaf_value(value: &Value) -> bool {
    match value {
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::String(_)
        | Value::Bytes(_)
        | Value::ModelValue(_) => true,
        Value::List(items) | Value::Set(items) => items.is_empty(),
        Value::Record(fields) => fields.is_empty(),
        Value::Map(entries) => entries.is_empty(),
    }
}

#[inline(always)]
fn is_flat_value(value: &Value) -> bool {
    match value {
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::String(_)
        | Value::Bytes(_)
        | Value::ModelValue(_) => true,
        Value::List(items) | Value::Set(items) => items.iter().all(is_leaf_value),
        Value::Record(fields) => fields.iter().all(|(_, item)| is_leaf_value(item)),
        Value::Map(entries) => entries
            .iter()
            .all(|(key, item)| is_leaf_value(key) && is_leaf_value(item)),
    }
}

#[inline(always)]
fn is_shallow_value(value: &Value) -> bool {
    match value {
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::String(_)
        | Value::Bytes(_)
        | Value::ModelValue(_) => true,
        Value::List(items) | Value::Set(items) => items.iter().all(is_flat_value),
        Value::Record(fields) => fields.iter().all(|(_, item)| is_flat_value(item)),
        Value::Map(entries) => entries
            .iter()
            .all(|(key, item)| is_flat_value(key) && is_flat_value(item)),
    }
}

fn is_leaf_expression(expression: &Expression) -> bool {
    match expression {
        Expression::Parameter { .. }
        | Expression::Capture { .. }
        | Expression::Bound { .. }
        | Expression::Old
        | Expression::UnmatchedCase { .. }
        | Expression::Literal { .. }
        | Expression::Read { .. } => true,
        Expression::Data { value } => is_leaf_value(value),
        _ => false,
    }
}

fn is_leaf_branch(branch: &Branch) -> bool {
    branch.steps.is_empty()
}

fn is_leaf_formula(formula: &TemporalFormula) -> bool {
    matches!(
        formula,
        TemporalFormula::Atom { expression } if is_leaf_expression(expression)
    )
}

#[inline(always)]
fn dispose_value(mut value: Value, budget: usize, worklist: &mut Worklist) {
    if is_flat_value(&value) {
        return;
    }
    if budget == 0 {
        worklist.push(Detached::Value(value));
        return;
    }
    descend_value(&mut value, budget - 1, worklist);
}

#[inline(never)]
fn descend_value(value: &mut Value, budget: usize, worklist: &mut Worklist) {
    detach_value(value, budget, worklist);
}

fn dispose_expression(mut expression: Expression, budget: usize, worklist: &mut Worklist) {
    if is_leaf_expression(&expression) {
        return;
    }
    if budget == 0 {
        worklist.push(Detached::Expression(expression));
        return;
    }
    detach_expression(&mut expression, budget - 1, worklist);
}

fn dispose_branch(mut branch: Branch, budget: usize, worklist: &mut Worklist) {
    if is_leaf_branch(&branch) {
        return;
    }
    if budget == 0 {
        worklist.push(Detached::Branch(branch));
        return;
    }
    detach_branch(&mut branch, budget - 1, worklist);
}

fn dispose_formula(mut formula: TemporalFormula, budget: usize, worklist: &mut Worklist) {
    if is_leaf_formula(&formula) {
        return;
    }
    if budget == 0 {
        worklist.push(Detached::Formula(formula));
        return;
    }
    detach_formula(&mut formula, budget - 1, worklist);
}

fn dispose_expressions(expressions: Vec<Expression>, budget: usize, worklist: &mut Worklist) {
    for expression in expressions {
        dispose_expression(expression, budget, worklist);
    }
}

fn dispose_fields(fields: Vec<(String, Expression)>, budget: usize, worklist: &mut Worklist) {
    for (_, expression) in fields {
        dispose_expression(expression, budget, worklist);
    }
}

fn dispose_branches(branches: Vec<Branch>, budget: usize, worklist: &mut Worklist) {
    for branch in branches {
        dispose_branch(branch, budget, worklist);
    }
}

fn dispose_update(update: Update, budget: usize, worklist: &mut Worklist) {
    dispose_expressions(update.path, budget, worklist);
    dispose_expression(update.value, budget, worklist);
}

fn detach_boxed(child: &mut Expression, budget: usize, worklist: &mut Worklist) {
    if !is_leaf_expression(child) {
        dispose_expression(mem::replace(child, Expression::Old), budget, worklist);
    }
}

fn detach_boxed_formula(child: &mut TemporalFormula, budget: usize, worklist: &mut Worklist) {
    if !is_leaf_formula(child) {
        dispose_formula(
            mem::replace(
                child,
                TemporalFormula::Atom {
                    expression: Expression::Old,
                },
            ),
            budget,
            worklist,
        );
    }
}

fn detach_domain(domain: &mut Domain, budget: usize, worklist: &mut Worklist) {
    match domain {
        Domain::Value { value } => detach_boxed(value, budget, worklist),
        Domain::Range { lower, upper } => {
            detach_boxed(lower, budget, worklist);
            detach_boxed(upper, budget, worklist);
        }
        Domain::Set { values } => dispose_expressions(mem::take(values), budget, worklist),
    }
}

fn detach_bindings(bounds: &mut Vec<Binding>, budget: usize, worklist: &mut Worklist) {
    for mut binding in mem::take(bounds) {
        detach_domain(&mut binding.domain, budget, worklist);
    }
}

fn detach_step(step: &mut Step, budget: usize, worklist: &mut Worklist) {
    match step {
        Step::Call {
            arguments,
            captures,
            ..
        } => {
            dispose_expressions(mem::take(arguments), budget, worklist);
            dispose_expressions(mem::take(captures), budget, worklist);
        }
        Step::Frame {
            arguments,
            captures,
            branches,
            ..
        } => {
            dispose_expressions(mem::take(arguments), budget, worklist);
            dispose_expressions(mem::take(captures), budget, worklist);
            dispose_branches(mem::take(branches), budget, worklist);
        }
        Step::Bind { domain, .. } | Step::Choose { domain, .. } => {
            detach_domain(domain, budget, worklist)
        }
        Step::Guard { expression }
        | Step::Assign {
            value: expression, ..
        } => detach_boxed(expression, budget, worklist),
    }
}

fn detach_branch(branch: &mut Branch, budget: usize, worklist: &mut Worklist) {
    for mut step in mem::take(&mut branch.steps) {
        detach_step(&mut step, budget, worklist);
    }
}

fn detach_value(value: &mut Value, budget: usize, worklist: &mut Worklist) {
    match value {
        Value::Null
        | Value::Bool(_)
        | Value::Int(_)
        | Value::String(_)
        | Value::Bytes(_)
        | Value::ModelValue(_) => {}
        Value::List(items) | Value::Set(items) => {
            for item in mem::take(items) {
                dispose_value(item, budget, worklist);
            }
        }
        Value::Record(fields) => {
            for (_, item) in mem::take(fields) {
                dispose_value(item, budget, worklist);
            }
        }
        Value::Map(entries) => {
            for (key, item) in mem::take(entries) {
                dispose_value(key, budget, worklist);
                dispose_value(item, budget, worklist);
            }
        }
    }
}

fn detach_expression(node: &mut Expression, budget: usize, worklist: &mut Worklist) {
    match node {
        Expression::Parameter { .. }
        | Expression::Capture { .. }
        | Expression::Bound { .. }
        | Expression::Old
        | Expression::UnmatchedCase { .. }
        | Expression::Literal { .. }
        | Expression::Read { .. } => {}
        Expression::Data { value } => {
            if !is_leaf_value(value) {
                dispose_value(mem::replace(value, Value::Null), budget, worklist);
            }
        }
        Expression::Frame {
            arguments,
            captures,
            body,
            ..
        } => {
            dispose_expressions(mem::take(arguments), budget, worklist);
            dispose_expressions(mem::take(captures), budget, worklist);
            detach_boxed(body, budget, worklist);
        }
        Expression::Call {
            arguments,
            captures,
            ..
        } => {
            dispose_expressions(mem::take(arguments), budget, worklist);
            dispose_expressions(mem::take(captures), budget, worklist);
        }
        Expression::Set { values } | Expression::Tuple { values } => {
            dispose_expressions(mem::take(values), budget, worklist)
        }
        Expression::Record { fields } | Expression::RecordSet { fields } => {
            dispose_fields(mem::take(fields), budget, worklist)
        }
        Expression::At { target, key } => {
            detach_boxed(target, budget, worklist);
            detach_boxed(key, budget, worklist);
        }
        Expression::Update { target, updates } => {
            detach_boxed(target, budget, worklist);
            for update in mem::take(updates) {
                dispose_update(update, budget, worklist);
            }
        }
        Expression::Forall { bounds, body }
        | Expression::Exists { bounds, body }
        | Expression::Function { bounds, body }
        | Expression::Filter { bounds, body }
        | Expression::MapSet { bounds, body } => {
            detach_bindings(bounds, budget, worklist);
            detach_boxed(body, budget, worklist);
        }
        Expression::Choose { binding, body } => {
            detach_domain(&mut binding.domain, budget, worklist);
            detach_boxed(body, budget, worklist);
        }
        Expression::Fold {
            initial,
            collection,
            body,
            ..
        } => {
            detach_boxed(initial, budget, worklist);
            detach_boxed(collection, budget, worklist);
            detach_boxed(body, budget, worklist);
        }
        Expression::SelectSeq { sequence, body, .. }
        | Expression::SortSeq { sequence, body, .. } => {
            detach_boxed(sequence, budget, worklist);
            detach_boxed(body, budget, worklist);
        }
        Expression::Assert { condition, message } => {
            detach_boxed(condition, budget, worklist);
            detach_boxed(message, budget, worklist);
        }
        Expression::Domain { argument }
        | Expression::VariantTag { argument }
        | Expression::Cardinality { argument }
        | Expression::SequenceSet { argument }
        | Expression::Len { argument }
        | Expression::Head { argument }
        | Expression::Tail { argument }
        | Expression::Powerset { argument }
        | Expression::Union { argument }
        | Expression::Prime { argument }
        | Expression::Not { argument }
        | Expression::Neg { argument } => detach_boxed(argument, budget, worklist),
        Expression::Cup { left, right }
        | Expression::Cap { left, right }
        | Expression::Difference { left, right }
        | Expression::SubsetEq { left, right }
        | Expression::Append { left, right }
        | Expression::Concat { left, right }
        | Expression::FunctionSet { left, right }
        | Expression::FunctionIn { left, right }
        | Expression::SequenceIn { left, right }
        | Expression::Add { left, right }
        | Expression::Sub { left, right }
        | Expression::Mul { left, right }
        | Expression::Div { left, right }
        | Expression::Mod { left, right }
        | Expression::Pow { left, right }
        | Expression::Eq { left, right }
        | Expression::Ne { left, right }
        | Expression::Lt { left, right }
        | Expression::Le { left, right }
        | Expression::Gt { left, right }
        | Expression::Ge { left, right }
        | Expression::And { left, right }
        | Expression::Or { left, right }
        | Expression::Implies { left, right } => {
            detach_boxed(left, budget, worklist);
            detach_boxed(right, budget, worklist);
        }
        Expression::Subseq { sequence, from, to } => {
            detach_boxed(sequence, budget, worklist);
            detach_boxed(from, budget, worklist);
            detach_boxed(to, budget, worklist);
        }
        Expression::If {
            condition,
            consequent,
            alternative,
        } => {
            detach_boxed(condition, budget, worklist);
            detach_boxed(consequent, budget, worklist);
            detach_boxed(alternative, budget, worklist);
        }
        Expression::Enabled {
            branches,
            subscript,
        } => {
            dispose_branches(mem::take(branches), budget, worklist);
            if let Some(child) = subscript.take() {
                dispose_expression(*child, budget, worklist);
            }
        }
        Expression::In { value, domain } => {
            detach_boxed(value, budget, worklist);
            detach_domain(domain, budget, worklist);
        }
    }
}

fn detach_formula(node: &mut TemporalFormula, budget: usize, worklist: &mut Worklist) {
    match node {
        TemporalFormula::Call {
            arguments,
            captures,
            ..
        } => {
            dispose_expressions(mem::take(arguments), budget, worklist);
            dispose_expressions(mem::take(captures), budget, worklist);
        }
        TemporalFormula::Frame {
            arguments,
            captures,
            formula,
            ..
        } => {
            dispose_expressions(mem::take(arguments), budget, worklist);
            dispose_expressions(mem::take(captures), budget, worklist);
            detach_boxed_formula(formula, budget, worklist);
        }
        TemporalFormula::Atom { expression } => detach_boxed(expression, budget, worklist),
        TemporalFormula::Not { formula }
        | TemporalFormula::Always { formula }
        | TemporalFormula::Eventually { formula } => {
            detach_boxed_formula(formula, budget, worklist)
        }
        TemporalFormula::And { left, right } | TemporalFormula::Or { left, right } => {
            detach_boxed_formula(left, budget, worklist);
            detach_boxed_formula(right, budget, worklist);
        }
        TemporalFormula::Forall { bounds, formula }
        | TemporalFormula::Exists { bounds, formula } => {
            detach_bindings(bounds, budget, worklist);
            detach_boxed_formula(formula, budget, worklist);
        }
    }
}

fn release(detach: impl FnOnce(&mut Worklist)) {
    let mut worklist = Worklist::new();
    detach(&mut worklist);
    while let Some(detached) = worklist.pop() {
        match detached {
            Detached::Expression(mut node) => {
                detach_expression(&mut node, DEPTH_BUDGET, &mut worklist)
            }
            Detached::Branch(mut node) => detach_branch(&mut node, DEPTH_BUDGET, &mut worklist),
            Detached::Formula(mut node) => detach_formula(&mut node, DEPTH_BUDGET, &mut worklist),
            Detached::Value(mut node) => detach_value(&mut node, DEPTH_BUDGET, &mut worklist),
        }
    }
}

#[inline(never)]
fn release_value(value: &mut Value) {
    release(|worklist| detach_value(value, DEPTH_BUDGET, worklist));
}

impl Drop for Expression {
    fn drop(&mut self) {
        if !is_leaf_expression(self) {
            release(|worklist| detach_expression(self, DEPTH_BUDGET, worklist));
        }
    }
}

impl Drop for Branch {
    fn drop(&mut self) {
        if !is_leaf_branch(self) {
            release(|worklist| detach_branch(self, DEPTH_BUDGET, worklist));
        }
    }
}

impl Drop for TemporalFormula {
    fn drop(&mut self) {
        if !is_leaf_formula(self) {
            release(|worklist| detach_formula(self, DEPTH_BUDGET, worklist));
        }
    }
}

impl Drop for Value {
    #[inline]
    fn drop(&mut self) {
        if !is_shallow_value(self) {
            release_value(self);
        }
    }
}
