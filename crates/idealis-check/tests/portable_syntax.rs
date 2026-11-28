use idealis_check::{Budget, Coverage, Selection, Verdict};

#[test]
fn sat_options_and_integer_width_do_not_restrict_the_reference_language() {
    let source = "model Members
entity Member
invariant Fits { #Member <= 8 }
scope Member = 1";
    let check = |source, checker: &str| {
        let model = idealis_model::parse(source).unwrap();
        idealis_check::check(
            &model,
            "Fits",
            &Selection::Named(checker.into()),
            Budget::default(),
        )
    };
    assert_eq!(
        check(source, "reference").unwrap().answers[0].verdict,
        Verdict::Holds
    );
    let report = check(source, "sat").unwrap();
    assert_eq!(report.answers[0].verdict, Verdict::Holds);
    assert_eq!(report.answers[0].coverage, Coverage::Bounded { steps: 4 });
    let negative = source.replace("<= 8", ">= -16");
    assert_eq!(
        check(&negative, "sat").unwrap().answers[0].verdict,
        Verdict::Holds
    );
    let wide = source.replace("<= 8", "<= 16");
    assert_eq!(
        check(&wide, "reference").unwrap().answers[0].verdict,
        Verdict::Holds
    );
    assert!(
        check(&wide, "sat")
            .unwrap_err()
            .to_string()
            .contains("integer literals")
    );
    let long = format!("{source}, steps = 40");
    assert_eq!(
        check(&long, "reference").unwrap().answers[0].verdict,
        Verdict::Holds
    );
    assert!(
        check(&long, "sat")
            .unwrap_err()
            .to_string()
            .contains("steps")
    );
    let legacy_ir = idealis_check::ir::to_ir(&idealis_model::parse(&long).unwrap()).unwrap();
    let (decoded, _) =
        idealis_check::ir::parse(&serde_json::to_string(&legacy_ir).unwrap()).unwrap();
    assert_eq!(decoded.scope.steps, Some(40));
    assert!(idealis_model::alloy_supports(&decoded).is_err());
    let no_scope = "model Singleton
enum Choice = Only
invariant Fits { one Choice }";
    assert_eq!(
        check(no_scope, "reference").unwrap().answers[0].verdict,
        Verdict::Holds
    );
    let model = idealis_model::parse(source).unwrap();
    let ir = serde_json::to_value(idealis_check::ir::to_ir(&model).unwrap()).unwrap();
    assert!(ir["scope"].get("steps").is_none());
    assert!(idealis_check::ir::parse(&ir.to_string()).is_ok());
}

#[test]
fn rust_reserves_identifiers_without_renaming_json_keys() {
    let source = "model Naming
data Request = { match: Nat } where { match <= 1 }
var use: lone Request
init { no use }
behavior loop(typeArg: Request) { update { use = typeArg } }
scope Int = 0..1, steps = 1";
    let model = idealis_model::parse(source).unwrap();
    let rust = idealis_model::bindings::rust(&model, source);
    assert!(rust.contains("pub r#match: u64"));
    assert!(rust.contains("to_Nat(&x.r#match)"));
    assert!(rust.contains("i128::from(x.r#match)"));
    assert!(rust.contains("o.get(\"match\")"));
    assert!(rust.contains("pub r#use: Option<Request>"));
    assert!(rust.contains("\"loop\" => Command::r#loop"));
}

#[test]
fn cli_sat_bound_overrides_the_legacy_model_setting() {
    let dir = std::env::temp_dir().join(format!("rcheck-sat-steps-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("count.ideal");
    std::fs::write(
        &path,
        "model Count enum Member = One invariant Fits { #Member <= 8 } scope steps = 40",
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .arg(path)
        .args(["--checker", "sat", "--sat-steps", "2", "--question", "Fits"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report[0]["answers"][0]["coverage"]["steps"], 2);
    assert_eq!(report[0]["answers"][0]["verdict"], "holds");
}
