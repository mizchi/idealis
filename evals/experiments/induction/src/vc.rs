//! Verification conditions of an Idealis model in SMT-LIB (plan 0106).
//! Signatures are uninterpreted sorts, so an unsat answer covers every
//! size; enums are finite datatypes; relations are Boolean functions, and
//! an expression becomes the first-order formula "this tuple belongs to it".
use idealis_model::{Compare, Expr, Formula, Model, Mult, Quantifier, SetOp, Shape};
use std::collections::BTreeMap;

/// The fragment the prototype encodes, or the reason a model is outside.
pub fn supported(model: &Model) -> Result<(), String> {
    if model.has_values() {
        return Err("integer and record values".into());
    }
    if !model.sums.is_empty() {
        return Err("union sorts".into());
    }
    if model
        .enums
        .iter()
        .any(|(name, _)| name == idealis_model::BOOL)
    {
        return Err("Bool".into());
    }
    if rmodel_uses_counts(model) {
        return Err("# counts are not first-order".into());
    }
    Ok(())
}

fn rmodel_uses_counts(model: &Model) -> bool {
    fn formula(f: &Formula) -> bool {
        match f {
            Formula::Ints(..) => true,
            Formula::Not(inner) | Formula::Quantified(_, _, _, inner) => formula(inner),
            Formula::And(parts) => parts.iter().any(formula),
            Formula::Or(l, r) | Formula::Implies(l, r) => formula(l) || formula(r),
            _ => false,
        }
    }
    model
        .facts
        .iter()
        .chain(&model.init)
        .chain(model.checks.iter().map(|c| &c.formula))
        .chain(model.actions.iter().flat_map(|a| &a.guards))
        .any(formula)
}

#[derive(Clone, Debug, PartialEq)]
enum Ty {
    Any,
    Set(String),
    Pair(String, String),
}

impl Ty {
    fn columns(&self) -> Vec<String> {
        match self {
            Ty::Any => Vec::new(),
            Ty::Set(a) => vec![a.clone()],
            Ty::Pair(a, b) => vec![a.clone(), b.clone()],
        }
    }
}

/// Which copy of the vars an expression reads.
#[derive(Clone, Copy, PartialEq)]
pub enum Copy_ {
    Pre,
    Post,
}

pub struct Encoder<'a> {
    pub model: &'a Model,
    fresh: usize,
}

fn shape_ty(shape: &Shape) -> Ty {
    match shape {
        Shape::Unary { sig, .. } => Ty::Set(sig.clone()),
        Shape::Binary { from, to, .. } => Ty::Pair(from.clone(), to.clone()),
    }
}

/// Bound names: a variable's SMT name and sort.
pub type Scope = BTreeMap<String, (String, String)>;

