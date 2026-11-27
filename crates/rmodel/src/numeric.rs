//! Arithmetic is exact for admitted models. The finite enumeration scope
//! bounds inputs to operations; intermediate expressions have their own limit.
use crate::{Arith, Error, Expr, Formula, Int, Model, SetOp};
use std::ops::RangeInclusive;

pub(crate) fn validate_predicate(model: &Model, formula: &Formula) -> Result<(), Error> {
    Arithmetic {
        model,
        limits: i64::MIN..=i64::MAX,
    }
    .formula(formula)
}

pub(crate) fn validate_binding(model: &Model, expression: &Expr) -> Result<(), Error> {
    Arithmetic {
        model,
        limits: i64::MIN..=i64::MAX,
    }
    .expression(expression)
}

pub(crate) fn validate(model: &Model, limits: RangeInclusive<i64>) -> Result<(), Error> {
    let check = Arithmetic { model, limits };
    if let Some((lo, hi)) = model.scope.ints {
        check.require((i128::from(lo), i128::from(hi)), "integer scope")?;
    }
    for formula in model
        .facts
        .iter()
        .chain(&model.init)
        .chain(model.checks.iter().map(|check| &check.formula))
        .chain(
            model
                .temporal
                .iter()
                .flat_map(|check| std::iter::once(&check.formula).chain(&check.trigger)),
        )
        .chain(model.transitions.iter().map(|check| &check.formula))
    {
        check.formula(formula)?;
    }
    for action in &model.actions {
        for guard in &action.guards {
            check.formula(guard)?;
        }
        for (_, value) in &action.assignments {
            check.expression(value)?;
        }
    }
    Ok(())
}

type Range = (i128, i128);

struct Arithmetic<'a> {
    model: &'a Model,
    limits: RangeInclusive<i64>,
}

