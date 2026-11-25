//! Re-checks a counterexample to induction with the reference evaluator
//! (plan 0106): the model is rewritten with each signature as an enum of
//! the counterexample's atoms, the configuration fixed by facts, the
//! pre-state as the only initial state, and only the counterexample's
//! action instance. The invariant must then break at depth 0 (initial
//! counterexample) or 1 (an action's).
use crate::smt::Cti;
use rmodel::{Compare, Expr, Formula, Model, Mult, Quantifier, SetOp, Shape};

fn expr(e: &Expr) -> String {
    match e {
        Expr::None => "none".into(),
        Expr::Name(name) => name.clone(),
        Expr::Binary(op, l, r) => {
            let symbol = match op {
                SetOp::Union => "+",
                SetOp::Difference => "-",
                SetOp::Intersection => "&",
                SetOp::Product => "->",
                SetOp::Join => ".",
                SetOp::Multiply => "*",
            };
            format!("({} {symbol} {})", expr(l), expr(r))
        }
        _ => unreachable!("no values in the fragment"),
    }
}

fn formula(f: &Formula) -> String {
    match f {
        Formula::Compare(op, l, r) => {
            let symbol = match op {
                Compare::In => "in",
                Compare::NotIn => "not in",
                Compare::Equal => "=",
                Compare::NotEqual => "!=",
            };
            format!("({} {symbol} {})", expr(l), expr(r))
        }
        Formula::Mult(mult, e) => {
            let word = match mult {
                Mult::Some => "some",
                Mult::One => "one",
                Mult::Lone => "lone",
                Mult::Set => return "(no none)".into(),
            };
            format!("({word} {})", expr(e))
        }
        Formula::Empty(e) => format!("(no {})", expr(e)),
        Formula::Not(inner) => format!("(not {})", formula(inner)),
        Formula::And(parts) if parts.is_empty() => "(no none)".into(),
        Formula::And(parts) => format!(
            "({})",
            parts.iter().map(formula).collect::<Vec<_>>().join(" and ")
        ),
        Formula::Or(l, r) => format!("({} or {})", formula(l), formula(r)),
        Formula::Implies(l, r) => format!("({} implies {})", formula(l), formula(r)),
        Formula::Quantified(q, v, sort, body) => {
            let word = match q {
                Quantifier::All => "all",
                Quantifier::Some => "some",
            };
            format!("({word} {v}: {sort} | {})", formula(body))
        }
        Formula::Ints(..) => unreachable!("no counts in the fragment"),
    }
}

fn mult(m: Mult) -> &'static str {
    match m {
        Mult::Set => "set",
        Mult::Lone => "lone",
        Mult::One => "one",
        Mult::Some => "some",
    }
}

fn value(tuples: &[Vec<String>]) -> String {
    if tuples.is_empty() {
        return "none".into();
    }
    tuples
        .iter()
        .map(|t| format!("({})", t.join(" -> ")))
        .collect::<Vec<_>>()
        .join(" + ")
}

/// The rewritten model's source.
pub fn source(model: &Model, cti: &Cti, check: &str, step: &str) -> String {
    let mut out = String::from("model Cti\n");
    for sig in &model.sigs {
        let atoms: Vec<String> = (0..cti.sizes[sig]).map(|i| format!("{sig}{i}")).collect();
        out.push_str(&format!("enum {sig} = {}\n", atoms.join(" | ")));
    }
    for (name, members) in &model.enums {
        out.push_str(&format!("enum {name} = {}\n", members.join(" | ")));
    }
    for relation in &model.relations {
        let keyword = if relation.var { "var" } else { "rel" };
        let shape = match &relation.shape {
            Shape::Unary { sig, mult: m } => format!("{} {sig}", mult(*m)),
            Shape::Binary { from, to, mult: m } => format!("{from} -> {} {to}", mult(*m)),
        };
        out.push_str(&format!("{keyword} {}: {shape}\n", relation.name));
    }
    for fact in &model.facts {
        out.push_str(&format!("fact {{ {} }}\n", formula(fact)));
    }
    for relation in model.relations.iter().filter(|r| !r.var) {
        out.push_str(&format!(
            "fact {{ {} = {} }}\n",
            relation.name,
            value(&cti.relations[&relation.name])
        ));
    }
    if step == "init" {
        for f in &model.init {
            out.push_str(&format!("init {{ {} }}\n", formula(f)));
        }
    }
    for var in model.vars() {
        out.push_str(&format!(
            "init {{ {} = {} }}\n",
            var.name,
            value(&cti.relations[&var.name])
        ));
    }
    if let Some(action) = model.actions.iter().find(|a| a.name == step) {
        let params: Vec<String> = action
            .params
            .iter()
            .map(|(p, s)| format!("{p}: {s}"))
            .collect();
        out.push_str(&format!(
            "action {}({}) {{\n",
            action.name,
            params.join(", ")
        ));
        for (p, _) in &action.params {
            out.push_str(&format!("  {p} = {}\n", cti.args[p]));
        }
        for guard in &action.guards {
            out.push_str(&format!("  {}\n", formula(guard)));
        }
        for (var, v) in &action.assignments {
            out.push_str(&format!("  {var}' = {}\n", expr(v)));
        }
        out.push_str("}\n");
    }
    for c in model
        .checks
        .iter()
        .filter(|c| c.kind == rmodel::CheckKind::Invariant)
    {
        out.push_str(&format!(
            "invariant {} {{ {} }}\n",
            c.name,
            formula(&c.formula)
        ));
    }
    let _ = check;
    out.push_str("scope steps = 2\n");
    out
}

/// Whether the reference evaluator confirms the counterexample.
pub fn confirms(model: &Model, cti: &Cti, check: &str, step: &str) -> Result<(), String> {
    let text = source(model, cti, check, step);
    let rewritten = rmodel::parse(&text).map_err(|e| format!("{e}\n{text}"))?;
    let report = rmodel::reference::analyze(&rewritten, 1_000_000);
    use rmodel::reference::Outcome;
    let depth = if step == "init" { 0 } else { 1 };
    // The pre-state of an action's counterexample satisfies every invariant.
    if step != "init" {
        for verdict in &report.verdicts {
            if verdict.outcome == (Outcome::Violated { depth: 0 }) {
                return Err(format!("the pre-state breaks {}\n{text}", verdict.check));
            }
        }
    }
    let outcome = if check == rmodel::DECLARATIONS {
        report.declarations
    } else {
        report
            .verdicts
            .iter()
            .find(|v| v.check == check)
            .expect("the check")
            .outcome
    };
    if outcome == (Outcome::Violated { depth }) {
        Ok(())
    } else {
        Err(format!(
            "the reference says {outcome:?}, not a violation at depth {depth}\n{text}"
        ))
    }
}