impl<'a> Encoder<'a> {
    pub fn new(model: &'a Model) -> Self {
        Self { model, fresh: 0 }
    }
    fn var(&mut self) -> String {
        self.fresh += 1;
        format!("x{}", self.fresh)
    }
    pub fn relation_name(&self, name: &str, copy: Copy_) -> String {
        let var = self.model.relation(name).is_some_and(|r| r.var);
        if var && copy == Copy_::Post {
            format!("r_{name}_post")
        } else {
            format!("r_{name}")
        }
    }
    fn ty(&self, expression: &Expr, scope: &Scope) -> Ty {
        match expression {
            Expr::None => Ty::Any,
            Expr::Name(name) => {
                if let Some((_, sort)) = scope.get(name) {
                    Ty::Set(sort.clone())
                } else if let Some(relation) = self.model.relation(name) {
                    shape_ty(&relation.shape)
                } else if let Some(enumeration) = self.model.member(name) {
                    Ty::Set(enumeration.to_owned())
                } else {
                    Ty::Set(name.clone())
                }
            }
            Expr::Binary(op, l, r) => {
                let (l, r) = (self.ty(l, scope), self.ty(r, scope));
                match op {
                    SetOp::Product => {
                        let mut columns = l.columns();
                        columns.extend(r.columns());
                        match columns.as_slice() {
                            [a, b] => Ty::Pair(a.clone(), b.clone()),
                            _ => Ty::Any,
                        }
                    }
                    SetOp::Join => match (l, r) {
                        (Ty::Set(_), Ty::Pair(_, c)) => Ty::Set(c),
                        (Ty::Pair(a, _), Ty::Set(_)) => Ty::Set(a),
                        (Ty::Pair(a, _), Ty::Pair(_, d)) => Ty::Pair(a, d),
                        _ => Ty::Any,
                    },
                    _ => {
                        if l == Ty::Any {
                            r
                        } else {
                            l
                        }
                    }
                }
            }
            _ => Ty::Any,
        }
    }
    /// The middle sort of a join.
    fn middle(&self, left: &Expr, right: &Expr, scope: &Scope) -> String {
        match (self.ty(left, scope), self.ty(right, scope)) {
            (Ty::Set(a), _) => a,
            (Ty::Pair(_, b), _) => b,
            (Ty::Any, Ty::Pair(b, _)) => b,
            _ => unreachable!("a typed join"),
        }
    }
    /// `tuple ∈ expression`.
    pub fn member(
        &mut self,
        expression: &Expr,
        tuple: &[String],
        scope: &Scope,
        copy: Copy_,
    ) -> String {
        match expression {
            Expr::None => "false".into(),
            Expr::Name(name) => {
                if let Some((smt, _)) = scope.get(name) {
                    format!("(= {} {smt})", tuple[0])
                } else if self.model.relation(name).is_some() {
                    format!("({} {})", self.relation_name(name, copy), tuple.join(" "))
                } else if self.model.member(name).is_some() {
                    format!("(= {} e_{name})", tuple[0])
                } else {
                    "true".into()
                }
            }
            Expr::Binary(op, l, r) => match op {
                SetOp::Union | SetOp::SetUnion => format!(
                    "(or {} {})",
                    self.member(l, tuple, scope, copy),
                    self.member(r, tuple, scope, copy)
                ),
                SetOp::Override => {
                    let Ty::Pair(_, sort) = self.ty(expression, scope) else {
                        unreachable!("typed binary override");
                    };
                    let column = self.var();
                    let row = [tuple[0].clone(), column.clone()];
                    format!(
                        "(or {} (and {} (not (exists (({column} S_{sort})) {}))))",
                        self.member(r, tuple, scope, copy),
                        self.member(l, tuple, scope, copy),
                        self.member(r, &row, scope, copy)
                    )
                }
                SetOp::Difference | SetOp::SetDifference => format!(
                    "(and {} (not {}))",
                    self.member(l, tuple, scope, copy),
                    self.member(r, tuple, scope, copy)
                ),
                SetOp::WithoutKeys => format!(
                    "(and {} (not {}))",
                    self.member(l, tuple, scope, copy),
                    self.member(r, &tuple[..1], scope, copy)
                ),
                SetOp::Intersection => format!(
                    "(and {} {})",
                    self.member(l, tuple, scope, copy),
                    self.member(r, tuple, scope, copy)
                ),
                SetOp::Product => {
                    let width = self.ty(l, scope).columns().len().max(1);
                    format!(
                        "(and {} {})",
                        self.member(l, &tuple[..width], scope, copy),
                        self.member(r, &tuple[width..], scope, copy)
                    )
                }
                SetOp::Join => {
                    let middle = self.middle(l, r, scope);
                    let m = self.var();
                    let (lt, rt) = (self.ty(l, scope), self.ty(r, scope));
                    let left_tuple: Vec<String> = match lt {
                        Ty::Pair(..) => vec![tuple[0].clone(), m.clone()],
                        _ => vec![m.clone()],
                    };
                    let right_tuple: Vec<String> = match rt {
                        Ty::Pair(..) => {
                            let last = tuple.last().cloned().unwrap_or_default();
                            if matches!(lt, Ty::Pair(..)) {
                                vec![m.clone(), tuple[1].clone()]
                            } else {
                                vec![m.clone(), last]
                            }
                        }
                        _ => vec![m.clone()],
                    };
                    format!(
                        "(exists (({m} S_{middle})) (and {} {}))",
                        self.member(l, &left_tuple, scope, copy),
                        self.member(r, &right_tuple, scope, copy)
                    )
                }
                SetOp::Multiply => unreachable!("no values in the fragment"),
            },
            _ => unreachable!("no values in the fragment"),
        }
    }
    fn binders(&mut self, columns: &[String]) -> (Vec<String>, String) {
        let names: Vec<String> = columns.iter().map(|_| self.var()).collect();
        let binders: Vec<String> = names
            .iter()
            .zip(columns)
            .map(|(n, s)| format!("({n} S_{s})"))
            .collect();
        (names, binders.join(" "))
    }
    pub fn formula(&mut self, formula: &Formula, scope: &Scope, copy: Copy_) -> String {
        match formula {
            Formula::Compare(op, l, r) => {
                let ty = match self.ty(l, scope) {
                    Ty::Any => self.ty(r, scope),
                    other => other,
                };
                if ty == Ty::Any {
                    // none against none.
                    return match op {
                        Compare::In | Compare::Equal => "true".into(),
                        _ => "false".into(),
                    };
                }
                let (names, binders) = self.binders(&ty.columns());
                let (ml, mr) = (
                    self.member(l, &names, scope, copy),
                    self.member(r, &names, scope, copy),
                );
                match op {
                    Compare::In => format!("(forall ({binders}) (=> {ml} {mr}))"),
                    Compare::NotIn => format!("(not (forall ({binders}) (=> {ml} {mr})))"),
                    Compare::Equal => format!("(forall ({binders}) (= {ml} {mr}))"),
                    Compare::NotEqual => format!("(not (forall ({binders}) (= {ml} {mr})))"),
                }
            }
            Formula::Empty(e) if self.ty(e, scope) == Ty::Any => "true".into(),
            Formula::Mult(mult, e) if self.ty(e, scope) == Ty::Any => {
                matches!(mult, Mult::Lone | Mult::Set).to_string()
            }
            Formula::Empty(e) => {
                let ty = self.ty(e, scope);
                let (names, binders) = self.binders(&ty.columns());
                format!(
                    "(forall ({binders}) (not {}))",
                    self.member(e, &names, scope, copy)
                )
            }
            Formula::Mult(mult, e) => {
                let ty = self.ty(e, scope);
                self.multiplicity(*mult, &ty.columns(), &mut |enc, tuple| {
                    enc.member(e, tuple, scope, copy)
                })
            }
            Formula::Ints(..) | Formula::HasTag(..) => unreachable!("outside the fragment"),
            Formula::Not(inner) => format!("(not {})", self.formula(inner, scope, copy)),
            Formula::And(parts) if parts.is_empty() => "true".into(),
            Formula::And(parts) => {
                let parts: Vec<String> =
                    parts.iter().map(|p| self.formula(p, scope, copy)).collect();
                format!("(and {})", parts.join(" "))
            }
            Formula::Or(l, r) => format!(
                "(or {} {})",
                self.formula(l, scope, copy),
                self.formula(r, scope, copy)
            ),
            Formula::Implies(l, r) => format!(
                "(=> {} {})",
                self.formula(l, scope, copy),
                self.formula(r, scope, copy)
            ),
            Formula::Quantified(q, variable, sort, body) => {
                let smt = self.var();
                let mut inner = scope.clone();
                inner.insert(variable.clone(), (smt.clone(), sort.clone()));
                let body = self.formula(body, &inner, copy);
                let word = match q {
                    Quantifier::All => "forall",
                    Quantifier::Some => "exists",
                };
                format!("({word} (({smt} S_{sort})) {body})")
            }
        }
    }
    /// A multiplicity over the tuples satisfying `member`.
    fn multiplicity(
        &mut self,
        mult: Mult,
        columns: &[String],
        member: &mut dyn FnMut(&mut Self, &[String]) -> String,
    ) -> String {
        let (a, ab) = self.binders(columns);
        let (b, bb) = self.binders(columns);
        let some = format!("(exists ({ab}) {})", member(self, &a));
        let equal: Vec<String> = a
            .iter()
            .zip(&b)
            .map(|(x, y)| format!("(= {x} {y})"))
            .collect();
        let lone = format!(
            "(forall ({ab} {bb}) (=> (and {} {}) (and {})))",
            member(self, &a),
            member(self, &b),
            equal.join(" ")
        );
        match mult {
            Mult::Set => "true".into(),
            Mult::Some => some,
            Mult::Lone => lone,
            Mult::One => format!("(and {some} {lone})"),
        }
    }
    /// A relation's declared multiplicity in one copy of the state.
    pub fn declared(&mut self, name: &str, copy: Copy_) -> String {
        let relation = self.model.relation(name).expect("a relation");
        let smt = self.relation_name(name, copy);
        match relation.shape.clone() {
            Shape::Unary { sig, mult } => {
                self.multiplicity(mult, &[sig], &mut |_, t| format!("({smt} {})", t[0]))
            }
            Shape::Binary { from, to, mult } => {
                if mult == Mult::Set {
                    return "true".into();
                }
                let k = self.var();
                let inner =
                    self.multiplicity(mult, &[to], &mut |_, t| format!("({smt} {k} {})", t[0]));
                format!("(forall (({k} S_{from})) {inner})")
            }
        }
    }
}

