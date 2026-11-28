//! Printers of candidate syntaxes for the readability loop (plan 0098).
//! A printer only writes text; no parser exists for a candidate until it is
//! adopted.
//!
//! v0 Quint-like: `type` for sorts and enums, `const` for configuration and
//! `var` for state, `Set[T]` and `A -> Set[B]` types with multiplicity
//! notes, method calls for set and relation operators, `S.forall(x => ...)`,
//! `action a(x: T) = all { ... }` with an implicit frame, and `invariant` /
//! `reachable` properties.
use idealis_syntax::{
    Arith, CheckKind, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Quantifier, SetOp,
    Shape,
};
use std::collections::BTreeSet;

pub fn quint_like(model: &Model) -> String {
    let mut printer = QuintLike {
        model,
        atoms: BTreeSet::new(),
    };
    printer.module()
}

struct QuintLike<'a> {
    model: &'a Model,
    /// Names bound to single atoms (parameters and quantified variables).
    atoms: BTreeSet<String>,
}

fn note(mult: Mult, binary: bool) -> &'static str {
    match (mult, binary) {
        (Mult::Set, _) => "",
        (Mult::Lone, false) => "  // at most one element",
        (Mult::One, false) => "  // exactly one element",
        (Mult::Some, false) => "  // at least one element",
        (Mult::Lone, true) => "  // each key relates to at most one value",
        (Mult::One, true) => "  // each key relates to exactly one value",
        (Mult::Some, true) => "  // each key relates to at least one value",
    }
}

