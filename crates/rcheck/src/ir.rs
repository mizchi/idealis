//! The kernel IR as JSON, version 1 (plan 0108): the contract every
//! adapter writes. It is rmodel's syntax tree before type checking;
//! `from_json` reads it and `rmodel::validate` checks it, so an adapter
//! never decides typing. Expressions and formulas are trees tagged by
//! `op`; unknown tags and fields are rejected.
use rmodel::{
    Arith, Compare, Expr, Formula, Int, IntCompare, Model, Mult, Quantifier, SetOp, Shape,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ir {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub sigs: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entity_definitions: BTreeMap<String, String>,
    #[serde(default)]
    pub enums: Vec<Named>,
    #[serde(default)]
    pub sums: Vec<Named>,
    #[serde(default)]
    pub records: Vec<RecordIr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences: Vec<SequenceIr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<VariantIr>,
    #[serde(default)]
    pub relations: Vec<RelationIr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derivations: Vec<DerivationIr>,
    #[serde(default)]
    pub facts: Vec<F>,
    #[serde(default)]
    pub init: Vec<F>,
    #[serde(default)]
    pub actions: Vec<ActionIr>,
    #[serde(default)]
    pub checks: Vec<CheckIr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub temporal: Vec<TemporalIr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TransitionIr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fairness: Vec<FairnessIr>,
    pub scope: ScopeIr,
    /// Parts of the source the adapter could not write in the IR.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted: Vec<Omitted>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub sort: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationIr {
    pub name: String,
    pub value: E,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordIr {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceIr {
    pub name: String,
    pub element: String,
    pub max: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantIr {
    pub name: String,
    pub cases: Vec<CaseIr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseIr {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MultIr {
    Set,
    Lone,
    One,
    Some,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum ShapeIr {
    Unary {
        sort: String,
        mult: MultIr,
    },
    Binary {
        from: String,
        to: String,
        mult: MultIr,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationIr {
    pub name: String,
    pub var: bool,
    pub shape: ShapeIr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionIr {
    pub name: String,
    #[serde(default)]
    pub params: Vec<Field>,
    #[serde(default)]
    pub guards: Vec<F>,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub var: String,
    pub value: E,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckKind {
    Invariant,
    Reachable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckIr {
    pub name: String,
    pub kind: CheckKind,
    pub formula: F,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionIr {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub formula: F,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalIr {
    pub name: String,
    pub formula: F,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Field>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<F>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fairness: Option<Vec<FairnessIr>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum FairnessIr {
    Weak {
        action: String,
        #[serde(default, skip_serializing_if = "is_false")]
        each: bool,
    },
    Strong {
        action: String,
        #[serde(default, skip_serializing_if = "is_false")]
        each: bool,
    },
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeIr {
    #[serde(default)]
    pub sizes: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ints: Option<(i64, i64)>,
    pub steps: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Omitted {
    pub name: String,
    pub reason: String,
}

/// An expression: a set or relation, or one integer or record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum E {
    Name {
        name: String,
    },
    None,
    If {
        condition: Box<F>,
        then: Box<E>,
        otherwise: Box<E>,
    },
    Select {
        var: String,
        sort: String,
        predicate: Box<F>,
    },
    Union {
        left: Box<E>,
        right: Box<E>,
    },
    SetUnion {
        left: Box<E>,
        right: Box<E>,
    },
    Override {
        left: Box<E>,
        right: Box<E>,
    },
    WithoutKeys {
        relation: Box<E>,
        keys: Box<E>,
    },
    Difference {
        left: Box<E>,
        right: Box<E>,
    },
    SetDifference {
        left: Box<E>,
        right: Box<E>,
    },
    Intersection {
        left: Box<E>,
        right: Box<E>,
    },
    Product {
        left: Box<E>,
        right: Box<E>,
    },
    Join {
        left: Box<E>,
        right: Box<E>,
    },
    Literal {
        value: i64,
    },
    Count {
        of: Box<E>,
    },
    Add {
        left: Box<E>,
        right: Box<E>,
    },
    Sub {
        left: Box<E>,
        right: Box<E>,
    },
    Mul {
        left: Box<E>,
        right: Box<E>,
    },
    Sequence {
        name: String,
        items: Vec<E>,
    },
    Append {
        seq: Box<E>,
        item: Box<E>,
    },
    Tail {
        of: Box<E>,
    },
    Take {
        seq: Box<E>,
        count: Box<E>,
    },
    RemoveAt {
        seq: Box<E>,
        index: Box<E>,
    },
    Length {
        of: Box<E>,
    },
    Elements {
        of: Box<E>,
    },
    RemoveAll {
        seq: Box<E>,
        excluded: Box<E>,
    },
    Keys {
        of: Box<E>,
    },
    Closure {
        of: Box<E>,
    },
    ReachableFrom {
        roots: Box<E>,
        edges: Box<E>,
    },
    Index {
        seq: Box<E>,
        index: Box<E>,
    },
    Variant {
        tag: String,
        payload: Option<Box<E>>,
    },
    Before {
        of: Box<E>,
    },
    After {
        of: Box<E>,
    },
    Payload {
        of: Box<E>,
        tag: String,
    },
    Record {
        name: String,
        fields: Vec<E>,
    },
}

/// An integer in a comparison: `#e`, a literal, or one integer value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum I {
    Count { of: E },
    Literal { value: i64 },
    Value { of: E },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum F {
    In {
        left: E,
        right: E,
    },
    NotIn {
        left: E,
        right: E,
    },
    Eq {
        left: E,
        right: E,
    },
    Ne {
        left: E,
        right: E,
    },
    Lt {
        left: I,
        right: I,
    },
    Le {
        left: I,
        right: I,
    },
    Gt {
        left: I,
        right: I,
    },
    Ge {
        left: I,
        right: I,
    },
    IntEq {
        left: I,
        right: I,
    },
    IntNe {
        left: I,
        right: I,
    },
    HasTag {
        of: E,
        tag: String,
    },
    Some {
        of: E,
    },
    One {
        of: E,
    },
    Lone {
        of: E,
    },
    No {
        of: E,
    },
    Not {
        arg: Box<F>,
    },
    And {
        args: Vec<F>,
    },
    Or {
        left: Box<F>,
        right: Box<F>,
    },
    Implies {
        left: Box<F>,
        right: Box<F>,
    },
    All {
        var: String,
        sort: String,
        body: Box<F>,
    },
    Exists {
        var: String,
        sort: String,
        body: Box<F>,
    },
}

fn mult(m: Mult) -> MultIr {
    match m {
        Mult::Set => MultIr::Set,
        Mult::Lone => MultIr::Lone,
        Mult::One => MultIr::One,
        Mult::Some => MultIr::Some,
    }
}

fn mult_back(m: MultIr) -> Mult {
    match m {
        MultIr::Set => Mult::Set,
        MultIr::Lone => Mult::Lone,
        MultIr::One => Mult::One,
        MultIr::Some => Mult::Some,
    }
}

fn expr(e: &Expr) -> Result<E, String> {
    let pair =
        |l: &Expr, r: &Expr| -> Result<_, String> { Ok((Box::new(expr(l)?), Box::new(expr(r)?))) };
    Ok(match e {
        Expr::Name(name) => E::Name { name: name.clone() },
        Expr::Field(receiver, field) => E::Join {
            left: Box::new(expr(receiver)?),
            right: Box::new(E::Name {
                name: field.clone(),
            }),
        },
        Expr::None => E::None,
        Expr::If(condition, yes, no) => E::If {
            condition: Box::new(formula(condition)?),
            then: Box::new(expr(yes)?),
            otherwise: Box::new(expr(no)?),
        },
        Expr::Select(var, sort, predicate) => E::Select {
            var: var.clone(),
            sort: sort.clone(),
            predicate: Box::new(formula(predicate)?),
        },
        Expr::Binary(op, l, r) => {
            let (left, right) = pair(l, r)?;
            match op {
                SetOp::Union => E::Union { left, right },
                SetOp::SetUnion => E::SetUnion { left, right },
                SetOp::Override => E::Override { left, right },
                SetOp::WithoutKeys => E::WithoutKeys {
                    relation: left,
                    keys: right,
                },
                SetOp::Difference => E::Difference { left, right },
                SetOp::SetDifference => E::SetDifference { left, right },
                SetOp::Intersection => E::Intersection { left, right },
                SetOp::Product => E::Product { left, right },
                SetOp::Join => E::Join { left, right },
                SetOp::Multiply => E::Mul { left, right },
            }
        }
        Expr::Literal(value) => E::Literal { value: *value },
        Expr::Count(of) => E::Count {
            of: Box::new(expr(of)?),
        },
        Expr::Arith(op, l, r) => {
            let (left, right) = pair(l, r)?;
            match op {
                Arith::Add => E::Add { left, right },
                Arith::Sub => E::Sub { left, right },
                Arith::Mul => E::Mul { left, right },
            }
        }
        Expr::Sequence(name, items) => E::Sequence {
            name: name.clone(),
            items: items.iter().map(expr).collect::<Result<_, _>>()?,
        },
        Expr::Append(seq, item) => E::Append {
            seq: Box::new(expr(seq)?),
            item: Box::new(expr(item)?),
        },
        Expr::Tail(seq) => E::Tail {
            of: Box::new(expr(seq)?),
        },
        Expr::Take(seq, count) => E::Take {
            seq: Box::new(expr(seq)?),
            count: Box::new(expr(count)?),
        },
        Expr::RemoveAt(seq, index) => E::RemoveAt {
            seq: Box::new(expr(seq)?),
            index: Box::new(expr(index)?),
        },
        Expr::Length(seq) => E::Length {
            of: Box::new(expr(seq)?),
        },
        Expr::RemoveAll(seq, excluded) => E::RemoveAll {
            seq: Box::new(expr(seq)?),
            excluded: Box::new(expr(excluded)?),
        },
        Expr::Closure(edges) => E::Closure {
            of: Box::new(expr(edges)?),
        },
        Expr::Keys(relation) => E::Keys {
            of: Box::new(expr(relation)?),
        },
        Expr::ReachableFrom(roots, edges) => E::ReachableFrom {
            roots: Box::new(expr(roots)?),
            edges: Box::new(expr(edges)?),
        },
        Expr::Elements(seq) => E::Elements {
            of: Box::new(expr(seq)?),
        },
        Expr::Index(seq, index) => E::Index {
            seq: Box::new(expr(seq)?),
            index: Box::new(expr(index)?),
        },
        Expr::Variant(tag, payload) => E::Variant {
            tag: tag.clone(),
            payload: payload.as_deref().map(expr).transpose()?.map(Box::new),
        },
        Expr::Payload(of, tag) => E::Payload {
            of: Box::new(expr(of)?),
            tag: tag.clone(),
        },
        Expr::At(phase, value) => {
            let of = Box::new(expr(value)?);
            match phase {
                rmodel::Phase::Before => E::Before { of },
                rmodel::Phase::After => E::After { of },
            }
        }
        Expr::Record(name, fields) => E::Record {
            name: name.clone(),
            fields: fields.iter().map(expr).collect::<Result<_, _>>()?,
        },
    })
}

fn expr_back(e: &E) -> Expr {
    let pair = |l: &E, r: &E| (Box::new(expr_back(l)), Box::new(expr_back(r)));
    match e {
        E::Name { name } => Expr::Name(name.clone()),
        E::None => Expr::None,
        E::If {
            condition,
            then,
            otherwise,
        } => Expr::If(
            Box::new(formula_back(condition)),
            Box::new(expr_back(then)),
            Box::new(expr_back(otherwise)),
        ),
        E::Select {
            var,
            sort,
            predicate,
        } => Expr::Select(var.clone(), sort.clone(), Box::new(formula_back(predicate))),
        E::Union { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::Union, l, r)
        }
        E::SetUnion { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::SetUnion, l, r)
        }
        E::Override { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::Override, l, r)
        }
        E::WithoutKeys { relation, keys } => {
            let (relation, keys) = pair(relation, keys);
            Expr::Binary(SetOp::WithoutKeys, relation, keys)
        }
        E::Difference { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::Difference, l, r)
        }
        E::SetDifference { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::SetDifference, l, r)
        }
        E::Intersection { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::Intersection, l, r)
        }
        E::Product { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::Product, l, r)
        }
        E::Join { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Binary(SetOp::Join, l, r)
        }
        E::Literal { value } => Expr::Literal(*value),
        E::Count { of } => Expr::Count(Box::new(expr_back(of))),
        // Arithmetic stays explicit; the checker types it.
        E::Add { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Arith(Arith::Add, l, r)
        }
        E::Sub { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Arith(Arith::Sub, l, r)
        }
        E::Mul { left, right } => {
            let (l, r) = pair(left, right);
            Expr::Arith(Arith::Mul, l, r)
        }
        E::Sequence { name, items } => {
            Expr::Sequence(name.clone(), items.iter().map(expr_back).collect())
        }
        E::Append { seq, item } => {
            Expr::Append(Box::new(expr_back(seq)), Box::new(expr_back(item)))
        }
        E::Tail { of } => Expr::Tail(Box::new(expr_back(of))),
        E::Take { seq, count } => Expr::Take(Box::new(expr_back(seq)), Box::new(expr_back(count))),
        E::RemoveAt { seq, index } => {
            Expr::RemoveAt(Box::new(expr_back(seq)), Box::new(expr_back(index)))
        }
        E::Length { of } => Expr::Length(Box::new(expr_back(of))),
        E::RemoveAll { seq, excluded } => {
            Expr::RemoveAll(Box::new(expr_back(seq)), Box::new(expr_back(excluded)))
        }
        E::Closure { of } => Expr::Closure(Box::new(expr_back(of))),
        E::Keys { of } => Expr::Keys(Box::new(expr_back(of))),
        E::ReachableFrom { roots, edges } => {
            Expr::ReachableFrom(Box::new(expr_back(roots)), Box::new(expr_back(edges)))
        }
        E::Elements { of } => Expr::Elements(Box::new(expr_back(of))),
        E::Index { seq, index } => {
            Expr::Index(Box::new(expr_back(seq)), Box::new(expr_back(index)))
        }
        E::Variant { tag, payload } => Expr::Variant(
            tag.clone(),
            payload.as_ref().map(|v| Box::new(expr_back(v))),
        ),
        E::Payload { of, tag } => Expr::Payload(Box::new(expr_back(of)), tag.clone()),
        E::Before { of } => Expr::At(rmodel::Phase::Before, Box::new(expr_back(of))),
        E::After { of } => Expr::At(rmodel::Phase::After, Box::new(expr_back(of))),
        E::Record { name, fields } => {
            Expr::Record(name.clone(), fields.iter().map(expr_back).collect())
        }
    }
}

fn int(i: &Int) -> Result<I, String> {
    Ok(match i {
        Int::Count(e) => I::Count { of: expr(e)? },
        Int::Literal(value) => I::Literal { value: *value },
        Int::Value(e) | Int::LocatedValue(e, _) => I::Value { of: expr(e)? },
    })
}

fn int_back(i: &I) -> Int {
    match i {
        I::Count { of } => Int::Count(expr_back(of)),
        I::Literal { value } => Int::Literal(*value),
        I::Value { of } => Int::Value(expr_back(of)),
    }
}

fn formula(f: &Formula) -> Result<F, String> {
    Ok(match f {
        Formula::Compare(op, l, r) => {
            let (left, right) = (expr(l)?, expr(r)?);
            match op {
                Compare::In => F::In { left, right },
                Compare::NotIn => F::NotIn { left, right },
                Compare::Equal => F::Eq { left, right },
                Compare::NotEqual => F::Ne { left, right },
            }
        }
        Formula::Ints(op, l, r) => {
            let (left, right) = (int(l)?, int(r)?);
            match op {
                IntCompare::Less => F::Lt { left, right },
                IntCompare::LessEqual => F::Le { left, right },
                IntCompare::Greater => F::Gt { left, right },
                IntCompare::GreaterEqual => F::Ge { left, right },
                IntCompare::Equal => F::IntEq { left, right },
                IntCompare::NotEqual => F::IntNe { left, right },
            }
        }
        Formula::Mult(m, e) => {
            let of = expr(e)?;
            match m {
                Mult::Some => F::Some { of },
                Mult::One => F::One { of },
                Mult::Lone => F::Lone { of },
                Mult::Set => return Err("a `set` multiplicity formula".into()),
            }
        }
        Formula::HasTag(of, tag) => F::HasTag {
            of: expr(of)?,
            tag: tag.clone(),
        },
        Formula::Empty(e) => F::No { of: expr(e)? },
        Formula::Not(inner) => F::Not {
            arg: Box::new(formula(inner)?),
        },
        Formula::And(parts) => F::And {
            args: parts.iter().map(formula).collect::<Result<_, _>>()?,
        },
        Formula::Or(l, r) => F::Or {
            left: Box::new(formula(l)?),
            right: Box::new(formula(r)?),
        },
        Formula::Implies(l, r) => F::Implies {
            left: Box::new(formula(l)?),
            right: Box::new(formula(r)?),
        },
        Formula::Quantified(q, var, sort, body) => {
            let (var, sort, body) = (var.clone(), sort.clone(), Box::new(formula(body)?));
            match q {
                Quantifier::All => F::All { var, sort, body },
                Quantifier::Some => F::Exists { var, sort, body },
            }
        }
    })
}

fn formula_back(f: &F) -> Formula {
    let cmp = |op, l: &E, r: &E| Formula::Compare(op, expr_back(l), expr_back(r));
    let ints = |op, l: &I, r: &I| Formula::Ints(op, int_back(l), int_back(r));
    match f {
        F::In { left, right } => cmp(Compare::In, left, right),
        F::HasTag { of, tag } => Formula::HasTag(expr_back(of), tag.clone()),
        F::NotIn { left, right } => cmp(Compare::NotIn, left, right),
        F::Eq { left, right } => cmp(Compare::Equal, left, right),
        F::Ne { left, right } => cmp(Compare::NotEqual, left, right),
        F::Lt { left, right } => ints(IntCompare::Less, left, right),
        F::Le { left, right } => ints(IntCompare::LessEqual, left, right),
        F::Gt { left, right } => ints(IntCompare::Greater, left, right),
        F::Ge { left, right } => ints(IntCompare::GreaterEqual, left, right),
        F::IntEq { left, right } => ints(IntCompare::Equal, left, right),
        F::IntNe { left, right } => ints(IntCompare::NotEqual, left, right),
        F::Some { of } => Formula::Mult(Mult::Some, expr_back(of)),
        F::One { of } => Formula::Mult(Mult::One, expr_back(of)),
        F::Lone { of } => Formula::Mult(Mult::Lone, expr_back(of)),
        F::No { of } => Formula::Empty(expr_back(of)),
        F::Not { arg } => Formula::Not(Box::new(formula_back(arg))),
        F::And { args } => Formula::And(args.iter().map(formula_back).collect()),
        F::Or { left, right } => {
            Formula::Or(Box::new(formula_back(left)), Box::new(formula_back(right)))
        }
        F::Implies { left, right } => {
            Formula::Implies(Box::new(formula_back(left)), Box::new(formula_back(right)))
        }
        F::All { var, sort, body } => Formula::Quantified(
            Quantifier::All,
            var.clone(),
            sort.clone(),
            Box::new(formula_back(body)),
        ),
        F::Exists { var, sort, body } => Formula::Quantified(
            Quantifier::Some,
            var.clone(),
            sort.clone(),
            Box::new(formula_back(body)),
        ),
    }
}

/// The IR of a checked model. Bool, which the checker adds, is left out.
fn fairness(rule: &rmodel::Fairness) -> FairnessIr {
    match rule.kind {
        rmodel::FairnessKind::Weak => FairnessIr::Weak {
            action: rule.action.clone(),
            each: rule.each,
        },
        rmodel::FairnessKind::Strong => FairnessIr::Strong {
            action: rule.action.clone(),
            each: rule.each,
        },
    }
}

fn fairness_back(rule: &FairnessIr) -> rmodel::Fairness {
    let (kind, action, each) = match rule {
        FairnessIr::Weak { action, each } => (rmodel::FairnessKind::Weak, action, each),
        FairnessIr::Strong { action, each } => (rmodel::FairnessKind::Strong, action, each),
    };
    rmodel::Fairness {
        kind,
        action: action.clone(),
        each: *each,
    }
}

pub fn to_ir(model: &Model) -> Result<Ir, String> {
    let named = |items: &[(String, Vec<String>)]| -> Vec<Named> {
        items
            .iter()
            .filter(|(name, _)| name != rmodel::BOOL)
            .map(|(name, members)| Named {
                name: name.clone(),
                members: members.clone(),
            })
            .collect()
    };
    Ok(Ir {
        version: VERSION,
        name: model.name.clone(),
        sigs: model.sigs.clone(),
        entity_definitions: model.entity_definitions.clone(),
        enums: named(&model.enums),
        sums: named(&model.sums),
        records: model
            .records
            .iter()
            .map(|r| RecordIr {
                name: r.name.clone(),
                fields: r
                    .fields
                    .iter()
                    .map(|(name, sort)| Field {
                        name: name.clone(),
                        sort: sort.clone(),
                    })
                    .collect(),
            })
            .collect(),
        sequences: model
            .sequences
            .iter()
            .map(|s| SequenceIr {
                name: s.name.clone(),
                element: s.element.clone(),
                max: s.max,
            })
            .collect(),
        variants: model
            .variants
            .iter()
            .map(|v| VariantIr {
                name: v.name.clone(),
                cases: v
                    .cases
                    .iter()
                    .map(|c| CaseIr {
                        tag: c.tag.clone(),
                        payload: c.payload.clone(),
                    })
                    .collect(),
            })
            .collect(),
        relations: model
            .relations
            .iter()
            .map(|r| RelationIr {
                name: r.name.clone(),
                var: r.var,
                shape: match &r.shape {
                    Shape::Unary { sig, mult: m } => ShapeIr::Unary {
                        sort: sig.clone(),
                        mult: mult(*m),
                    },
                    Shape::Binary { from, to, mult: m } => ShapeIr::Binary {
                        from: from.clone(),
                        to: to.clone(),
                        mult: mult(*m),
                    },
                },
            })
            .collect(),
        derivations: model
            .derivations
            .iter()
            .map(|(name, value)| {
                Ok(DerivationIr {
                    name: name.clone(),
                    value: expr(value)?,
                })
            })
            .collect::<Result<_, String>>()?,
        facts: model.facts.iter().map(formula).collect::<Result<_, _>>()?,
        init: model.init.iter().map(formula).collect::<Result<_, _>>()?,
        actions: model
            .actions
            .iter()
            .map(|a| {
                Ok(ActionIr {
                    name: a.name.clone(),
                    params: a
                        .params
                        .iter()
                        .map(|(name, sort)| Field {
                            name: name.clone(),
                            sort: sort.clone(),
                        })
                        .collect(),
                    guards: a
                        .guards
                        .iter()
                        .map(formula)
                        .collect::<Result<_, String>>()?,
                    assignments: a
                        .assignments
                        .iter()
                        .map(|(var, value)| {
                            Ok(Assignment {
                                var: var.clone(),
                                value: expr(value)?,
                            })
                        })
                        .collect::<Result<_, String>>()?,
                })
            })
            .collect::<Result<_, String>>()?,
        checks: model
            .checks
            .iter()
            .map(|c| {
                Ok(CheckIr {
                    name: c.name.clone(),
                    kind: match c.kind {
                        rmodel::CheckKind::Invariant => CheckKind::Invariant,
                        rmodel::CheckKind::Reachable => CheckKind::Reachable,
                    },
                    formula: formula(&c.formula)?,
                })
            })
            .collect::<Result<_, String>>()?,
        temporal: model
            .temporal
            .iter()
            .map(|check| {
                Ok(TemporalIr {
                    name: check.name.clone(),
                    formula: formula(&check.formula)?,
                    params: check
                        .params
                        .iter()
                        .map(|(name, sort)| Field {
                            name: name.clone(),
                            sort: sort.clone(),
                        })
                        .collect(),
                    trigger: check.trigger.as_ref().map(formula).transpose()?,
                    fairness: check
                        .fairness
                        .as_ref()
                        .map(|rules| rules.iter().map(fairness).collect()),
                })
            })
            .collect::<Result<_, String>>()?,
        transitions: model
            .transitions
            .iter()
            .map(|check| {
                Ok(TransitionIr {
                    name: check.name.clone(),
                    action: check.action.clone(),
                    formula: formula(&check.formula)?,
                })
            })
            .collect::<Result<_, String>>()?,
        fairness: model.fairness.iter().map(fairness).collect(),
        scope: ScopeIr {
            sizes: model.scope.sizes.clone(),
            ints: model.scope.ints,
            steps: model.scope.steps,
        },
        omitted: Vec::new(),
    })
}

fn check_version(version: u32) -> Result<(), String> {
    if version != VERSION {
        return Err(format!("IR version {version} (want {VERSION})"));
    }
    Ok(())
}

/// A checked model from the IR, through rmodel's type checker.
pub fn from_ir(ir: &Ir) -> Result<Model, String> {
    check_version(ir.version)?;
    let model = Model {
        name: ir.name.clone(),
        sigs: ir.sigs.clone(),
        entity_definitions: ir.entity_definitions.clone(),
        enums: ir
            .enums
            .iter()
            .map(|n| (n.name.clone(), n.members.clone()))
            .collect(),
        sums: ir
            .sums
            .iter()
            .map(|n| (n.name.clone(), n.members.clone()))
            .collect(),
        records: ir
            .records
            .iter()
            .map(|r| rmodel::Record {
                name: r.name.clone(),
                fields: r
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), f.sort.clone()))
                    .collect(),
            })
            .collect(),
        sequences: ir
            .sequences
            .iter()
            .map(|s| rmodel::Sequence {
                name: s.name.clone(),
                element: s.element.clone(),
                max: s.max,
            })
            .collect(),
        variants: ir
            .variants
            .iter()
            .map(|v| rmodel::Variant {
                name: v.name.clone(),
                cases: v
                    .cases
                    .iter()
                    .map(|c| rmodel::Case {
                        tag: c.tag.clone(),
                        payload: c.payload.clone(),
                    })
                    .collect(),
            })
            .collect(),
        relations: ir
            .relations
            .iter()
            .map(|r| rmodel::Relation {
                name: r.name.clone(),
                var: r.var,
                shape: match &r.shape {
                    ShapeIr::Unary { sort, mult: m } => Shape::Unary {
                        sig: sort.clone(),
                        mult: mult_back(*m),
                    },
                    ShapeIr::Binary { from, to, mult: m } => Shape::Binary {
                        from: from.clone(),
                        to: to.clone(),
                        mult: mult_back(*m),
                    },
                },
            })
            .collect(),
        derivations: ir
            .derivations
            .iter()
            .map(|d| (d.name.clone(), expr_back(&d.value)))
            .collect(),
        facts: ir.facts.iter().map(formula_back).collect(),
        init: ir.init.iter().map(formula_back).collect(),
        actions: ir
            .actions
            .iter()
            .map(|a| rmodel::Action {
                name: a.name.clone(),
                params: a
                    .params
                    .iter()
                    .map(|f| (f.name.clone(), f.sort.clone()))
                    .collect(),
                guards: a.guards.iter().map(formula_back).collect(),
                assignments: a
                    .assignments
                    .iter()
                    .map(|x| (x.var.clone(), expr_back(&x.value)))
                    .collect(),
            })
            .collect(),
        checks: ir
            .checks
            .iter()
            .map(|c| rmodel::Check {
                name: c.name.clone(),
                kind: match c.kind {
                    CheckKind::Invariant => rmodel::CheckKind::Invariant,
                    CheckKind::Reachable => rmodel::CheckKind::Reachable,
                },
                formula: formula_back(&c.formula),
            })
            .collect(),
        temporal: ir
            .temporal
            .iter()
            .map(|check| rmodel::TemporalCheck {
                name: check.name.clone(),
                formula: formula_back(&check.formula),
                params: check
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.sort.clone()))
                    .collect(),
                trigger: check.trigger.as_ref().map(formula_back),
                fairness: check
                    .fairness
                    .as_ref()
                    .map(|rules| rules.iter().map(fairness_back).collect()),
            })
            .collect(),
        transitions: ir
            .transitions
            .iter()
            .map(|check| rmodel::TransitionCheck {
                name: check.name.clone(),
                action: check.action.clone(),
                formula: formula_back(&check.formula),
            })
            .collect(),
        fairness: ir.fairness.iter().map(fairness_back).collect(),
        scope: rmodel::Scope {
            sizes: ir.scope.sizes.clone(),
            ints: ir.scope.ints,
            steps: ir.scope.steps,
        },
    };
    rmodel::validate(model).map_err(|e| e.to_string())
}

/// Reads IR JSON, rejecting unknown tags and fields, and checks the model.
pub fn parse(json: &str) -> Result<(Model, Vec<Omitted>), String> {
    #[derive(Deserialize)]
    struct Header {
        version: u32,
    }
    let header: Header = serde_json::from_str(json).map_err(|error| format!("IR: {error}"))?;
    check_version(header.version)?;
    let ir: Ir = serde_json::from_str(json).map_err(|e| format!("IR: {e}"))?;
    Ok((from_ir(&ir)?, ir.omitted))
}
