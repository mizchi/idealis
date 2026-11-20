use rtla::ast::{self, DefinitionKind, Expression, ExpressionKind as E};

pub(crate) fn children_mut(value: &mut Expression) -> Vec<&mut Expression> {
    match &mut value.node {
        E::Number { .. }
        | E::Integer { .. }
        | E::Decimal { .. }
        | E::String { .. }
        | E::Identifier { .. }
        | E::ExceptAt => vec![],
        E::Apply { args, .. } | E::Bracket { args } => args.iter_mut().collect(),
        E::Prime { expression }
        | E::Unchanged { expression }
        | E::Enabled { expression }
        | E::Always { expression }
        | E::Eventually { expression } => vec![expression],
        E::And { operands, .. } | E::Or { operands, .. } => operands.iter_mut().collect(),
        E::SetEnumeration { elements } | E::Tuple { elements } => elements.iter_mut().collect(),
        E::Product { factors } => factors.iter_mut().collect(),
        E::If {
            condition,
            consequent,
            alternative,
        } => vec![condition, consequent, alternative],
        E::Case { arms, other } => arms
            .iter_mut()
            .flat_map(|arm| [&mut arm.condition, &mut arm.body])
            .chain(other.iter_mut().map(Box::as_mut))
            .collect(),
        E::Lambda { body, .. } | E::Label { body, .. } | E::UnboundedQuantifier { body, .. } => {
            vec![body]
        }
        E::Let { definitions, body } => {
            let mut children = vec![body.as_mut()];
            for definition in definitions {
                children.push(&mut definition.body);
                if let DefinitionKind::Function { bounds } = &mut definition.head {
                    children.extend(bounds.iter_mut().map(|bound| &mut bound.set));
                }
            }
            children
        }
        E::LetModule {
            definitions,
            instances,
            body,
            ..
        } => {
            let mut children = vec![body.as_mut()];
            for definition in definitions {
                children.push(&mut definition.body);
                if let DefinitionKind::Function { bounds } = &mut definition.head {
                    children.extend(bounds.iter_mut().map(|bound| &mut bound.set));
                }
            }
            children.extend(instances.iter_mut().flat_map(|instance| {
                instance
                    .substitutions
                    .iter_mut()
                    .map(|entry| &mut entry.expression)
            }));
            children
        }
        E::Forall { bounds, body }
        | E::Exists { bounds, body }
        | E::FunctionConstruction { bounds, body }
        | E::SetMap {
            bounds,
            expression: body,
        } => bounds
            .iter_mut()
            .map(|bound| &mut bound.set)
            .chain(std::iter::once(body.as_mut()))
            .collect(),
        E::Choose { set, body, .. } => set
            .iter_mut()
            .map(Box::as_mut)
            .chain(std::iter::once(body.as_mut()))
            .collect(),
        E::SetFilter { bound, predicate } => vec![&mut bound.set, predicate],
        E::FunctionSet { domain, range } => vec![domain, range],
        E::Record { fields } | E::RecordSet { fields } => {
            fields.iter_mut().map(|field| &mut field.value).collect()
        }
        E::Except { function, updates } => {
            let mut children = vec![function.as_mut()];
            for update in updates {
                for path in &mut update.path {
                    if let ast::ExceptPath::Index { args } = path {
                        children.extend(args.iter_mut());
                    }
                }
                children.push(&mut update.value);
            }
            children
        }
        E::FunctionApplication { function, args } => std::iter::once(function.as_mut())
            .chain(args.iter_mut())
            .collect(),
        E::FieldAccess { record, .. } => vec![record],
        E::BoxAction { action, subscript }
        | E::AngleAction { action, subscript }
        | E::WeakFairness { action, subscript }
        | E::StrongFairness { action, subscript } => vec![action, subscript],
        E::Selection {
            expression,
            selectors,
        } => std::iter::once(expression.as_mut())
            .chain(selectors.iter_mut().flat_map(|selector| match selector {
                ast::Selector::Name { args, .. } | ast::Selector::Arguments { args } => {
                    args.as_mut_slice()
                }
                _ => &mut [],
            }))
            .collect(),
        E::AssumeProve {
            assumptions, prove, ..
        } => assumptions
            .iter_mut()
            .filter_map(|assumption| match assumption {
                ast::ProofAssumption::Expression { expression } => Some(expression),
                ast::ProofAssumption::New { set, .. } => set.as_deref_mut(),
            })
            .chain(std::iter::once(prove.as_mut()))
            .collect(),
    }
}
