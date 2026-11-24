//! differential <first-seed> <last-seed> <output-dir>: generates a typed
//! rmodel model per seed and compares the reference evaluator, rtlc on the
//! TLA+ lowering and ralloy's SAT on the Alloy lowering (plan 0097).
//! Writes one JSON line per check to results.jsonl, and the source of every
//! model with a disagreement or an engine error to models/<seed>.model.
use std::fmt::Write as _;

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

#[derive(Clone)]
struct Rel {
    name: String,
    var: bool,
    binary: bool,
}

struct Generator {
    random: Random,
    sorts: Vec<String>,
    members: Vec<String>,
    rels: Vec<Rel>,
    fresh: usize,
    /// Building a join operand, where `none` has no arity.
    joining: bool,
}

impl Generator {
    /// A unary expression over the names in scope.
    fn set(&mut self, depth: usize, locals: &[String], vars: bool) -> String {
        let leaf = depth == 0 || self.random.chance(40);
        if leaf {
            let mut options: Vec<String> = self.sorts.clone();
            options.extend(self.members.iter().cloned());
            options.extend(locals.iter().cloned());
            options.extend(
                self.rels
                    .iter()
                    .filter(|r| !r.binary && (vars || !r.var))
                    .map(|r| r.name.clone()),
            );
            if self.random.chance(10) && !self.joining {
                return "none".into();
            }
            return self.random.pick(&options).clone();
        }
        match self.random.below(3) {
            0 => {
                let op = *self.random.pick(&["+", "-", "&"]);
                let (l, r) = (
                    self.set(depth - 1, locals, vars),
                    self.set(depth - 1, locals, vars),
                );
                format!("({l} {op} {r})")
            }
            1 if self.has_binary(vars) => {
                let outer = std::mem::replace(&mut self.joining, true);
                let (s, p) = (
                    self.set(depth - 1, locals, vars),
                    self.pair(depth - 1, locals, vars),
                );
                self.joining = outer;
                format!("({s}.{p})")
            }
            2 if self.has_binary(vars) => {
                let outer = std::mem::replace(&mut self.joining, true);
                let (p, s) = (
                    self.pair(depth - 1, locals, vars),
                    self.set(depth - 1, locals, vars),
                );
                self.joining = outer;
                format!("({p}.{s})")
            }
            _ => self.set(0, locals, vars),
        }
    }
    fn has_binary(&self, vars: bool) -> bool {
        self.rels.iter().any(|r| r.binary && (vars || !r.var))
    }
    /// A binary expression.
    fn pair(&mut self, depth: usize, locals: &[String], vars: bool) -> String {
        let binaries: Vec<String> = self
            .rels
            .iter()
            .filter(|r| r.binary && (vars || !r.var))
            .map(|r| r.name.clone())
            .collect();
        if depth == 0 || self.random.chance(40) {
            if !binaries.is_empty() && self.random.chance(70) {
                return self.random.pick(&binaries).clone();
            }
            let (l, r) = (self.set(0, locals, vars), self.set(0, locals, vars));
            return format!("({l} -> {r})");
        }
        match self.random.below(3) {
            0 => {
                let op = *self.random.pick(&["+", "-", "&"]);
                let (l, r) = (
                    self.pair(depth - 1, locals, vars),
                    self.pair(depth - 1, locals, vars),
                );
                format!("({l} {op} {r})")
            }
            1 => {
                let (l, r) = (
                    self.set(depth - 1, locals, vars),
                    self.set(depth - 1, locals, vars),
                );
                format!("({l} -> {r})")
            }
            _ => {
                let outer = std::mem::replace(&mut self.joining, true);
                let (l, r) = (
                    self.pair(depth - 1, locals, vars),
                    self.pair(depth - 1, locals, vars),
                );
                self.joining = outer;
                format!("({l}.{r})")
            }
        }
    }
    fn formula(&mut self, depth: usize, locals: &[String], vars: bool) -> String {
        let leaf = depth == 0 || self.random.chance(35);
        if leaf {
            return match self.random.below(5) {
                0 => {
                    let word = *self.random.pick(&["some", "no", "one", "lone"]);
                    format!("{word} {}", self.set(2, locals, vars))
                }
                1 => {
                    let op = *self.random.pick(&["in", "=", "!=", "not in"]);
                    let (l, r) = (self.set(2, locals, vars), self.set(2, locals, vars));
                    format!("{l} {op} {r}")
                }
                2 if self.has_binary(vars) => {
                    let op = *self.random.pick(&["in", "="]);
                    let (l, r) = (self.pair(1, locals, vars), self.pair(1, locals, vars));
                    format!("{l} {op} {r}")
                }
                3 => {
                    let op = *self.random.pick(&["<", "<=", ">", ">=", "=", "!="]);
                    let value = self.random.below(4);
                    format!("#{} {op} {value}", self.set(1, locals, vars))
                }
                _ => format!("some {}", self.set(1, locals, vars)),
            };
        }
        match self.random.below(5) {
            0 => format!("not ({})", self.formula(depth - 1, locals, vars)),
            1 => format!(
                "({} and {})",
                self.formula(depth - 1, locals, vars),
                self.formula(depth - 1, locals, vars)
            ),
            2 => format!(
                "({} or {})",
                self.formula(depth - 1, locals, vars),
                self.formula(depth - 1, locals, vars)
            ),
            3 => format!(
                "({} implies {})",
                self.formula(depth - 1, locals, vars),
                self.formula(depth - 1, locals, vars)
            ),
            _ => {
                self.fresh += 1;
                let name = format!("q{}", self.fresh);
                let sort = self.random.pick(&self.sorts.clone()).clone();
                let word = *self.random.pick(&["all", "some"]);
                let mut inner = locals.to_vec();
                inner.push(name.clone());
                format!(
                    "({word} {name}: {sort} | {})",
                    self.formula(depth - 1, &inner, vars)
                )
            }
        }
    }
    /// A formula that reads some var, so it is not constant.
    fn state_formula(&mut self, depth: usize, locals: &[String]) -> String {
        let vars: Vec<String> = self
            .rels
            .iter()
            .filter(|r| r.var)
            .map(|r| r.name.clone())
            .collect();
        for _ in 0..32 {
            let formula = self.formula(depth, locals, true);
            let mentions = formula
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|token| vars.iter().any(|v| v == token));
            if mentions {
                return formula;
            }
        }
        format!("some {}", vars[0])
    }
}