/// The declarations shared by every VC.
pub fn prelude(model: &Model) -> String {
    let mut out = String::from("(set-option :produce-models true)\n");
    for sig in &model.sigs {
        out.push_str(&format!("(declare-sort S_{sig} 0)\n"));
    }
    for (name, members) in &model.enums {
        let constructors: Vec<String> = members.iter().map(|m| format!("(e_{m})")).collect();
        out.push_str(&format!(
            "(declare-datatypes ((S_{name} 0)) (({})))\n",
            constructors.join(" ")
        ));
    }
    for relation in &model.relations {
        let columns: Vec<String> = shape_ty(&relation.shape)
            .columns()
            .iter()
            .map(|s| format!("S_{s}"))
            .collect();
        out.push_str(&format!(
            "(declare-fun r_{} ({}) Bool)\n",
            relation.name,
            columns.join(" ")
        ));
        if relation.var {
            out.push_str(&format!(
                "(declare-fun r_{}_post ({}) Bool)\n",
                relation.name,
                columns.join(" ")
            ));
        }
    }
    out
}

/// One proof obligation: its name and SMT-LIB assertions (unsat = holds).
pub struct Obligation {
    pub check: String,
    /// `init`, or the action's name.
    pub step: String,
    pub smt: String,
}

pub fn obligations(model: &Model) -> Vec<Obligation> {
    let mut out = Vec::new();
    let invariants: Vec<_> = model
        .checks
        .iter()
        .filter(|c| c.kind == idealis_model::CheckKind::Invariant)
        .collect();
    let none = Scope::new();
    let base = |enc: &mut Encoder, initial: bool| -> Vec<String> {
        let mut assertions = Vec::new();
        for fact in &model.facts {
            assertions.push(enc.formula(fact, &none, Copy_::Pre));
        }
        for relation in &model.relations {
            assertions.push(enc.declared(&relation.name, Copy_::Pre));
        }
        if initial {
            for formula in &model.init {
                assertions.push(enc.formula(formula, &none, Copy_::Pre));
            }
        } else {
            for check in &invariants {
                assertions.push(enc.formula(&check.formula, &none, Copy_::Pre));
            }
        }
        assertions
    };
    // Each action instance: parameters as constants, the guards, and the
    // post-state defined by the assignments and the implicit frame.
    let step =
        |enc: &mut Encoder, action: &idealis_model::Action| -> (String, Vec<String>, Scope) {
            let mut decls = String::new();
            let mut scope = Scope::new();
            for (param, sort) in &action.params {
                decls.push_str(&format!("(declare-const p_{param} S_{sort})\n"));
                scope.insert(param.clone(), (format!("p_{param}"), sort.clone()));
            }
            let mut assertions = Vec::new();
            for guard in &action.guards {
                assertions.push(enc.formula(guard, &scope, Copy_::Pre));
            }
            for var in model.vars() {
                let columns = shape_ty(&var.shape).columns();
                let (names, binders) = enc.binders(&columns);
                let post = format!("(r_{}_post {})", var.name, names.join(" "));
                let value = match action.assignments.iter().find(|(v, _)| v == &var.name) {
                    Some((_, value)) => enc.member(value, &names, &scope, Copy_::Pre),
                    // Artificial fault 2 of plan 0106 leaves it unconstrained.
                    None if std::env::var("INDUCTION_MUTANT").as_deref() == Ok("frame") => {
                        continue;
                    }
                    None => format!("(r_{} {})", var.name, names.join(" ")),
                };
                assertions.push(format!("(forall ({binders}) (= {post} {value}))"));
            }
            (decls, assertions, scope)
        };
    let render = |decls: &str, assertions: &[String], goal: &str| {
        let mut smt = prelude(model);
        smt.push_str(decls);
        for a in assertions {
            smt.push_str(&format!("(assert {a})\n"));
        }
        smt.push_str(&format!("(assert (not {goal}))\n(check-sat)\n"));
        smt
    };
    for check in &invariants {
        let mut enc = Encoder::new(model);
        let assertions = base(&mut enc, true);
        let goal = enc.formula(&check.formula, &none, Copy_::Pre);
        out.push(Obligation {
            check: check.name.clone(),
            step: "init".into(),
            smt: render("", &assertions, &goal),
        });
        for action in &model.actions {
            let mut enc = Encoder::new(model);
            let mut assertions = base(&mut enc, false);
            let (decls, steps, _) = step(&mut enc, action);
            assertions.extend(steps);
            // Checks look only at states within the multiplicities (0099).
            // Artificial fault 1 of plan 0106 drops this.
            if std::env::var("INDUCTION_MUTANT").as_deref() != Ok("post-multiplicity") {
                for var in model.vars() {
                    assertions.push(enc.declared(&var.name, Copy_::Post));
                }
            }
            let goal = enc.formula(&check.formula, &none, Copy_::Post);
            out.push(Obligation {
                check: check.name.clone(),
                step: action.name.clone(),
                smt: render(&decls, &assertions, &goal),
            });
        }
    }
    // Declarations: the post-state keeps the vars' multiplicities.
    for action in &model.actions {
        let mut enc = Encoder::new(model);
        let mut assertions = base(&mut enc, false);
        let (decls, steps, _) = step(&mut enc, action);
        assertions.extend(steps);
        let goals: Vec<String> = model
            .vars()
            .map(|var| var.name.clone())
            .collect::<Vec<_>>()
            .iter()
            .map(|name| enc.declared(name, Copy_::Post))
            .collect();
        let goal = if goals.is_empty() {
            "true".to_owned()
        } else {
            format!("(and {})", goals.join(" "))
        };
        out.push(Obligation {
            check: idealis_model::DECLARATIONS.into(),
            step: action.name.clone(),
            smt: render(&decls, &assertions, &goal),
        });
    }
    out
}