impl QuintLike<'_> {
    fn module(&mut self) -> String {
        let model = self.model;
        let mut out = format!("module {} {{\n", model.name);
        for sig in &model.sigs {
            out.push_str(&format!("  type {sig}\n"));
        }
        for (name, members) in &model.enums {
            out.push_str(&format!("  type {name} = {}\n", members.join(" | ")));
        }
        // A union of sorts, not a tagged sum: `+` as for sets (plan 0100).
        for (name, children) in &model.sums {
            out.push_str(&format!("  type {name} = {}\n", children.join(" + ")));
        }
        for record in &model.records {
            let fields: Vec<String> = record
                .fields
                .iter()
                .map(|(field, sort)| format!("{field}: {sort}"))
                .collect();
            out.push_str(&format!(
                "  type {} = {{ {} }}\n",
                record.name,
                fields.join(", ")
            ));
        }
        for relation in &model.relations {
            let keyword = if relation.var { "var" } else { "const" };
            let (kind, mult, binary) = match &relation.shape {
                Shape::Unary { sig, mult } => (format!("Set[{sig}]"), *mult, false),
                Shape::Binary { from, to, mult } => (format!("{from} -> Set[{to}]"), *mult, true),
            };
            out.push_str(&format!(
                "  {keyword} {}: {kind}{}\n",
                relation.name,
                note(mult, binary)
            ));
        }
        for fact in &model.facts {
            out.push_str(&format!("  assume {}\n", self.formula(fact)));
        }
        if !model.init.is_empty() {
            let parts: Vec<String> = model.init.iter().map(|f| self.formula(f)).collect();
            out.push_str(&format!("  action init = all {{ {} }}\n", parts.join(", ")));
        }
        for action in &model.actions {
            for (param, _) in &action.params {
                self.atoms.insert(param.clone());
            }
            let params: Vec<String> = action
                .params
                .iter()
                .map(|(name, sig)| format!("{name}: {sig}"))
                .collect();
            let mut parts: Vec<String> = action.guards.iter().map(|g| self.formula(g)).collect();
            for (var, value) in &action.assignments {
                parts.push(format!("{var}' = {}", self.expression(value)));
            }
            out.push_str(&format!(
                "  action {}({}) = all {{\n{}\n  }}\n",
                action.name,
                params.join(", "),
                parts
                    .iter()
                    .map(|part| format!("    {part},"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
            for (param, _) in &action.params {
                self.atoms.remove(param);
            }
        }
        for check in &model.checks {
            let keyword = match check.kind {
                CheckKind::Invariant => "invariant",
                CheckKind::Reachable => "reachable",
            };
            out.push_str(&format!(
                "  {keyword} {} = {}\n",
                check.name,
                self.formula(&check.formula)
            ));
        }
        let mut scope: Vec<String> = model
            .scope
            .sizes
            .iter()
            .map(|(sig, size)| format!("{sig} = {size}"))
            .collect();
        if let Some(steps) = model.scope.steps {
            scope.push(format!("steps = {steps}"));
        }
        if !scope.is_empty() {
            out.push_str(&format!("  check {{ {} }}\n", scope.join(", ")));
        }
        out.push_str("}\n");
        out
    }
    fn is_atom(&self, expression: &Expr) -> bool {
        match expression {
            Expr::Name(name) => self.atoms.contains(name) || self.model.member(name).is_some(),
            _ => false,
        }
    }
    fn is_pair(&self, expression: &Expr) -> bool {
        match expression {
            Expr::None => false,
            Expr::Name(name) => {
                !self.atoms.contains(name)
                    && matches!(
                        self.model.relation(name).map(|r| &r.shape),
                        Some(Shape::Binary { .. })
                    )
            }
            Expr::Binary(SetOp::Product | SetOp::WithoutKeys, _, _) | Expr::Closure(_) => true,
            Expr::Binary(SetOp::Join, left, right) => self.is_pair(left) && self.is_pair(right),
            Expr::Binary(_, left, right) => self.is_pair(left) || self.is_pair(right),
            Expr::At(_, expr) => self.is_pair(expr),
            Expr::Field(receiver, _) => self.is_pair(receiver),
            Expr::If(_, yes, no) => self.is_pair(yes) || self.is_pair(no),
            Expr::Literal(_)
            | Expr::Count(_)
            | Expr::Arith(..)
            | Expr::Record(..)
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
    /// A set-valued expression; a single atom is written `Set(x)`.
    fn set(&mut self, expression: &Expr) -> String {
        if self.is_atom(expression) {
            return format!("Set({})", self.expression(expression));
        }
        self.expression(expression)
    }
    fn expression(&mut self, expression: &Expr) -> String {
        match expression {
            Expr::Select(variable, sort, predicate) => {
                self.atoms.insert(variable.clone());
                let body = self.formula(predicate);
                self.atoms.remove(variable);
                format!("{sort}.filter({variable} => {body})")
            }
            Expr::If(condition, yes, no) => format!(
                "(if ({}) {} else {})",
                self.formula(condition),
                self.expression(yes),
                self.expression(no)
            ),
            Expr::None => "Set()".into(),
            Expr::Name(name) => name.clone(),
            // Values (plan 0103), written as in Quint.
            Expr::At(phase, expr) => {
                let method = if *phase == crate::Phase::After {
                    "after"
                } else {
                    "before"
                };
                format!("{method}({})", self.expression(expr))
            }
            Expr::Literal(value) => value.to_string(),
            Expr::Count(counted) => format!("{}.size()", self.set(counted)),
            Expr::Arith(operator, left, right) => {
                let symbol = match operator {
                    Arith::Add => "+",
                    Arith::Sub => "-",
                    Arith::Mul => "*",
                };
                format!(
                    "({} {symbol} {})",
                    self.expression(left),
                    self.expression(right)
                )
            }
            Expr::Sequence(_, values) => format!(
                "List({})",
                values
                    .iter()
                    .map(|v| self.expression(v))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Expr::Append(seq, value) => format!(
                "{}.append({})",
                self.expression(seq),
                self.expression(value)
            ),
            Expr::Tail(seq) => format!("tail({})", self.expression(seq)),
            Expr::Take(seq, count) => {
                format!("take({}, {})", self.expression(seq), self.expression(count))
            }
            Expr::Elements(seq) => format!("{}.elements()", self.expression(seq)),
            Expr::Length(seq) => format!("{}.length()", self.expression(seq)),
            Expr::RemoveAll(seq, excluded) => {
                format!(
                    "removeAll({}, {})",
                    self.expression(seq),
                    self.expression(excluded)
                )
            }
            Expr::Closure(edges) => format!("closure({})", self.set(edges)),
            Expr::Keys(relation) => format!("{}.map(row => row._1)", self.set(relation)),
            Expr::ReachableFrom(roots, edges) => format!(
                "reachableFrom({}, {})",
                self.set(roots),
                self.expression(edges)
            ),
            Expr::RemoveAt(seq, index) => {
                format!(
                    "removeAt({}, {})",
                    self.expression(seq),
                    self.expression(index)
                )
            }
            Expr::Index(seq, index) => {
                format!("at({}, {})", self.expression(seq), self.expression(index))
            }
            Expr::Variant(tag, payload) => format!(
                "{tag}({})",
                payload
                    .as_ref()
                    .map_or(String::new(), |v| self.expression(v))
            ),
            Expr::Payload(value, tag) => format!("payload({}, {tag})", self.expression(value)),
            Expr::Field(receiver, field) => {
                let value = if self.is_pair(receiver) {
                    format!("(row._1, row._2.{field})")
                } else {
                    format!("row.{field}")
                };
                format!("{}.map(row => {value})", self.set(receiver))
            }
            Expr::Record(name, values) => {
                let record = self.model.record(name).expect("a declared record");
                let fields: Vec<String> = record
                    .fields
                    .iter()
                    .zip(values)
                    .map(|((field, _), value)| format!("{field}: {}", self.expression(value)))
                    .collect();
                format!("{{ {} }}", fields.join(", "))
            }
            Expr::Binary(SetOp::Multiply, ..) => unreachable!("the checker rewrites *"),
            Expr::Binary(operator, left, right) => match operator {
                SetOp::Union | SetOp::SetUnion => {
                    format!("{}.union({})", self.set(left), self.set(right))
                }
                SetOp::Override => format!(
                    "{}.override({})",
                    self.expression(left),
                    self.expression(right)
                ),
                SetOp::WithoutKeys => format!(
                    "{}.filter(row => not({}.contains(row._1)))",
                    self.set(left),
                    self.set(right)
                ),
                SetOp::Difference | SetOp::SetDifference => {
                    format!("{}.exclude({})", self.set(left), self.set(right))
                }
                SetOp::Intersection => format!("{}.intersect({})", self.set(left), self.set(right)),
                SetOp::Product => format!("tuples({}, {})", self.set(left), self.set(right)),
                SetOp::Multiply => unreachable!("the checker rewrites *"),
                SetOp::Join => match (self.is_pair(left), self.is_pair(right)) {
                    (false, true) => {
                        format!("{}.image({})", self.expression(right), self.set(left))
                    }
                    (true, false) => {
                        format!("{}.preimage({})", self.expression(left), self.set(right))
                    }
                    _ => format!(
                        "{}.compose({})",
                        self.expression(left),
                        self.expression(right)
                    ),
                },
            },
        }
    }
    fn formula(&mut self, formula: &Formula) -> String {
        match formula {
            Formula::Compare(operator, left, right) => {
                let element = self.is_atom(left);
                let (l, r) = (self.set(left), self.set(right));
                match (operator, element) {
                    (Compare::In, true) => format!("{r}.contains({})", self.expression(left)),
                    (Compare::NotIn, true) => {
                        format!("not({r}.contains({}))", self.expression(left))
                    }
                    (Compare::In, false) => format!("{l}.subseteq({r})"),
                    (Compare::NotIn, false) => format!("not({l}.subseteq({r}))"),
                    (Compare::Equal, _) => format!("{l} == {r}"),
                    (Compare::NotEqual, _) => format!("{l} != {r}"),
                }
            }
            Formula::Ints(operator, left, right) => {
                let mut int = |int: &Int| match int {
                    Int::Count(counted) => format!("{}.size()", self.set(counted)),
                    Int::Literal(value) => value.to_string(),
                    Int::Value(value) | Int::LocatedValue(value, _) => self.expression(value),
                };
                let symbol = match operator {
                    IntCompare::Less => "<",
                    IntCompare::LessEqual => "<=",
                    IntCompare::Greater => ">",
                    IntCompare::GreaterEqual => ">=",
                    IntCompare::Equal => "==",
                    IntCompare::NotEqual => "!=",
                };
                format!("{} {symbol} {}", int(left), int(right))
            }
            Formula::Mult(mult, counted) => {
                let set = self.set(counted);
                match mult {
                    Mult::Some => format!("{set}.nonEmpty()"),
                    Mult::One => format!("{set}.size() == 1"),
                    Mult::Lone => format!("{set}.size() <= 1"),
                    Mult::Set => "true".into(),
                }
            }
            Formula::HasTag(value, tag) => format!("is({}, {tag})", self.expression(value)),
            Formula::Empty(counted) => format!("{}.isEmpty()", self.set(counted)),
            Formula::Not(inner) => format!("not({})", self.formula(inner)),
            Formula::And(parts) if parts.is_empty() => "true".into(),
            Formula::And(parts) if parts.len() == 1 => self.formula(&parts[0]),
            Formula::And(parts) => {
                let parts: Vec<String> = parts.iter().map(|p| self.formula(p)).collect();
                format!("and {{ {} }}", parts.join(", "))
            }
            Formula::Or(left, right) => {
                format!("or {{ {}, {} }}", self.formula(left), self.formula(right))
            }
            Formula::Implies(left, right) => {
                format!("({}).implies({})", self.formula(left), self.formula(right))
            }
            Formula::Quantified(quantifier, variable, sig, body) => {
                self.atoms.insert(variable.clone());
                let body = self.formula(body);
                self.atoms.remove(variable);
                let word = match quantifier {
                    Quantifier::All => "forall",
                    Quantifier::Some => "exists",
                };
                format!("{sig}.{word}({variable} => {body})")
            }
        }
    }
}