fn generate(seed: u64) -> String {
    let mut g = Generator {
        random: Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1),
        sorts: Vec::new(),
        members: Vec::new(),
        rels: Vec::new(),
        fresh: 0,
        joining: false,
    };
    let mut out = format!("model G{seed}\n");
    let mut scope = Vec::new();
    for index in 0..1 + g.random.below(2) {
        let name = format!("S{index}");
        out.push_str(&format!("sig {name}\n"));
        scope.push(format!("{name} = {}", 1 + g.random.below(3)));
        g.sorts.push(name);
    }
    if g.random.chance(40) {
        out.push_str("enum E = Ea | Eb\n");
        g.sorts.push("E".into());
        g.members.extend(["Ea".to_owned(), "Eb".to_owned()]);
    }
    let statics = g.random.below(3);
    let vars = 1 + g.random.below(2);
    for index in 0..statics + vars {
        let var = index >= statics;
        let binary = g.random.chance(50);
        let from = g.random.pick(&g.sorts.clone()).clone();
        let to = g.random.pick(&g.sorts.clone()).clone();
        let mult = *g.random.pick(&["set", "set", "lone", "one", "some"]);
        let name = format!("{}{index}", if var { "v" } else { "r" });
        let keyword = if var { "var" } else { "rel" };
        if binary {
            out.push_str(&format!("{keyword} {name}: {from} -> {mult} {to}\n"));
        } else {
            out.push_str(&format!("{keyword} {name}: {mult} {from}\n"));
        }
        g.rels.push(Rel { name, var, binary });
    }
    if statics > 0 && g.random.chance(60) {
        let fact = g.formula(2, &[], false);
        out.push_str(&format!("fact {{ {fact} }}\n"));
    }
    let init = g.state_formula(1, &[]);
    out.push_str(&format!("init {{ {init} }}\n"));
    for index in 0..1 + g.random.below(3) {
        let params: Vec<(String, String)> = (0..g.random.below(3))
            .map(|p| {
                (
                    format!("p{index}_{p}"),
                    g.random.pick(&g.sorts.clone()).clone(),
                )
            })
            .collect();
        let locals: Vec<String> = params.iter().map(|(name, _)| name.clone()).collect();
        let mut body = String::new();
        for _ in 0..g.random.below(3) {
            let _ = writeln!(body, "  {}", g.formula(1, &locals, true));
        }
        let target = g
            .random
            .pick(&g.rels.iter().filter(|r| r.var).cloned().collect::<Vec<_>>())
            .clone();
        let value = if target.binary {
            g.pair(2, &locals, true)
        } else {
            g.set(2, &locals, true)
        };
        let _ = writeln!(body, "  {}' = {value}", target.name);
        let signature: Vec<String> = params.iter().map(|(n, s)| format!("{n}: {s}")).collect();
        out.push_str(&format!(
            "action a{index}({}) {{\n{body}}}\n",
            signature.join(", ")
        ));
    }
    let invariant = g.state_formula(3, &[]);
    let reachable = g.state_formula(3, &[]);
    out.push_str(&format!(
        "invariant Inv {{ {invariant} }}\nreachable Goal {{ {reachable} }}\n"
    ));
    out.push_str(&format!("scope {}, steps = 6\n", scope.join(", ")));
    out
}

