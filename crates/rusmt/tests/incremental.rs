//! Incremental context updates must answer exactly like a full reparse of the
//! accumulated source: same status, model, work/stats and error.
use rusmt::{Assumption, CacheOptions, CheckCache, Problem, Solver};
use serde_json::{Value, json};

/// Reference semantics: rebuild the whole source on every change.
struct Reference {
    source: String,
    frames: Vec<usize>,
}
impl Reference {
    /// Appends without reparsing. Limits only grow with the source, so one
    /// later successful `assert` covers every appended prefix.
    fn append(&mut self, expression: &str) {
        self.source.push_str(&format!("\n(assert {expression})"));
    }
    fn assert(&mut self, expression: &str) -> Result<(), String> {
        let source = format!("{}\n(assert {expression})", self.source);
        Problem::parse(&source).map_err(|error| error.to_string())?;
        self.source = source;
        Ok(())
    }
    fn push(&mut self, count: u32) -> Result<(), String> {
        if count as usize > 256 - self.frames.len() {
            return Err("resource: scope depth exceeds 256".into());
        }
        for _ in 0..count {
            self.frames.push(self.source.len());
        }
        Ok(())
    }
    fn pop(&mut self, count: u32) -> Result<(), String> {
        if count as usize > self.frames.len() {
            return Err("protocol: pop exceeds scope depth".into());
        }
        if count > 0 {
            let level = self.frames.len() - count as usize;
            self.source.truncate(self.frames[level]);
            self.frames.truncate(level);
        }
        Ok(())
    }
    fn check(&self, assumptions: &[Assumption], budget: u32) -> Value {
        let problem = Problem::parse(&self.source).unwrap();
        match problem.solve_assuming(assumptions, budget) {
            Ok(solution) => serde_json::to_value(solution).unwrap(),
            Err(error) => json!(error.to_string()),
        }
    }
}

fn observed(solver: &Solver, assumptions: &[Assumption], budget: u32) -> Value {
    match solver.check_assuming(assumptions, budget) {
        Ok(solution) => serde_json::to_value(solution).unwrap(),
        Err(error) => json!(error.to_string()),
    }
}

const BUILD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DECLARATIONS: &str = "(declare-const x Int)(declare-const y Int)(declare-const z Int)(declare-const p Bool)(declare-const q Bool)";
const EXPRESSIONS: &[&str] = &[
    "(<= (- x y) 2)",
    "(< y z)",
    "(= y 3)",
    "(= z 5)",
    "(= x 4)",
    "(= x (+ y 1))",
    "(and (= y 3) (<= x 9))",
    "(or p (<= x y))",
    "(=> q (> z x))",
    "(= (ite p x y) (+ z 1))",
    "(let ((w (- z 2))) (>= w y))",
    "(xor p q)",
    "(distinct x y z)",
    "(<= (* 2 (- x y)) 5)",
    "(<= (+ x y) 9)",
    "(= (* x y) 2)",
    "(not (= y 3))",
    "false",
    "(+ x 1)",
    "(and)",
    "(< x undeclared)",
];

#[test]
fn incremental_updates_match_full_reparse_on_generated_sequences() {
    let mut state = 0x1234_5678_u64;
    let mut next = |bound: u64| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % bound
    };
    let mut compared = 0;
    let mut hits = 0;
    let mut cache = CheckCache::default();
    for _ in 0..400 {
        let mut solver = Solver::parse(DECLARATIONS).unwrap();
        let mut reference = Reference {
            source: DECLARATIONS.into(),
            frames: vec![],
        };
        for step in 0..24 {
            let label = format!("{:?} step {step}", reference.source);
            match next(6) {
                0 | 1 => {
                    let expression = EXPRESSIONS[next(EXPRESSIONS.len() as u64) as usize];
                    let left = solver.assert(expression).map_err(|error| error.to_string());
                    assert_eq!(left, reference.assert(expression), "{label} {expression}");
                }
                2 => {
                    let count = next(3) as u32;
                    let left = solver.push(count).map_err(|error| error.to_string());
                    assert_eq!(left, reference.push(count), "{label}");
                }
                3 => {
                    let count = next(3) as u32;
                    let left = solver.pop(count).map_err(|error| error.to_string());
                    assert_eq!(left, reference.pop(count), "{label}");
                }
                _ => {
                    let assumptions: Vec<_> = (0..next(3))
                        .map(|_| Assumption {
                            symbol: ["p", "q", "x", "missing"][next(4) as usize].into(),
                            positive: next(2) == 0,
                        })
                        .collect();
                    let budget = [0, 7, 40, 100_000][next(4) as usize];
                    let expected = reference.check(&assumptions, budget);
                    assert_eq!(
                        observed(&solver, &assumptions, budget),
                        expected,
                        "{label} {budget}"
                    );
                    compared += 1;
                    let options = CacheOptions {
                        enabled: true,
                        build_digest: BUILD,
                        max_entries: 4096,
                    };
                    match solver.check_incremental(&assumptions, budget, &mut cache, options) {
                        Err(error) => assert_eq!(json!(error.to_string()), expected),
                        Ok(cached) if cached.stats.hits == 1 => {
                            hits += 1;
                            // A hit must be the answer an unbounded fresh search gives.
                            let mut answer = serde_json::to_value(&cached.result).unwrap();
                            let mut complete = reference.check(&assumptions, 100_000);
                            assert_eq!(
                                answer.as_object_mut().unwrap().remove("stats"),
                                Some(json!({"work":0,"branches":0,"relaxations":0}))
                            );
                            complete.as_object_mut().unwrap().remove("stats");
                            assert_eq!(answer, complete, "{label} cached");
                        }
                        Ok(cached) => assert_eq!(
                            serde_json::to_value(&cached.result).unwrap(),
                            expected,
                            "{label} miss"
                        ),
                    }
                }
            }
        }
        assert_eq!(
            observed(&solver, &[], 100_000),
            reference.check(&[], 100_000)
        );
    }
    assert!(compared > 1_000, "{compared}");
    assert!(hits > 200, "{hits}");
}

