use super::{CallFrame, EvalContext, Values};
use crate::CoreError;
use crate::ir::{Expression, Scalar};
use crate::safety_values::Arena;
use std::sync::Arc;

struct Snapshot {
    current: Vec<Scalar>,
    next: Vec<Option<Scalar>>,
    locals: Vec<Scalar>,
    old: Option<Scalar>,
    prime: bool,
    frame: Option<Arc<CallFrame>>,
    evaluation_depth: u8,
    recursive: bool,
}

impl Snapshot {
    fn new(context: EvalContext<'_>) -> Self {
        Self {
            current: context.current.to_vec(),
            next: context.next.to_vec(),
            locals: context.locals.to_vec(),
            old: context.old,
            prime: context.prime,
            frame: context.frame.cloned().map(Arc::new),
            evaluation_depth: context.evaluation_depth,
            recursive: context.recursive,
        }
    }

    fn context(&self) -> EvalContext<'_> {
        EvalContext {
            current: &self.current,
            next: &self.next,
            locals: &self.locals,
            old: self.old,
            prime: self.prime,
            frame: self.frame.as_deref(),
            evaluation_depth: self.evaluation_depth,
            recursive: self.recursive,
        }
    }
}

pub(super) enum FunctionValue {
    Scalar(Scalar),
    Lazy(Box<LazyFunction>),
    Composite(Vec<(Scalar, FunctionValue)>),
}

pub(super) struct LazyFunction {
    domains: Vec<(Values, Option<usize>)>,
    body: Arc<Expression>,
    snapshot: Snapshot,
}

impl Expression {
    pub(super) fn eval_function(
        &self,
        mut context: EvalContext<'_>,
        arena: &mut Arena,
    ) -> Result<FunctionValue, CoreError> {
        match self {
            Self::At { target, key } => {
                context.descend(arena)?;
                let target = target.eval_function(context, arena)?;
                let key = key.eval(context, arena)?;
                target.select(key, arena)
            }
            Self::Record { fields } => {
                context.descend(arena)?;
                let mut values = vec![];
                for (name, value) in fields {
                    arena.tick()?;
                    let name = arena.string(name.clone())?;
                    values.push((name, value.eval_function(context, arena)?));
                }
                Ok(FunctionValue::Composite(values))
            }
            Self::Tuple { values } => {
                context.descend(arena)?;
                let mut result = vec![];
                for (index, value) in values.iter().enumerate() {
                    arena.tick()?;
                    let key = Scalar::Int(
                        i32::try_from(index + 1)
                            .map_err(|_| CoreError::protocol("Sequence length overflow"))?,
                    );
                    result.push((key, value.eval_function(context, arena)?));
                }
                Ok(FunctionValue::Composite(result))
            }
            Self::Frame { .. }
            | Self::Call { .. }
            | Self::Parameter { .. }
            | Self::Capture { .. } => {
                context.descend(arena)?;
                self.eval_reference(context, arena, Self::eval_function)
            }
            Self::Prime { argument } => {
                context.descend(arena)?;
                argument.eval_function(
                    EvalContext {
                        prime: true,
                        ..context
                    },
                    arena,
                )
            }
            Self::If {
                condition,
                consequent,
                alternative,
            } => {
                context.descend(arena)?;
                if condition.eval(context, arena)?.boolean()? {
                    consequent
                } else {
                    alternative
                }
                .eval_function(context, arena)
            }
            Self::Function { bounds, body } => {
                context.descend(arena)?;
                let domains = bounds
                    .iter()
                    .map(|binding| Ok((binding.domain.values(context, arena)?, binding.tuple_size)))
                    .collect::<Result<_, CoreError>>()?;
                Ok(FunctionValue::Lazy(Box::new(LazyFunction {
                    domains,
                    body: Arc::new(body.as_ref().clone()),
                    snapshot: Snapshot::new(context),
                })))
            }
            _ => Ok(FunctionValue::Scalar(self.eval(context, arena)?)),
        }
    }
}

impl FunctionValue {
    pub(super) fn at(self, key: Scalar, arena: &mut Arena) -> Result<Scalar, CoreError> {
        self.select(key, arena)?.materialize(arena)
    }

