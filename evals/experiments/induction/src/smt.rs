//! z3 as a subprocess, and concrete counterexamples to induction read back
//! from it over fixed small universes (plan 0106).
use std::collections::BTreeMap;
use std::io::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Unsat,
    Sat,
    Unknown(String),
}

pub fn run(smt: &str, seconds: u32) -> (Answer, String) {
    let mut child = std::process::Command::new("z3")
        .args(["-in", &format!("-T:{seconds}")])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("z3 on PATH");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(smt.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let first = text.lines().next().unwrap_or("").trim().to_owned();
    let answer = match first.as_str() {
        "unsat" => Answer::Unsat,
        "sat" => Answer::Sat,
        other => Answer::Unknown(
            format!("{other} {}", String::from_utf8_lossy(&output.stderr))
                .trim()
                .to_owned(),
        ),
    };
    (answer, text)
}

/// A concrete counterexample: the size of each signature, the value of
/// each relation (pre and post copies) as tuples of atom names, and the
/// action's arguments.
#[derive(Clone, Debug, Default)]
pub struct Cti {
    pub sizes: BTreeMap<String, usize>,
    pub relations: BTreeMap<String, Vec<Vec<String>>>,
    pub args: BTreeMap<String, String>,
}

/// Atom names of a sort in a fixed universe.
pub fn atoms(
    model: &rmodel::Model,
    sizes: &BTreeMap<String, usize>,
    sort: &str,
) -> Vec<(String, String)> {
    if let Some((_, members)) = model.enums.iter().find(|(name, _)| name == sort) {
        return members
            .iter()
            .map(|m| (m.clone(), format!("e_{m}")))
            .collect();
    }
    (0..sizes[sort])
        .map(|i| (format!("{sort}{i}"), format!("c_{sort}_{i}")))
        .collect()
}

fn tuples(
    model: &rmodel::Model,
    sizes: &BTreeMap<String, usize>,
    columns: &[String],
) -> Vec<Vec<(String, String)>> {
    let mut all = vec![Vec::new()];
    for column in columns {
        let options = atoms(model, sizes, column);
        all = all
            .into_iter()
            .flat_map(|prefix: Vec<(String, String)>| {
                options.iter().map(move |o| {
                    let mut next = prefix.clone();
                    next.push(o.clone());
                    next
                })
            })
            .collect();
    }
    all
}

fn columns(shape: &rmodel::Shape) -> Vec<String> {
    match shape {
        rmodel::Shape::Unary { sig, .. } => vec![sig.clone()],
        rmodel::Shape::Binary { from, to, .. } => vec![from.clone(), to.clone()],
    }
}

/// Size assignments in order of total size, each sort from 1 to `max`.
fn size_orders(sigs: &[String], max: usize) -> Vec<BTreeMap<String, usize>> {
    let mut all = vec![BTreeMap::new()];
    for sig in sigs {
        all = all
            .into_iter()
            .flat_map(|prefix: BTreeMap<String, usize>| {
                (1..=max).map(move |n| {
                    let mut next = prefix.clone();
                    next.insert(sig.clone(), n);
                    next
                })
            })
            .collect();
    }
    all.sort_by_key(|sizes| sizes.values().sum::<usize>());
    all
}

/// The smallest concrete counterexample within `max` atoms per sort.
pub fn concrete(
    model: &rmodel::Model,
    smt: &str,
    params: &[(String, String)],
    max: usize,
) -> Option<Cti> {
    let body = smt.trim_end().strip_suffix("(check-sat)").expect("a VC");
    for sizes in size_orders(&model.sigs, max) {
        let mut script = body.to_owned();
        for sig in &model.sigs {
            let names: Vec<String> = (0..sizes[sig]).map(|i| format!("c_{sig}_{i}")).collect();
            for name in &names {
                script.push_str(&format!("(declare-const {name} S_{sig})\n"));
            }
            if names.len() > 1 {
                script.push_str(&format!("(assert (distinct {}))\n", names.join(" ")));
            }
            let alternatives: Vec<String> = names.iter().map(|n| format!("(= x {n})")).collect();
            script.push_str(&format!(
                "(assert (forall ((x S_{sig})) (or {})))\n",
                alternatives.join(" ")
            ));
        }
        script.push_str("(check-sat)\n");
        let mut queries = Vec::new();
        for relation in &model.relations {
            let copies: &[&str] = if relation.var { &["", "_post"] } else { &[""] };
            for copy in copies {
                for tuple in tuples(model, &sizes, &columns(&relation.shape)) {
                    let args: Vec<String> = tuple.iter().map(|(_, smt)| smt.clone()).collect();
                    let names: Vec<String> = tuple.iter().map(|(name, _)| name.clone()).collect();
                    queries.push((
                        format!("(r_{}{copy} {})", relation.name, args.join(" ")),
                        Some((format!("{}{copy}", relation.name), names)),
                        None,
                    ));
                }
            }
        }
        for (param, sort) in params {
            for (name, smt) in atoms(model, &sizes, sort) {
                queries.push((
                    format!("(= p_{param} {smt})"),
                    None,
                    Some((param.clone(), name)),
                ));
            }
        }
        for (query, _, _) in &queries {
            script.push_str(&format!("(get-value ({query}))\n"));
        }
        let (answer, text) = run(&script, 20);
        if answer != Answer::Sat {
            continue;
        }
        let values: Vec<bool> = text
            .lines()
            .skip(1)
            .map(|line| line.trim_end().ends_with("true))"))
            .collect();
        let mut cti = Cti {
            sizes: sizes.clone(),
            ..Cti::default()
        };
        for relation in &model.relations {
            cti.relations.entry(relation.name.clone()).or_default();
            if relation.var {
                cti.relations
                    .entry(format!("{}_post", relation.name))
                    .or_default();
            }
        }
        for ((_, relation, arg), value) in queries.iter().zip(values) {
            if !value {
                continue;
            }
            if let Some((name, tuple)) = relation {
                cti.relations.get_mut(name).unwrap().push(tuple.clone());
            }
            if let Some((param, atom)) = arg {
                cti.args.insert(param.clone(), atom.clone());
            }
        }
        return Some(cti);
    }
    None
}
