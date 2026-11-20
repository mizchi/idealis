use crate::analysis::{Node, NodeMut, analysis_children, walk_mut};
use crate::{LevelSummary, Result, unsupported};
use rtlc::ir::{self, Expression as I, Step, TemporalFormula as T};
use std::collections::{HashMap, HashSet};

pub(crate) fn level_summary(node: Node<'_>, cache: &HashMap<usize, LevelSummary>) -> LevelSummary {
    let mut summary = LevelSummary::default();
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        let children = analysis_children(node);
        match node {
            Node::Expression(I::Parameter { slot }) => {
                summary.parameters.insert(*slot);
            }
            Node::Expression(I::Capture { slot }) => {
                summary.captures.insert(*slot);
            }
            Node::Expression(I::Read { next, .. }) => {
                summary.base = summary.base.max(if *next { 2 } else { 1 });
            }
            Node::Expression(I::Prime { .. }) => summary.base = 2,
            Node::Expression(I::Enabled { .. }) => summary.base = summary.base.max(1),
            Node::Expression(I::Frame { .. })
            | Node::Step(Step::Frame { .. })
            | Node::Temporal(T::Frame { .. }) => {
                let mut body = LevelSummary::default();
                for edge in &children {
                    if edge.descriptor.scope == "frame" {
                        merge(&mut body, level_summary(edge.node, cache));
                    }
                }
                summary.base = summary.base.max(body.base);
                for (slots, role) in [
                    (&body.parameters, "frame-actual"),
                    (&body.captures, "frame-capture"),
                ] {
                    let actuals: Vec<_> = children
                        .iter()
                        .filter(|edge| edge.descriptor.role == role)
                        .collect();
                    for slot in slots {
                        if let Some(edge) = actuals.get(*slot) {
                            pending.push(edge.node);
                        }
                    }
                }
            }
            Node::Expression(I::Call { definition, .. })
            | Node::Step(Step::Call { definition, .. })
            | Node::Temporal(T::Call { definition, .. }) => {
                if let Some(body) = cache.get(definition) {
                    summary.base = summary.base.max(body.base);
                    for (slots, role) in [
                        (&body.parameters, "call-actual"),
                        (&body.captures, "call-capture"),
                    ] {
                        let actuals: Vec<_> = children
                            .iter()
                            .filter(|edge| edge.descriptor.role == role)
                            .collect();
                        for slot in slots {
                            if let Some(edge) = actuals.get(*slot) {
                                pending.push(edge.node);
                            }
                        }
                    }
                }
            }
            _ => pending.extend(children.into_iter().map(|edge| edge.node)),
        }
    }
    summary
}
fn merge(target: &mut LevelSummary, source: LevelSummary) {
    target.base = target.base.max(source.base);
    target.parameters.extend(source.parameters);
    target.captures.extend(source.captures);
}
pub(crate) fn capture_slots(node: Node<'_>) -> HashSet<usize> {
    let mut slots = HashSet::new();
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        if let Node::Expression(I::Capture { slot }) = node {
            slots.insert(*slot);
        }
        pending.extend(
            analysis_children(node)
                .into_iter()
                .filter(|edge| edge.descriptor.scope != "frame")
                .map(|edge| edge.node),
        );
    }
    slots
}
pub(crate) fn local_scope(node: Node<'_>) -> usize {
    let mut maximum = 0;
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        maximum = maximum.max(match node {
            Node::Expression(I::Bound { slot })
            | Node::Binding(ir::Binding { slot, .. })
            | Node::Step(Step::Bind { slot, .. }) => slot + 1,
            Node::Expression(I::Fold { accumulator, .. }) => accumulator + 2,
            _ => 0,
        });
        pending.extend(
            analysis_children(node)
                .into_iter()
                .filter(|edge| edge.descriptor.scope != "frame")
                .map(|edge| edge.node),
        );
    }
    maximum
}
pub(crate) fn remap_captures(node: NodeMut<'_>, mapping: &HashMap<usize, usize>) {
    walk_mut(node, true, &mut |node| {
        if let NodeMut::Expression(I::Capture { slot }) = node {
            *slot = mapping[slot];
        }
    });
}
pub(crate) fn call_ids(node: Node<'_>) -> HashSet<usize> {
    let mut ids = HashSet::new();
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        if let Node::Call(id) = node {
            ids.insert(*id);
        }
        pending.extend(analysis_children(node).into_iter().map(|edge| edge.node));
    }
    ids
}
pub(crate) fn remap_calls(node: NodeMut<'_>, mapping: &HashMap<usize, usize>) {
    walk_mut(node, false, &mut |node| {
        if let NodeMut::Call(id) = node {
            *id = mapping[id];
        }
    });
}
pub(crate) fn trim_call_captures(node: NodeMut<'_>, mappings: &[HashMap<usize, usize>]) {
    walk_mut(node, false, &mut |node| {
        if let NodeMut::Expression(I::Call {
            definition,
            captures,
            ..
        })
        | NodeMut::Step(Step::Call {
            definition,
            captures,
            ..
        })
        | NodeMut::Temporal(T::Call {
            definition,
            captures,
            ..
        }) = node
        {
            let mut slot = 0;
            captures.retain(|_| {
                let keep = mappings[*definition].contains_key(&slot);
                slot += 1;
                keep
            });
        }
    });
}
fn compact_frame(
    captures: &mut Vec<I>,
    count: &mut usize,
    scope: &mut usize,
    mut body: NodeMut<'_>,
) -> bool {
    let used = capture_slots(body.view());
    let mapping: HashMap<_, _> = (0..captures.len())
        .filter(|slot| used.contains(slot))
        .enumerate()
        .map(|(new, old)| (old, new))
        .collect();
    let changed = *count != mapping.len();
    remap_captures(body.reborrow(), &mapping);
    let mut slot = 0;
    captures.retain(|_| {
        let keep = mapping.contains_key(&slot);
        slot += 1;
        keep
    });
    *count = captures.len();
    *scope = local_scope(body.view());
    changed
}
pub(crate) fn compact_anonymous(node: NodeMut<'_>) -> bool {
    let mut changed = false;
    walk_mut(node, false, &mut |node| match node {
        NodeMut::Expression(I::Frame {
            captures,
            capture_count,
            local_scope,
            body,
            ..
        }) => {
            changed |= compact_frame(
                captures,
                capture_count,
                local_scope,
                NodeMut::Expression(body),
            );
        }
        NodeMut::Step(Step::Frame {
            captures,
            capture_count,
            local_scope,
            branches,
            ..
        }) => {
            let used: HashSet<_> = branches
                .iter()
                .flat_map(|branch| capture_slots(Node::Branch(branch)))
                .collect();
            let mapping: HashMap<_, _> = (0..captures.len())
                .filter(|slot| used.contains(slot))
                .enumerate()
                .map(|(new, old)| (old, new))
                .collect();
            changed |= *capture_count != mapping.len();
            for branch in branches.iter_mut() {
                remap_captures(NodeMut::Branch(branch), &mapping);
            }
            let mut slot = 0;
            captures.retain(|_| {
                let keep = mapping.contains_key(&slot);
                slot += 1;
                keep
            });
            *capture_count = captures.len();
            *local_scope = branches
                .iter()
                .map(|branch| self::local_scope(Node::Branch(branch)))
                .max()
                .unwrap_or(0);
        }
        NodeMut::Temporal(T::Frame {
            captures,
            capture_count,
            local_scope,
            formula,
            ..
        }) => {
            changed |= compact_frame(
                captures,
                capture_count,
                local_scope,
                NodeMut::Temporal(formula),
            );
        }
        _ => {}
    });
    changed
}
pub(crate) fn check_primes(node: Node<'_>, cache: &HashMap<usize, LevelSummary>) -> Result<()> {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        let children = analysis_children(node);
        if let Node::Expression(I::Prime { .. }) = node {
            for edge in &children {
                if edge.descriptor.role == "prime-argument"
                    && level_summary(edge.node, cache).base > 1
                {
                    return unsupported("prime of an action expression");
                }
            }
        }
        pending.extend(children.into_iter().map(|edge| edge.node));
    }
    Ok(())
}
pub(crate) fn check_depth(node: Node<'_>) -> Result<()> {
    check_depth_from(node, 0)
}
pub(crate) fn check_depth_from(node: Node<'_>, depth: usize) -> Result<()> {
    let mut pending = vec![(node, depth, depth)];
    while let Some((node, depth, temporal_depth)) = pending.pop() {
        if matches!(node, Node::Temporal(_)) && temporal_depth > 128 {
            return unsupported("compiled temporal nesting exceeds 128");
        }
        if matches!(
            node,
            Node::Expression(_) | Node::Step(Step::Frame { .. }) | Node::Temporal(T::Frame { .. })
        ) && depth > 128
        {
            return unsupported("compiled expression nesting exceeds 128");
        }
        pending.extend(analysis_children(node).into_iter().map(|edge| {
            (
                edge.node,
                depth + edge.depth,
                temporal_depth + usize::from(matches!(node, Node::Temporal(_))),
            )
        }));
    }
    Ok(())
}