    fn select(self, key: Scalar, arena: &mut Arena) -> Result<Self, CoreError> {
        match self {
            Self::Scalar(value) => Ok(Self::Scalar(arena.at(value, key)?)),
            Self::Composite(mut values) => {
                let index = values
                    .iter()
                    .position(|(candidate, _)| *candidate == key)
                    .ok_or_else(|| {
                        CoreError::protocol("Function argument is outside its domain")
                    })?;
                Ok(values.swap_remove(index).1)
            }
            Self::Lazy(function) => {
                let keys = if function.domains.len() == 1 {
                    vec![key]
                } else {
                    arena.sequence(key)?
                };
                if keys.len() != function.domains.len() {
                    return Err(CoreError::protocol(
                        "Function argument is outside its domain",
                    ));
                }
                let mut context = function.snapshot.context();
                let mut locals = context.locals.to_vec();
                for ((values, tuple_size), key) in function.domains.iter().zip(keys) {
                    let found = match values {
                        Values::Range(range) => {
                            if arena.model_membership(key)? {
                                false
                            } else {
                                range.contains(&key.integer()?)
                            }
                        }
                        Values::Items(values) => {
                            let mut found = false;
                            for candidate in values.clone() {
                                if arena.equal(candidate, key)? {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        }
                    };
                    if !found {
                        return Err(CoreError::protocol(
                            "Function argument is outside its domain",
                        ));
                    }
                    context.descend(arena)?;
                    arena.tick()?;
                    arena.check_tuple(key, *tuple_size)?;
                    locals.push(key);
                }
                context.descend(arena)?;
                function.body.eval_function(
                    EvalContext {
                        locals: &locals,
                        ..context
                    },
                    arena,
                )
            }
        }
    }

    fn materialize(self, arena: &mut Arena) -> Result<Scalar, CoreError> {
        fn visit(
            function: &LazyFunction,
            position: usize,
            mut context: EvalContext<'_>,
            entries: &mut Vec<(Scalar, Scalar)>,
            arena: &mut Arena,
        ) -> Result<(), CoreError> {
            context.descend(arena)?;
            if let Some((values, tuple_size)) = function.domains.get(position) {
                let mut locals = context.locals.to_vec();
                for value in values.clone() {
                    arena.tick()?;
                    arena.check_tuple(value, *tuple_size)?;
                    locals.push(value);
                    visit(
                        function,
                        position + 1,
                        EvalContext {
                            locals: &locals,
                            ..context
                        },
                        entries,
                        arena,
                    )?;
                    locals.pop();
                }
            } else {
                let keys = &context.locals[function.snapshot.locals.len()..];
                let key = if keys.len() == 1 {
                    keys[0]
                } else {
                    arena.tuple(keys.to_vec())?
                };
                entries.push((key, function.body.eval(context, arena)?));
            }
            Ok(())
        }
        match self {
            Self::Scalar(value) => Ok(value),
            Self::Lazy(function) => {
                let mut entries = vec![];
                visit(
                    &function,
                    0,
                    function.snapshot.context(),
                    &mut entries,
                    arena,
                )?;
                arena.function(entries)
            }
            Self::Composite(values) => {
                let values = values
                    .into_iter()
                    .map(|(key, value)| Ok((key, value.materialize(arena)?)))
                    .collect::<Result<_, CoreError>>()?;
                arena.function(values)
            }
        }
    }

    pub(super) fn domain(self, arena: &mut Arena) -> Result<Scalar, CoreError> {
        fn visit(
            domains: &[(Values, Option<usize>)],
            position: usize,
            keys: &mut Vec<Scalar>,
            result: &mut Vec<Scalar>,
            arena: &mut Arena,
        ) -> Result<(), CoreError> {
            if let Some((values, _)) = domains.get(position) {
                for value in values.clone() {
                    arena.tick()?;
                    keys.push(value);
                    visit(domains, position + 1, keys, result, arena)?;
                    keys.pop();
                }
            } else {
                result.push(if keys.len() == 1 {
                    keys[0]
                } else {
                    arena.tuple(keys.clone())?
                });
            }
            Ok(())
        }
        match self {
            Self::Scalar(value) => arena.domain(value),
            Self::Composite(values) => arena.set(values.into_iter().map(|(key, _)| key).collect()),
            Self::Lazy(function) => {
                let mut keys = vec![];
                visit(&function.domains, 0, &mut vec![], &mut keys, arena)?;
                arena.set(keys)
            }
        }
    }
}
