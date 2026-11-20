use rtla::ast::{Expression, ExpressionKind as E, Module};
use rtla_compiler::{CompileOptions, IrVersion, compile_with_options};
use rtlc::SafetyBounds;
use serde_json::{Value, json};

const FRONTEND_LIMIT: &str = "expression or operator nesting exceeds 40";
const CHECKER_LIMIT: &str = "Safety expression nesting exceeds 48";
const PARSER_LIMIT: &str = "TLA parsing: Expression nesting exceeds 64";

fn large(body: fn()) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("../../../fixtures/nesting-chains/cases.json")).unwrap()
}

fn named(cases: &[Value], name: &str) -> Value {
    cases
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("case {name}"))
        .clone()
}

fn bare(text: &str) -> String {
    let text = text.strip_prefix("Unsupported TLA input: ").unwrap_or(text);
    match text.rfind(" at ") {
        Some(index)
            if text[index + 4..]
                .split(':')
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())) =>
        {
            text[..index].to_owned()
        }
        _ => text.to_owned(),
    }
}

fn options(version: u8) -> CompileOptions {
    CompileOptions {
        irVersion: if version == 1 {
            IrVersion::V1
        } else {
            IrVersion::V2
        },
    }
}

fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 100,
        max_states: 1000,
        max_work: 1_000_000,
        max_temporal_nodes: None,
    }
}

fn compile(source: &str, config: &str, version: u8) -> Result<rtlc::SafetyProgram, String> {
    let module = rtla::parse_module(source).map_err(|error| format!("TLA parsing: {error}"))?;
    let config = rtla::parse_config(config).map_err(|error| format!("TLA parsing: {error}"))?;
    compile_with_options(&module, &config, options(version)).map_err(|error| error.message)
}

fn checked(program: &rtlc::SafetyProgram) -> Value {
    match program.check(bounds()) {
        Err(error) => json!({"stage": "check", "message": bare(&error.message)}),
        Ok(result) if result.status == "complete" => json!({
            "stage": "result",
            "status": "complete",
            "states": result.states,
            "generated": result.generated,
            "work": result.work,
        }),
        Ok(result) => {
            assert_eq!(result.status, "invariant-violation");
            json!({
                "stage": "result",
                "status": "invariant-violation",
                "trace": result.violation.unwrap().trace.len(),
                "work": result.work,
            })
        }
    }
}

fn outcome(source: &str, config: &str, version: u8) -> (Value, Option<rtlc::SafetyProgram>) {
    match compile(source, config, version) {
        Err(message) => (json!({"stage": "compile", "message": bare(&message)}), None),
        Ok(program) => (checked(&program), Some(program)),
    }
}

fn agrees(actual: &Value, expected: &Value) -> bool {
    if expected["stage"] == "result" && expected["work"].is_null() {
        let mut actual = actual.clone();
        actual["work"] = Value::Null;
        actual == *expected
    } else {
        actual == expected
    }
}

fn wire(program: &rtlc::SafetyProgram) -> String {
    serde_json::to_string(program).unwrap()
}

fn group(name: &str) {
    group_versions(name, &[1, 2]);
}

fn group_versions(name: &str, versions: &[u8]) {
    cases_versions(|case| case["group"] == name, versions);
}