#[test]
fn later_constants_rewrite_earlier_assertions_and_pop_restores_them() {
    let mut solver = Solver::parse("(declare-const x Int)(declare-const y Int)").unwrap();
    solver.assert("(<= (- x y) 2)").unwrap();
    let before = serde_json::to_value(solver.check(100_000)).unwrap();
    solver.push(1).unwrap();
    solver.assert("(= y 3)").unwrap();
    solver.assert("(<= (+ x y) 9)").unwrap();
    let inside = serde_json::to_value(solver.check(100_000)).unwrap();
    assert_eq!(inside["values"]["y"], "3");
    solver.pop(1).unwrap();
    assert_eq!(serde_json::to_value(solver.check(100_000)).unwrap(), before);
    assert!(solver.assert("(<= (+ x y) 9)").is_err());
}

#[test]
fn accumulated_source_limits_fail_at_the_same_assertion_as_a_reparse() {
    let long = "v".repeat(4000);
    let token_heavy = format!("(and {})", vec!["(<= x 1)"; 60].join(" "));
    let byte_heavy = format!("(<= |{long}| 1)");
    for (expression, expected) in [
        (token_heavy, "resource: too many tokens"),
        (byte_heavy, "resource: source exceeds 1 MiB"),
    ] {
        let declarations = format!("(declare-const x Int)\n(declare-const |{long}| Int)");
        let mut solver = Solver::parse(&declarations).unwrap();
        let mut reference = Reference {
            source: declarations,
            frames: vec![],
        };
        // Find the failing step on the solver, then replay the last steps
        // against full reparses. The first replayed reparse must succeed, so a
        // solver that fails late is caught as well as one that fails early.
        let mut probe = Solver::parse(&reference.source).unwrap();
        let limit = (0..20_000_usize)
            .find(|_| probe.assert(&expression).is_err())
            .expect("accumulated source must hit a limit");
        let window = limit.saturating_sub(3);
        for _ in 0..window {
            solver.assert(&expression).unwrap();
            reference.append(&expression);
        }
        let mut failed_at = None;
        for step in window..=limit {
            let left = solver
                .assert(&expression)
                .map_err(|error| error.to_string());
            assert_eq!(left, reference.assert(&expression), "step {step}");
            if let Err(message) = left {
                assert_eq!(message, expected);
                failed_at = Some(step);
                break;
            }
        }
        assert!(failed_at.is_some_and(|step| step > 100), "{failed_at:?}");
        assert_eq!(observed(&solver, &[], 0), reference.check(&[], 0));
    }
    // Fill the source up to exactly 1 MiB, then exceed it by one byte.
    let declarations = format!("(declare-const x Int)\n(declare-const |{long}| Int)");
    let mut solver = Solver::parse(&declarations).unwrap();
    let mut reference = Reference {
        source: declarations,
        frames: vec![],
    };
    let gap = |reference: &Reference| 1_048_576 - reference.source.len();
    for (filler, floor) in [
        (format!("(<= |{long}| 1)"), 12_000),
        ("(<= x 1)".into(), 4_000),
    ] {
        while gap(&reference) > floor {
            solver.assert(&filler).unwrap();
            reference.append(&filler);
        }
    }
    // "\n(assert (<= x 1D))" spends 17 bytes besides the digits D.
    let exact = format!("(<= x 1{})", "0".repeat(gap(&reference) - 18));
    for expression in [exact.as_str(), "true"] {
        let expected = reference.assert(expression);
        let actual = solver.assert(expression).map_err(|error| error.to_string());
        assert_eq!(actual, expected, "{}", reference.source.len());
    }
    assert_eq!(
        solver.assert("true").err().unwrap().to_string(),
        "resource: source exceeds 1 MiB"
    );
    assert_eq!(reference.source.len(), 1_048_576);
    assert_eq!(observed(&solver, &[], 0), reference.check(&[], 0));
    let deep = format!("{}true{}", "(not ".repeat(64), ")".repeat(64));
    let error = Solver::parse("").unwrap().assert(&deep).err().unwrap();
    assert_eq!(error.to_string(), "resource: nesting exceeds 64");
}
