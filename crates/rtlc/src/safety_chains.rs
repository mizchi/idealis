use crate::ir::Expression;

const BUFFERED_SPINE: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Junction {
    And,
    Or,
}

impl Junction {
    fn operands(self, expression: &Expression) -> Option<(&Expression, &Expression)> {
        match (self, expression) {
            (Self::And, Expression::And { left, right })
            | (Self::Or, Expression::Or { left, right }) => Some((left, right)),
            _ => None,
        }
    }

    pub(crate) fn head(expression: &Expression) -> Option<Self> {
        match expression {
            Expression::And { left, .. } if matches!(**left, Expression::And { .. }) => {
                Some(Self::And)
            }
            Expression::Or { left, .. } if matches!(**left, Expression::Or { .. }) => {
                Some(Self::Or)
            }
            _ => None,
        }
    }

    pub(crate) fn visit_terms<'a, E>(
        self,
        head: &'a Expression,
        mut visit: impl FnMut(&'a Expression, usize) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let mut buffered = [head; BUFFERED_SPINE];
        let mut spilled = Vec::new();
        let mut length = 0;
        let mut node = head;
        while let Some((left, right)) = self.operands(node) {
            if length < BUFFERED_SPINE {
                buffered[length] = right;
            } else {
                spilled.push(right);
            }
            length += 1;
            node = left;
        }
        if !visit(node, length)? {
            return Ok(false);
        }
        for index in (0..length).rev() {
            let right = if index < BUFFERED_SPINE {
                buffered[index]
            } else {
                spilled[index - BUFFERED_SPINE]
            };
            if !visit(right, index + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

pub(crate) fn is_if_chain(expression: &Expression) -> bool {
    matches!(expression, Expression::If { alternative, .. } if matches!(**alternative, Expression::If { .. }))
}

pub(crate) fn visit_if_chain<'a, E>(
    head: &'a Expression,
    mut visit: impl FnMut(&'a Expression, usize) -> Result<(), E>,
) -> Result<(), E> {
    let mut node = head;
    let mut offset = 0;
    while let Expression::If {
        condition,
        consequent,
        alternative,
    } = node
    {
        offset += 1;
        visit(condition, offset)?;
        visit(consequent, offset)?;
        node = alternative;
    }
    visit(node, offset)
}

pub(crate) fn chain_terms(expression: &Expression) -> Option<Vec<(&Expression, usize)>> {
    let mut terms = Vec::new();
    if let Some(junction) = Junction::head(expression) {
        junction
            .visit_terms(expression, |term, offset| {
                terms.push((term, offset));
                Ok::<bool, ()>(true)
            })
            .ok()?;
        return Some(terms);
    }
    if is_if_chain(expression) {
        visit_if_chain(expression, |term, offset| {
            terms.push((term, offset));
            Ok::<(), ()>(())
        })
        .ok()?;
        return Some(terms);
    }
    None
}
