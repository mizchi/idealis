use crate::{Error, INT, IntCompare, Model, NAT, Record};

/// A numeric operand in a data type's value-local predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefinementOperand {
    Field(Vec<String>),
    Literal(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refinement {
    pub left: RefinementOperand,
    pub op: IntCompare,
    pub right: RefinementOperand,
}

impl Refinement {
    pub fn operator(token: &str) -> Option<IntCompare> {
        Some(match token {
            "<" => IntCompare::Less,
            "<=" => IntCompare::LessEqual,
            ">" => IntCompare::Greater,
            ">=" => IntCompare::GreaterEqual,
            "=" => IntCompare::Equal,
            "!=" => IntCompare::NotEqual,
            _ => return None,
        })
    }

    pub fn symbol(&self) -> &'static str {
        match self.op {
            IntCompare::Less => "<",
            IntCompare::LessEqual => "<=",
            IntCompare::Greater => ">",
            IntCompare::GreaterEqual => ">=",
            IntCompare::Equal => "=",
            IntCompare::NotEqual => "!=",
        }
    }

    pub fn holds(&self, field: impl Fn(&[String]) -> i64) -> bool {
        let value = |operand: &RefinementOperand| match operand {
            RefinementOperand::Field(path) => field(path),
            RefinementOperand::Literal(n) => *n,
        };
        let (left, right) = (value(&self.left), value(&self.right));
        match self.op {
            IntCompare::Less => left < right,
            IntCompare::LessEqual => left <= right,
            IntCompare::Greater => left > right,
            IntCompare::GreaterEqual => left >= right,
            IntCompare::Equal => left == right,
            IntCompare::NotEqual => left != right,
        }
    }
}

pub(crate) fn validate(model: &Model, record: &Record) -> Result<(), Error> {
    if !record.refinements.is_empty() && record.data.is_none() {
        return Err(Error::kind(format!(
            "{}: where requires a data type",
            record.name
        )));
    }
    for clause in &record.refinements {
        if matches!(clause.left, RefinementOperand::Literal(_))
            && matches!(clause.right, RefinementOperand::Literal(_))
        {
            return Err(Error::kind(format!(
                "{}: where must compare a field",
                record.name
            )));
        }
        for operand in [&clause.left, &clause.right] {
            match operand {
                RefinementOperand::Literal(n) if n.unsigned_abs() > 9_007_199_254_740_991 => {
                    return Err(Error::kind(format!(
                        "{}: where literal exceeds the safe integer range",
                        record.name
                    )));
                }
                RefinementOperand::Literal(_) => {}
                RefinementOperand::Field(path) => {
                    let mut current = Some(record);
                    let mut sort = "";
                    for name in path {
                        let field = current.and_then(|record| {
                            record.fields.iter().find(|(field, _)| field == name)
                        });
                        let Some((_, field_sort)) = field else {
                            return Err(Error::kind(format!(
                                "{}: unknown where field {}",
                                record.name,
                                path.join(".")
                            )));
                        };
                        sort = field_sort;
                        current = model.record(sort);
                    }
                    if !matches!(sort, INT | NAT) {
                        return Err(Error::kind(format!(
                            "{}: where field {} must be Int or Nat",
                            record.name,
                            path.join(".")
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}