fn cases_versions(selected: impl Fn(&Value) -> bool, versions: &[u8]) {
    let cases = cases();
    let mut failures = vec![];
    for case in cases.iter().filter(|case| selected(case)) {
        let source = case["source"].as_str().unwrap();
        let config = case["config"].as_str().unwrap();
        for &version in versions {
            let expected = &case["expect"]["rust"][format!("v{version}")];
            let (actual, program) = outcome(source, config, version);
            if !agrees(&actual, expected) {
                failures.push(format!(
                    "{} v{version}: expected {expected}, got {actual}",
                    case["name"]
                ));
                continue;
            }
            if version != 2 {
                continue;
            }
            let (Some(program), Some(partner)) = (program, case["equivalent"].as_str()) else {
                continue;
            };
            let other = named(&cases, partner);
            match compile(
                other["source"].as_str().unwrap(),
                other["config"].as_str().unwrap(),
                2,
            ) {
                Ok(other) if wire(&other) == wire(&program) => {}
                Ok(_) => failures.push(format!("{} v2: IR differs from {partner}", case["name"])),
                Err(message) => failures.push(format!("{partner} v2: {message}")),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn chain_lengths_follow_the_expected_layer() {
    large(|| group("length"));
}

#[test]
fn chain_semantics_match_the_equivalent_spelling() {
    large(|| group("semantics"));
}

#[test]
fn chain_positions_match_the_equivalent_spelling() {
    large(|| group("position"));
}

#[test]
fn chain_errors_are_reported_in_lowering_order() {
    large(|| group("order"));
}

#[test]
fn branch_limit_is_reported_before_a_later_undefined_operator() {
    large(|| {
        cases_versions(
            |case| {
                [
                    "limit-next-or-flat-4097",
                    "limit-next-and-chain-13",
                    "limit-next-or-inner-then-missing",
                    "limit-next-and-chain-then-missing",
                ]
                .contains(&case["name"].as_str().unwrap_or_default())
            },
            &[2],
        )
    });
}

#[test]
fn nesting_that_is_not_a_chain_keeps_its_limit() {
    large(|| group("boundary"));
}

fn rust(cases: &[Value], name: &str, version: &str) -> Value {
    named(cases, name)["expect"]["rust"][version].clone()
}

fn complete(states: u64, generated: u64, work: u64) -> Value {
    json!({"stage": "result", "status": "complete", "states": states, "generated": generated, "work": work})
}

fn refused(stage: &str, message: &str) -> Value {
    json!({"stage": stage, "message": message})
}

#[test]
fn a_chain_counts_as_one_level_up_to_the_checker_limit() {
    large(a_chain_counts_as_one_level_up_to_the_checker_limit_body);
}

fn a_chain_counts_as_one_level_up_to_the_checker_limit_body() {
    let cases = cases();
    for form in ["paren-and", "paren-or"] {
        for length in [2, 3, 4, 24, 25, 38, 39, 40, 44, 47, 48] {
            let expected = rust(&cases, &format!("{form}-{length}"), "v2");
            assert_eq!(expected["status"], "complete", "{form}-{length}");
        }
        for length in [49, 54, 61] {
            assert_eq!(
                rust(&cases, &format!("{form}-{length}"), "v2"),
                refused("check", CHECKER_LIMIT),
                "{form}-{length}"
            );
        }
    }
    for length in [2, 3, 4, 24, 25, 38, 39, 40, 44, 47] {
        let expected = rust(&cases, &format!("if-chain-{length}"), "v2");
        assert_eq!(expected["status"], "complete", "if-chain-{length}");
    }
    for length in [48, 49, 54, 61] {
        assert_eq!(
            rust(&cases, &format!("if-chain-{length}"), "v2"),
            refused("check", CHECKER_LIMIT),
            "if-chain-{length}"
        );
    }
    assert_eq!(rust(&cases, "paren-and-44", "v2"), complete(6, 7, 20));
    for form in ["paren-and", "paren-or", "if-chain"] {
        for version in ["v1", "v2"] {
            let expected = rust(&cases, &format!("{form}-62"), version);
            assert!(
                expected["message"]
                    .as_str()
                    .unwrap()
                    .starts_with(PARSER_LIMIT),
                "{form}-62 {version}"
            );
        }
    }
}

#[test]
fn version_one_keeps_counting_every_level() {
    large(version_one_keeps_counting_every_level_body);
}

fn version_one_keeps_counting_every_level_body() {
    let cases = cases();
    for length in [39, 40, 44, 54, 61] {
        assert_eq!(
            rust(&cases, &format!("paren-and-{length}"), "v1"),
            refused("compile", FRONTEND_LIMIT)
        );
    }
    for length in [38, 39, 40, 44, 54, 61] {
        assert_eq!(
            rust(&cases, &format!("if-chain-{length}"), "v1"),
            refused("compile", FRONTEND_LIMIT)
        );
    }
    assert_eq!(
        rust(&cases, "paren-and-25", "v1"),
        refused("check", CHECKER_LIMIT)
    );
}

fn quint(name: &str, property: &str, version: u8) -> (Value, Option<rtlc::SafetyProgram>) {
    let directory = "../../fixtures/nesting-chains/quint";
    let read = |path: String| std::fs::read_to_string(path).unwrap();
    outcome(
        &read(format!("{directory}/{name}.tla")),
        &read(format!("{directory}/{name}-{property}.cfg")),
        version,
    )
}

#[test]
fn quint_conjunction_of_44_terms_is_checked_like_the_flat_form() {
    large(quint_conjunction_of_44_terms_is_checked_like_the_flat_form_body);
}

fn quint_conjunction_of_44_terms_is_checked_like_the_flat_form_body() {
    let (safety, _) = quint("LongConjunction", "safety", 2);
    assert_eq!(safety, complete(6, 7, 20));
    let (broken, _) = quint("LongConjunction", "broken", 2);
    assert_eq!(broken["status"], "invariant-violation");
    assert_eq!(broken["trace"], 5);
    for property in ["safety", "broken"] {
        assert_eq!(
            quint("LongConjunction", property, 1).0,
            refused("compile", FRONTEND_LIMIT)
        );
    }
}

#[test]
fn quint_if_chain_of_54_branches_compiles_and_stops_at_the_checker() {
    large(quint_if_chain_of_54_branches_compiles_and_stops_at_the_checker_body);
}

fn quint_if_chain_of_54_branches_compiles_and_stops_at_the_checker_body() {
    for property in ["safety", "broken"] {
        let (result, program) = quint("LongIfChain", property, 2);
        assert!(program.is_some(), "{property} compiles");
        assert_eq!(result, refused("check", CHECKER_LIMIT));
        assert_eq!(
            quint("LongIfChain", property, 1).0,
            refused("compile", FRONTEND_LIMIT)
        );
    }
}

const DEPTHS: [usize; 3] = [44, 52, 60];

fn terms(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("x /= {}", 200 + index))
        .collect()
}

fn binder_terms(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| {
            if index % 3 == 1 {
                format!(
                    "(\\E k{index} \\in {{0}} : x /= {} + k{index})",
                    200 + index
                )
            } else {
                format!("x /= {}", 200 + index)
            }
        })
        .collect()
}

fn flat_module(position: &str, terms: &[String]) -> String {
    let joined = terms.join(" /\\ ");
    let body = match position {
        "invariant" => format!("Next == x' = IF x < 5 THEN x + 1 ELSE 0\nInv == {joined}\n"),
        "action" | "binders" => {
            format!("Next == (x' = IF x < 5 THEN x + 1 ELSE 0) /\\ {joined}\nInv == x >= 0\n")
        }
        _ => unreachable!(),
    };
    format!("---- MODULE Nested ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\n{body}====\n")
}

fn operands(module: &Module, name: &str) -> Vec<Expression> {
    let definition = module
        .definitions
        .iter()
        .find(|definition| definition.name == name)
        .unwrap();
    match &definition.body.node {
        E::And { operands, .. } => operands.clone(),
        other => panic!("{name} is {other:?}"),
    }
}

fn nested(and: bool, operands: Vec<Expression>, left: bool) -> Expression {
    let location = operands[0].location;
    let wrap = |operands: Vec<Expression>| Expression {
        node: if and {
            E::And {
                operands,
                is_bulleted: false,
            }
        } else {
            E::Or {
                operands,
                is_bulleted: false,
            }
        },
        location,
    };
    let mut remaining = operands.into_iter();
    let first = remaining.next().unwrap();
    if left {
        remaining.fold(first, |inner, operand| wrap(vec![inner, operand]))
    } else {
        let rest: Vec<_> = remaining.collect();
        rest.into_iter()
            .rev()
            .reduce(|inner, operand| wrap(vec![operand, inner]))
            .map_or(first.clone(), |tail| wrap(vec![first, tail]))
    }
}

fn replaced(module: &Module, name: &str, body: Expression) -> Module {
    let mut module = module.clone();
    module
        .definitions
        .iter_mut()
        .find(|definition| definition.name == name)
        .unwrap()
        .body = body;
    module
}

#[test]
fn nesting_deeper_than_the_parser_accepts_is_walked_on_the_sides_the_position_allows() {
    large(nesting_deeper_than_the_parser_accepts_is_walked_on_the_sides_the_position_allows_body);
}

fn nesting_deeper_than_the_parser_accepts_is_walked_on_the_sides_the_position_allows_body() {
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE").unwrap();
    for position in ["invariant", "action", "binders"] {
        for depth in DEPTHS {
            let terms = if position == "binders" {
                binder_terms(depth)
            } else {
                terms(depth)
            };
            let source = flat_module(position, &terms);
            let module = rtla::parse_module(&source).unwrap();
            let name = if position == "invariant" {
                "Inv"
            } else {
                "Next"
            };
            let flat = compile_with_options(&module, &config, options(2))
                .unwrap_or_else(|error| panic!("{position} {depth} flat: {}", error.message));
            let all = operands(&module, name);
            for left in [true, false] {
                let shaped = replaced(&module, name, nested(true, all.clone(), left));
                let compiled = compile_with_options(&shaped, &config, options(2));
                if position == "invariant" && !left {
                    assert_eq!(
                        compiled.err().map(|error| error.message),
                        Some(FRONTEND_LIMIT.to_owned()),
                        "{position} {depth} right-nested"
                    );
                    continue;
                }
                let program = compiled
                    .unwrap_or_else(|error| panic!("{position} {depth} {left}: {}", error.message));
                assert_eq!(
                    wire(&program),
                    wire(&flat),
                    "{position} {depth} left={left}"
                );
            }
        }
    }
}

#[test]
fn disjunction_nested_in_the_action_is_walked_from_either_side() {
    large(disjunction_nested_in_the_action_is_walked_from_either_side_body);
}

fn disjunction_nested_in_the_action_is_walked_from_either_side_body() {
    let disjuncts: Vec<String> = (0..60)
        .map(|index| format!("(x = {index} /\\ x' = {})", index + 1))
        .collect();
    let source = format!(
        "---- MODULE Nested ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == {}\nInv == x >= 0\n====\n",
        disjuncts.join(" \\/ ")
    );
    let module = rtla::parse_module(&source).unwrap();
    let config =
        rtla::parse_config("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE").unwrap();
    let flat = compile_with_options(&module, &config, options(2))
        .unwrap_or_else(|error| panic!("flat disjunction: {}", error.message));
    let definition = module
        .definitions
        .iter()
        .find(|definition| definition.name == "Next")
        .unwrap();
    let E::Or { operands, .. } = &definition.body.node else {
        panic!("Next is not a disjunction");
    };
    for left in [true, false] {
        let shaped = replaced(&module, "Next", nested(false, operands.clone(), left));
        let program = compile_with_options(&shaped, &config, options(2))
            .unwrap_or_else(|error| panic!("disjunction left={left}: {}", error.message));
        assert_eq!(wire(&program), wire(&flat), "left={left}");
        assert_eq!(checked(&program), checked(&flat));
    }
}