fn reference_word(outcome: rmodel::reference::Outcome) -> (String, Option<usize>) {
    use rmodel::reference::Outcome;
    match outcome {
        Outcome::Empty => ("empty".into(), None),
        Outcome::Holds => ("holds".into(), None),
        Outcome::Violated { depth } => ("violated".into(), Some(depth)),
        Outcome::Reachable { depth } => ("reachable".into(), Some(depth)),
        Outcome::Unreachable => ("unreachable".into(), None),
        Outcome::Unknown => ("unknown".into(), None),
    }
}

fn explicit(model: &rmodel::Model, check: &rmodel::Check) -> String {
    let Ok((source, config)) = rmodel::to_tla(model, &check.name) else {
        return "lowering-error".into();
    };
    let program = match idealis_core::compile_tla_with_options(
        &source,
        &config,
        idealis_core::CompileOptions {
            irVersion: idealis_core::IrVersion::V2,
        },
    ) {
        Ok(program) => program,
        Err(error) => return format!("error: {}", error.message),
    };
    match program.check(idealis_core::SafetyBounds {
        max_depth: 1_000,
        max_states: 200_000,
        max_work: 50_000_000,
        max_temporal_nodes: None,
    }) {
        Err(error) => format!("error: {}", error.message),
        Ok(result) => match (
            result.status.as_str(),
            result.violation.is_some(),
            check.kind,
        ) {
            ("complete", _, rmodel::CheckKind::Invariant) => "holds".into(),
            ("complete", _, rmodel::CheckKind::Reachable) => "unreachable".into(),
            (_, true, rmodel::CheckKind::Invariant) => "violated".into(),
            (_, true, rmodel::CheckKind::Reachable) => "reachable".into(),
            (other, _, _) => other.into(),
        },
    }
}

fn sat(alloy: &Result<ralloy::Model, String>, check: &rmodel::Check) -> String {
    let alloy = match alloy {
        Ok(alloy) => alloy,
        Err(error) => return format!("error: {error}"),
    };
    let label = rmodel::alloy_label(&check.name);
    let Some(index) = alloy.commands().iter().position(|c| c.label == label) else {
        return "error: no command".into();
    };
    match alloy.analyze(index, 5_000_000) {
        Err(error) => format!("error: {error}"),
        Ok(analysis) => match analysis.status {
            ralloy::Status::Counterexample => "violated".into(),
            ralloy::Status::NoCounterexample => "holds".into(),
            ralloy::Status::Instance => "reachable".into(),
            // A check without instances has inconsistent facts: empty. A run
            // without one is empty or unreachable; compared below.
            ralloy::Status::NoInstance if check.kind == rmodel::CheckKind::Invariant => {
                "empty".into()
            }
            ralloy::Status::NoInstance => "unreachable".into(),
            ralloy::Status::Unknown => "unknown".into(),
        },
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [first, last, output] = arguments.as_slice() else {
        panic!("usage: differential <first-seed> <last-seed> <output-dir>");
    };
    let (first, last): (u64, u64) = (first.parse().unwrap(), last.parse().unwrap());
    std::fs::create_dir_all(format!("{output}/models")).unwrap();
    let mut lines = String::new();
    for seed in first..=last {
        let source = generate(seed);
        let model = match rmodel::parse(&source) {
            Ok(model) => model,
            Err(error) => {
                let _ = writeln!(
                    lines,
                    "{{\"seed\":{seed},\"generator\":{:?}}}",
                    error.to_string()
                );
                std::fs::write(format!("{output}/models/{seed}.model"), &source).unwrap();
                continue;
            }
        };
        let verdicts = rmodel::reference::evaluate(&model, 200_000);
        let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model)).map_err(|e| e.to_string());
        let mut keep = false;
        for check in &model.checks {
            let (reference, depth) = reference_word(
                verdicts
                    .iter()
                    .find(|v| v.check == check.name)
                    .unwrap()
                    .outcome,
            );
            let explicit = explicit(&model, check);
            let sat = sat(&alloy, check);
            let deep = depth.is_some_and(|d| d >= model.scope.steps);
            let explicit_agrees = explicit == reference;
            let sat_agrees =
                sat == reference || deep || reference == "empty" && sat == "unreachable";
            if !explicit_agrees || !sat_agrees || reference == "unknown" {
                keep = true;
            }
            let _ = writeln!(
                lines,
                "{{\"seed\":{seed},\"check\":{:?},\"reference\":{reference:?},\"depth\":{},\"explicit\":{explicit:?},\"sat\":{sat:?},\"deep\":{deep},\"explicitAgrees\":{explicit_agrees},\"satAgrees\":{sat_agrees}}}",
                check.name,
                depth.map_or("null".into(), |d| d.to_string()),
            );
        }
        if keep {
            std::fs::write(format!("{output}/models/{seed}.model"), &source).unwrap();
        }
    }
    std::fs::write(format!("{output}/results.jsonl"), lines).unwrap();
}