impl Arithmetic<'_> {
    fn require(&self, (min, max): Range, subject: &str) -> Result<(), Error> {
        if min < i128::from(*self.limits.start()) || max > i128::from(*self.limits.end()) {
            return Err(Error::kind(format!(
                "{subject} may exceed the supported integer range {}..={}",
                self.limits.start(),
                self.limits.end()
            )));
        }
        Ok(())
    }

    /// A projection from a record literal can read an intermediate value
    /// that is not materialized in a state and is therefore not scope-bound.
    fn projected<'a>(&self, expression: &'a Expr) -> &'a Expr {
        if let Expr::At(_, inner) = expression {
            return self.projected(inner);
        }
        if let Expr::Field(receiver, field) = expression
            && let Expr::Record(name, fields) = self.projected(receiver)
            && let Some(record) = self.model.record(name)
            && let Some(index) = record.field_index(field)
        {
            return self.projected(&fields[index]);
        }
        expression
    }

    fn cardinality_bound(&self, expression: &Expr) -> i128 {
        const MAX: i128 = i64::MAX as i128;
        match expression {
            Expr::None => 0,
            Expr::Name(_) => {
                // Updates can count over more atoms than Alloy's formula limit.
                // A binary relation has at most one value per atom or two atom columns.
                let atoms = (crate::syntax::atoms(self.model).max(20) as i128).min(MAX);
                (atoms * atoms).min(MAX)
            }
            Expr::At(_, expr) => self.cardinality_bound(expr),
            Expr::Field(receiver, _) | Expr::Keys(receiver) => self.cardinality_bound(receiver),
            Expr::If(_, yes, no) => self.cardinality_bound(yes).max(self.cardinality_bound(no)),
            Expr::Elements(seq) => self.length_bound(seq),
            Expr::ReachableFrom(..) | Expr::Select(..) => crate::syntax::atoms(self.model) as i128,
            Expr::Closure(_) => (crate::syntax::atoms(self.model) as i128).pow(2).min(MAX),
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
            | Expr::Length(_)
            | Expr::Index(..)
            | Expr::Variant(..)
            | Expr::Payload(..) => 1,
            Expr::Binary(op, left, right) => {
                let l = self.cardinality_bound(left);
                let r = self.cardinality_bound(right);
                match op {
                    SetOp::Union | SetOp::SetUnion | SetOp::Override => (l + r).min(MAX),
                    SetOp::Difference | SetOp::SetDifference | SetOp::WithoutKeys => l,
                    SetOp::Intersection => l.min(r),
                    SetOp::Product | SetOp::Join => (l * r).min(MAX),
                    SetOp::Multiply => unreachable!("rewritten by the type checker"),
                }
            }
        }
    }

    fn length_bound(&self, expr: &Expr) -> i128 {
        match self.projected(expr) {
            Expr::Sequence(_, items) => items.len() as i128,
            Expr::Append(seq, _) => 1 + self.length_bound(seq),
            Expr::Tail(seq) => (self.length_bound(seq) - 1).max(0),
            Expr::Take(seq, _) | Expr::RemoveAt(seq, _) | Expr::RemoveAll(seq, _) => {
                self.length_bound(seq)
            }
            Expr::If(_, yes, no) => self.length_bound(yes).max(self.length_bound(no)),
            _ => crate::structured::MAX_SEQUENCE_LENGTH as i128,
        }
    }

    fn bounds(&self, expression: &Expr) -> Result<Range, Error> {
        let bounds = match self.projected(expression) {
            Expr::Literal(value) => (i128::from(*value), i128::from(*value)),
            Expr::Count(inner) => (0, self.cardinality_bound(inner)),
            Expr::Length(seq) => (0, self.length_bound(seq)),
            Expr::If(_, yes, no) => {
                let (a, b) = self.bounds(yes)?;
                let (c, d) = self.bounds(no)?;
                (a.min(c), b.max(d))
            }
            Expr::Arith(op, left, right) => {
                let (a, b) = self.bounds(left)?;
                let (c, d) = self.bounds(right)?;
                match op {
                    Arith::Add => (a + c, b + d),
                    Arith::Sub => (a - d, b - c),
                    Arith::Mul => {
                        let values = [a * c, a * d, b * c, b * d];
                        (*values.iter().min().unwrap(), *values.iter().max().unwrap())
                    }
                }
            }
            _ => {
                let (lo, hi) = self.model.scope.ints.unwrap_or((i64::MIN, i64::MAX));
                (i128::from(lo), i128::from(hi))
            }
        };
        self.require(bounds, "integer arithmetic")?;
        Ok(bounds)
    }

    fn expression(&self, expression: &Expr) -> Result<(), Error> {
        match expression {
            Expr::Literal(value) => {
                self.require((i128::from(*value), i128::from(*value)), "integer literal")?;
            }
            Expr::If(condition, yes, no) => {
                self.formula(condition)?;
                self.expression(yes)?;
                self.expression(no)?;
            }
            Expr::Select(_, _, predicate) => self.formula(predicate)?,
            Expr::Arith(_, left, right) => {
                self.expression(left)?;
                self.expression(right)?;
                self.bounds(expression)?;
            }
            Expr::Binary(_, left, right)
            | Expr::Append(left, right)
            | Expr::Take(left, right)
            | Expr::RemoveAt(left, right)
            | Expr::RemoveAll(left, right)
            | Expr::ReachableFrom(left, right)
            | Expr::Index(left, right) => {
                self.expression(left)?;
                self.expression(right)?;
            }
            Expr::Count(inner) => {
                self.expression(inner)?;
                self.require((0, self.cardinality_bound(inner)), "integer count")?;
            }
            Expr::Record(_, fields) | Expr::Sequence(_, fields) => {
                for field in fields {
                    self.expression(field)?;
                }
            }
            Expr::Variant(_, payload) => {
                if let Some(expr) = payload {
                    self.expression(expr)?;
                }
            }
            Expr::Tail(expr)
            | Expr::Keys(expr)
            | Expr::Closure(expr)
            | Expr::Length(expr)
            | Expr::Elements(expr)
            | Expr::Field(expr, _)
            | Expr::Payload(expr, _)
            | Expr::At(_, expr) => self.expression(expr)?,
            Expr::Name(_) | Expr::None => {}
        }
        Ok(())
    }

    fn integer(&self, integer: &Int) -> Result<(), Error> {
        match integer {
            Int::Literal(value) => {
                self.require((i128::from(*value), i128::from(*value)), "integer literal")
            }
            Int::Count(expr) => {
                self.expression(expr)?;
                self.require((0, self.cardinality_bound(expr)), "integer count")
            }
            Int::Value(expr) | Int::LocatedValue(expr, _) => self.expression(expr),
        }
    }

    fn formula(&self, formula: &Formula) -> Result<(), Error> {
        match formula {
            Formula::Compare(_, left, right) => {
                self.expression(left)?;
                self.expression(right)
            }
            Formula::Ints(_, left, right) => {
                self.integer(left)?;
                self.integer(right)
            }
            Formula::Mult(_, expr) | Formula::Empty(expr) | Formula::HasTag(expr, _) => {
                self.expression(expr)
            }
            Formula::Not(body) | Formula::Quantified(_, _, _, body) => self.formula(body),
            Formula::And(parts) => {
                for formula in parts {
                    self.formula(formula)?;
                }
                Ok(())
            }
            Formula::Or(left, right) | Formula::Implies(left, right) => {
                self.formula(left)?;
                self.formula(right)
            }
        }
    }
}
