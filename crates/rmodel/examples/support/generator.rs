//! The seeded generator of well-sorted rmodel models shared by the
//! differential test and the readability loop (plans 0097-0099). Since 0099
//! every expression has a sort-correct type; the 0097 runs used the earlier,
//! arity-only generator in that revision.
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
    fn pick<T: Clone>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())].clone()
    }
}

#[derive(Clone)]
struct Rel {
    name: String,
    var: bool,
    /// The sort of a set, or (from, to) of a binary relation.
    from: String,
    to: Option<String>,
}

struct Generator {
    random: Random,
    sorts: Vec<String>,
    /// Enum members with their enum.
    members: Vec<(String, String)>,
    rels: Vec<Rel>,
    fresh: usize,
    /// Sum sorts with their children (plan 0100).
    sums: Vec<(String, Vec<String>)>,
    /// Since 0103: var attributes of one value, `name: from -> one sort`,
    /// where sort is Int, Bool or the record R (fields n: Int, b: Bool).
    values: Vec<(String, String, String)>,
}

type Locals = Vec<(String, String)>;

impl Generator {
    /// A set of `sort`.
    fn set(&mut self, sort: &str, depth: usize, locals: &Locals, vars: bool) -> String {
        if depth == 0 || self.random.chance(40) {
            let mut options = vec![sort.to_owned()];
            // A sum sort's set may be one of its children's sorts.
            if let Some((_, children)) = self.sums.iter().find(|(sum, _)| sum == sort) {
                options.extend(children.iter().cloned());
            }
            options.extend(
                self.members
                    .iter()
                    .filter(|(_, e)| e == sort)
                    .map(|(m, _)| m.clone()),
            );
            options.extend(
                locals
                    .iter()
                    .filter(|(_, s)| s == sort)
                    .map(|(n, _)| n.clone()),
            );
            options.extend(
                self.rels
                    .iter()
                    .filter(|r| r.to.is_none() && r.from == sort && (vars || !r.var))
                    .map(|r| r.name.clone()),
            );
            if self.random.chance(8) {
                return "none".into();
            }
            return self.random.pick(&options);
        }
        match self.random.below(3) {
            0 => {
                let op = self.random.pick(&["+", "-", "&"]);
                let (l, r) = (
                    self.set(sort, depth - 1, locals, vars),
                    self.set(sort, depth - 1, locals, vars),
                );
                format!("({l} {op} {r})")
            }
            1 => {
                let via = self.random.pick(&self.sorts.clone());
                let (s, p) = (
                    self.join_set(&via, depth - 1, locals, vars),
                    self.pair(&via, sort, depth - 1, locals, vars),
                );
                format!("({s}.{p})")
            }
            _ => {
                let via = self.random.pick(&self.sorts.clone());
                let (p, s) = (
                    self.pair(sort, &via, depth - 1, locals, vars),
                    self.join_set(&via, depth - 1, locals, vars),
                );
                format!("({p}.{s})")
            }
        }
    }
    /// A set operand of a join, where `none` would have no type.
    fn join_set(&mut self, sort: &str, depth: usize, locals: &Locals, vars: bool) -> String {
        // A sum's child would narrow the type; joins and products of a sum
        // use the sum itself.
        if self.sums.iter().any(|(sum, _)| sum == sort) {
            return sort.to_owned();
        }
        let set = self.set(sort, depth, locals, vars);
        if set == "none" { sort.to_owned() } else { set }
    }
    /// A binary relation from `from` to `to`.
    fn pair(&mut self, from: &str, to: &str, depth: usize, locals: &Locals, vars: bool) -> String {
        let relations: Vec<String> = self
            .rels
            .iter()
            .filter(|r| r.from == from && r.to.as_deref() == Some(to) && (vars || !r.var))
            .map(|r| r.name.clone())
            .collect();
        if depth == 0 || self.random.chance(40) {
            if !relations.is_empty() && self.random.chance(70) {
                return self.random.pick(&relations);
            }
            let (l, r) = (
                self.join_set(from, 0, locals, vars),
                self.join_set(to, 0, locals, vars),
            );
            return format!("({l} -> {r})");
        }
        match self.random.below(3) {
            0 => {
                let op = self.random.pick(&["+", "-", "&"]);
                let (l, r) = (
                    self.pair(from, to, depth - 1, locals, vars),
                    self.pair(from, to, depth - 1, locals, vars),
                );
                format!("({l} {op} {r})")
            }
            1 => {
                let (l, r) = (
                    self.join_set(from, depth - 1, locals, vars),
                    self.join_set(to, depth - 1, locals, vars),
                );
                format!("({l} -> {r})")
            }
            _ => {
                let via = self.random.pick(&self.sorts.clone());
                let (l, r) = (
                    self.pair(from, &via, depth - 1, locals, vars),
                    self.pair(&via, to, depth - 1, locals, vars),
                );
                format!("({l}.{r})")
            }
        }
    }
    /// One integer: a literal, a count, an attribute of a local, or
    /// arithmetic on two of them (plan 0103).
    fn int(&mut self, depth: usize, locals: &Locals) -> String {
        let mut options = vec![self.random.below(3).to_string()];
        for (name, from, sort) in self.values.clone() {
            for (local, local_sort) in locals {
                if *local_sort == from && sort == "Int" {
                    options.push(format!("{local}.{name}"));
                }
                if *local_sort == from && sort == "R" {
                    options.push(format!("{local}.{name}.n"));
                }
            }
        }
        for (local, local_sort) in locals {
            if local_sort == "Int" {
                options.push(local.clone());
            }
        }
        if depth > 0 && self.random.chance(40) {
            let op = self.random.pick(&["+", "-", "*"]);
            let (l, r) = (self.int(depth - 1, locals), self.int(depth - 1, locals));
            return format!("({l} {op} {r})");
        }
        if self.random.chance(15) {
            let sort = self.random.pick(&self.sorts.clone());
            return format!("#{}", self.join_set(&sort, 0, locals, true));
        }
        self.random.pick(&options)
    }
    /// A formula over values: an integer comparison or a Bool attribute.
    fn value_formula(&mut self, locals: &Locals) -> Option<String> {
        let (name, from, sort) = self.random.pick(&self.values.clone());
        let local = locals
            .iter()
            .find(|(_, s)| *s == from)
            .map(|(l, _)| l.clone());
        let Some(local) = local else {
            // Quantify over the attribute's sort to reach a value.
            self.fresh += 1;
            let bound = format!("q{}", self.fresh);
            let mut inner = locals.clone();
            inner.push((bound.clone(), from.clone()));
            let word = self.random.pick(&["all", "some"]);
            let body = self.value_formula(&inner)?;
            return Some(format!("({word} {bound}: {from} | {body})"));
        };
        Some(match sort.as_str() {
            "Bool" => {
                let not = if self.random.chance(50) { "not " } else { "" };
                format!("{not}{local}.{name}")
            }
            "R" if self.random.chance(40) => format!("{local}.{name}.b"),
            _ => {
                let op = self.random.pick(&["<", "<=", ">", ">=", "=", "!="]);
                let (l, r) = (self.int(1, locals), self.int(1, locals));
                // An integer comparison starting with a literal parses as
                // one; `a.x = 1` is a set comparison of the same meaning.
                format!("{l} {op} {r}")
            }
        })
    }
    fn formula(&mut self, depth: usize, locals: &Locals, vars: bool) -> String {
        let sort = self.random.pick(&self.sorts.clone());
        if vars
            && !self.values.is_empty()
            && self.random.chance(25)
            && let Some(formula) = self.value_formula(locals)
        {
            return formula;
        }
        if depth == 0 || self.random.chance(35) {
            return match self.random.below(5) {
                0 => {
                    let word = self.random.pick(&["some", "no", "one", "lone"]);
                    format!("{word} {}", self.set(&sort, 2, locals, vars))
                }
                1 => {
                    let op = self.random.pick(&["in", "=", "!=", "not in"]);
                    let (l, r) = (
                        self.set(&sort, 2, locals, vars),
                        self.set(&sort, 2, locals, vars),
                    );
                    format!("{l} {op} {r}")
                }
                2 => {
                    let to = self.random.pick(&self.sorts.clone());
                    let op = self.random.pick(&["in", "="]);
                    let (l, r) = (
                        self.pair(&sort, &to, 1, locals, vars),
                        self.pair(&sort, &to, 1, locals, vars),
                    );
                    format!("{l} {op} {r}")
                }
                3 => {
                    let op = self.random.pick(&["<", "<=", ">", ">=", "=", "!="]);
                    let value = self.random.below(4);
                    format!("#{} {op} {value}", self.set(&sort, 1, locals, vars))
                }
                _ => format!("some {}", self.set(&sort, 1, locals, vars)),
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
                let word = self.random.pick(&["all", "some"]);
                let mut inner = locals.clone();
                inner.push((name.clone(), sort.clone()));
                format!(
                    "({word} {name}: {sort} | {})",
                    self.formula(depth - 1, &inner, vars)
                )
            }
        }
    }
    /// A formula that reads some var, so it is not constant.
    fn state_formula(&mut self, depth: usize, locals: &Locals) -> String {
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

/// A well-sorted rmodel model for a seed.
pub fn generate(seed: u64) -> String {
    let mut g = Generator {
        random: Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1),
        sorts: Vec::new(),
        members: Vec::new(),
        rels: Vec::new(),
        fresh: 0,
        sums: Vec::new(),
        values: Vec::new(),
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
        g.members.extend([
            ("Ea".to_owned(), "E".to_owned()),
            ("Eb".to_owned(), "E".to_owned()),
        ]);
    }
    // Since 0100: a sum of two sorts.
    if g.sorts.len() >= 2 && g.random.chance(30) {
        let children = vec![g.sorts[0].clone(), g.sorts[1].clone()];
        out.push_str(&format!("type P = {}\n", children.join(" + ")));
        g.sorts.push("P".into());
        g.sums.push(("P".into(), children));
    }
    let statics = g.random.below(3);
    let vars = 1 + g.random.below(2);
    for index in 0..statics + vars {
        let var = index >= statics;
        let binary = g.random.chance(50);
        let from = g.random.pick(&g.sorts.clone());
        let to = g.random.pick(&g.sorts.clone());
        let mult = g.random.pick(&["set", "set", "lone", "one", "some"]);
        let name = format!("{}{index}", if var { "v" } else { "r" });
        let keyword = if var { "var" } else { "rel" };
        if binary {
            out.push_str(&format!("{keyword} {name}: {from} -> {mult} {to}\n"));
        } else {
            out.push_str(&format!("{keyword} {name}: {mult} {from}\n"));
        }
        g.rels.push(Rel {
            name,
            var,
            from,
            to: binary.then_some(to),
        });
    }
    // Since 0103: value attributes of the first sort.
    let host = g.sorts[0].clone();
    let record = g.random.chance(15);
    if record {
        out.push_str("record R { n: Int, b: Bool }\n");
    }
    for (sort, percent) in [
        ("Int", 30),
        ("Bool", 30),
        ("R", if record { 100 } else { 0 }),
    ] {
        if g.random.chance(percent) {
            let name = format!("w{}", g.values.len());
            out.push_str(&format!("var {name}: {host} -> one {sort}\n"));
            g.values.push((name, host.clone(), sort.to_owned()));
        }
    }
    let uses_int = g.values.iter().any(|(_, _, sort)| sort != "Bool");
    if uses_int {
        // Records multiply their fields' values: a smaller range keeps the
        // reference's domains enumerable.
        scope.push(if record {
            "Int = 0..2".to_owned()
        } else {
            "Int = -1..2".to_owned()
        });
    }
    if statics > 0 && g.random.chance(60) {
        let fact = g.formula(2, &Vec::new(), false);
        out.push_str(&format!("fact {{ {fact} }}\n"));
    }
    let init = g.state_formula(1, &Vec::new());
    out.push_str(&format!("init {{ {init} }}\n"));
    for (name, from, sort) in g.values.clone() {
        let value = match sort.as_str() {
            "Int" => "0".to_owned(),
            "Bool" => g.random.pick(&["true", "false"]).to_owned(),
            _ => "R { n: 0, b: false }".to_owned(),
        };
        out.push_str(&format!("init {{ all x: {from} | x.{name} = {value} }}\n"));
    }
    // One action per value attribute, setting it for a parameter.
    for (index, (name, from, sort)) in g.values.clone().into_iter().enumerate() {
        let mut params: Locals = vec![("x".to_owned(), from.clone())];
        if sort != "Bool" {
            params.push(("k".to_owned(), "Int".to_owned()));
        }
        let mut body = String::new();
        if g.random.chance(50) {
            let _ = writeln!(body, "  {}", g.value_formula(&params).unwrap());
        }
        let value = match sort.as_str() {
            "Int" => g.int(1, &params),
            "Bool" => g.random.pick(&["true", "false"]).to_owned(),
            _ => format!(
                "R {{ n: {}, b: {} }}",
                g.int(1, &params),
                g.random.pick(&["true", "false"])
            ),
        };
        let _ = writeln!(body, "  {name}' = {name} - x -> x.{name} + x -> ({value})");
        let signature: Vec<String> = params.iter().map(|(n, s)| format!("{n}: {s}")).collect();
        out.push_str(&format!(
            "action set{index}({}) {{\n{body}}}\n",
            signature.join(", ")
        ));
    }
    for index in 0..1 + g.random.below(3) {
        let params: Locals = (0..g.random.below(3))
            .map(|p| (format!("p{index}_{p}"), g.random.pick(&g.sorts.clone())))
            .collect();
        let mut body = String::new();
        for _ in 0..g.random.below(3) {
            let _ = writeln!(body, "  {}", g.formula(1, &params, true));
        }
        let targets: Vec<Rel> = g.rels.iter().filter(|r| r.var).cloned().collect();
        let target = g.random.pick(&targets);
        let value = match &target.to {
            Some(to) => g.pair(&target.from, to, 2, &params, true),
            None => g.set(&target.from, 2, &params, true),
        };
        let _ = writeln!(body, "  {}' = {value}", target.name);
        let signature: Vec<String> = params.iter().map(|(n, s)| format!("{n}: {s}")).collect();
        out.push_str(&format!(
            "action a{index}({}) {{\n{body}}}\n",
            signature.join(", ")
        ));
    }
    let invariant = g.state_formula(3, &Vec::new());
    let reachable = g.state_formula(3, &Vec::new());
    out.push_str(&format!(
        "invariant Inv {{ {invariant} }}\nreachable Goal {{ {reachable} }}\n"
    ));
    out.push_str(&format!("scope {}, steps = 6\n", scope.join(", ")));
    out
}