pub(crate) fn action_dimensions(branches: &[ir::Branch]) -> (usize, usize) {
    action_dimensions_shared(branches, &HashMap::new())
}
fn action_dimensions_shared(
    branches: &[ir::Branch],
    sizes: &HashMap<usize, (usize, usize)>,
) -> (usize, usize) {
    let mut paths = 0usize;
    let mut longest = 0usize;
    for branch in branches {
        let children = analysis_children(Node::Branch(branch));
        let mut branch_paths = 1usize;
        let mut length = 0usize;
        for edge in children
            .into_iter()
            .filter(|edge| edge.descriptor.role == "action-step")
        {
            let Node::Step(step) = edge.node else {
                continue;
            };
            length += 1;
            let children = analysis_children(Node::Step(step));
            if let Step::Call { definition, .. } = step
                && let Some((count, maximum)) = sizes.get(definition)
            {
                branch_paths = branch_paths.saturating_mul(*count).min(4097);
                length = length.saturating_add(*maximum).min(257);
            }
            if matches!(step, Step::Frame { .. }) {
                let mut count = 0usize;
                let mut maximum = 0usize;
                for edge in children
                    .into_iter()
                    .filter(|edge| edge.descriptor.role == "action-frame-body")
                {
                    if let Node::Branch(branch) = edge.node {
                        let (inner_count, inner_steps) =
                            action_dimensions_shared(std::slice::from_ref(branch), sizes);
                        count = count.saturating_add(inner_count).min(4097);
                        maximum = maximum.max(inner_steps);
                    }
                }
                branch_paths = branch_paths.saturating_mul(count).min(4097);
                length = length.saturating_add(maximum).min(257);
            }
        }
        paths = paths.saturating_add(branch_paths).min(4097);
        longest = longest.max(length);
    }
    (paths, longest)
}
pub(crate) fn check_shared_expansion(
    program: &ir::SafetyProgram,
    definitions: &[&ir::Definition],
    order: &[usize],
) -> Result<()> {
    let mut sizes = HashMap::new();
    let check = |size: (usize, usize)| -> Result<()> {
        if size.0 > 4096 || size.1 > 256 {
            return unsupported("action expansion exceeds 4096 branches or 256 steps");
        }
        Ok(())
    };
    for id in order {
        let definition = definitions[*id];
        if let ir::DefinitionBody::Action(branches) = &definition.body {
            let size = action_dimensions_shared(branches, &sizes);
            check(size)?;
            if !definition.recursive {
                sizes.insert(*id, size);
            }
        }
    }
    check(action_dimensions_shared(&program.initial, &sizes))?;
    check(action_dimensions_shared(&program.next, &sizes))?;
    let mut pending = vec![Node::Program(program)];
    pending.extend(
        definitions
            .iter()
            .map(|definition| Node::Body(&definition.body)),
    );
    while let Some(node) = pending.pop() {
        if let Node::Expression(I::Enabled { branches, .. }) = node {
            check(action_dimensions_shared(branches, &sizes))?;
        }
        pending.extend(analysis_children(node).into_iter().map(|edge| edge.node));
    }
    Ok(())
}
pub(crate) fn limited(branches: Vec<ir::Branch>) -> Result<Vec<ir::Branch>> {
    let (paths, longest) = action_dimensions(&branches);
    if paths > 4096 || longest > 256 {
        return unsupported("action expansion exceeds 4096 branches or 256 steps");
    }
    Ok(branches)
}
pub(crate) fn expression_frame(frame: &T, body: I) -> I {
    let T::Frame {
        arguments,
        captures,
        parameter_count,
        capture_count,
        ..
    } = frame
    else {
        unreachable!()
    };
    let mut value = I::Frame {
        arguments: arguments.clone(),
        captures: captures.clone(),
        parameter_count: *parameter_count,
        capture_count: *capture_count,
        local_scope: local_scope(Node::Expression(&body)),
        body: Box::new(body),
    };
    compact_anonymous(NodeMut::Expression(&mut value));
    value
}
pub(crate) fn atom_expression(formula: &T) -> Option<I> {
    let children = analysis_children(Node::Temporal(formula));
    match formula {
        T::Atom { .. } => children.into_iter().find_map(|edge| {
            if let Node::Expression(value) = edge.node {
                Some(value.clone())
            } else {
                None
            }
        }),
        T::Frame { .. } => children
            .into_iter()
            .filter(|edge| edge.descriptor.role == "frame-body")
            .find_map(|edge| {
                if let Node::Temporal(value) = edge.node {
                    atom_expression(value).map(|body| expression_frame(formula, body))
                } else {
                    None
                }
            }),
        _ => None,
    }
}
pub(crate) fn validate_temporal(
    formula: &T,
    path: &[String],
    direct: bool,
    frames: &[&T],
    cache: &HashMap<usize, LevelSummary>,
    definitions: &[&ir::Definition],
) -> Result<()> {
    if let T::Call {
        definition,
        arguments,
        captures,
    } = formula
    {
        if frames.len() >= 128 {
            return unsupported("temporal call validation nesting exceeds 128");
        }
        let definition = definitions
            .get(*definition)
            .ok_or_else(|| crate::CompileError {
                message: "temporal definition ID".into(),
            })?;
        let ir::DefinitionBody::Temporal(body) = &definition.body else {
            return unsupported("temporal definition kind");
        };
        let frame = T::Frame {
            arguments: arguments.clone(),
            captures: captures.clone(),
            parameter_count: definition.parameter_count,
            capture_count: definition.capture_count,
            local_scope: definition.local_scope,
            formula: body.clone(),
        };
        return validate_temporal(&frame, path, direct, frames, cache, definitions);
    }
    let children = analysis_children(Node::Temporal(formula));
    if let T::Atom { .. } = formula {
        for edge in children {
            if let Node::Expression(expression) = edge.node {
                let expression = frames.iter().rev().fold(expression.clone(), |body, frame| {
                    expression_frame(frame, body)
                });
                let tail: Vec<_> = path.iter().rev().take(2).map(String::as_str).collect();
                if level_summary(Node::Expression(&expression), cache).base > 1
                    && !(direct && path.len() == 1)
                    && tail != ["eventually", "always"]
                    && tail != ["always", "eventually"]
                {
                    return unsupported("temporal actions require []<>A or <>[]A");
                }
            }
        }
        return Ok(());
    }
    for edge in children {
        if let Node::Temporal(value) = edge.node {
            let mut path = path.to_vec();
            let mut frames = frames.to_vec();
            match formula {
                T::Frame { .. } => frames.push(formula),
                T::Always { .. } => path.push("always".into()),
                T::Eventually { .. } => path.push("eventually".into()),
                _ => {}
            }
            validate_temporal(value, &path, direct, &frames, cache, definitions)?;
        }
    }
    Ok(())
}
