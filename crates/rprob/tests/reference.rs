use rmodel::reference::Outcome::{self, Empty, Holds, Reachable, Unknown, Unreachable, Violated};

const PERMISSIONS: &str = include_str!("../../../fixtures/rmodel/permissions.model");
const SHIPMENT: &str = include_str!("../../../fixtures/rprob/shipment.model");

fn agree(source: &str, budgets: &[usize]) -> rmodel::reference::Report {
    let model = rmodel::parse(source).unwrap();
    let compiled = rprob::Compiled::new(&model).unwrap();
    let mut full = None;
    for &budget in budgets {
        let expected = rmodel::reference::analyze(&model, budget);
        let actual = compiled.analyze(budget);
        assert_eq!(actual.verdicts, expected.verdicts, "budget {budget}");
        assert_eq!(actual.scope, expected.scope, "budget {budget}");
        assert_eq!(
            actual.declarations, expected.declarations,
            "budget {budget}"
        );
        assert_eq!(actual.enabled, expected.enabled, "budget {budget}");
        full = Some(actual);
    }
    full.unwrap()
}

fn outcome(report: &rmodel::reference::Report, name: &str) -> Outcome {
    report
        .verdicts
        .iter()
        .find(|verdict| verdict.check == name)
        .unwrap()
        .outcome
}

#[test]
fn finite_relations_match_the_reference() {
    for source in [
        PERMISSIONS,
        include_str!("../../../fixtures/rmodel/permissions-open.model"),
        include_str!("../../../fixtures/rmodel/sessions.model"),
        include_str!("../../../fixtures/rmodel/mutex.model"),
        concat!(
            include_str!("../../../fixtures/rcheck/linked/raft/domain.model"),
            "\n",
            include_str!("../../../fixtures/rcheck/linked/raft/election.model"),
        ),
        include_str!("../../../fixtures/rprob/edges.model"),
        "model Graph
        sig Node
        rel links: Node -> set Node
        var copied: Node -> set Node
        init { no copied }
        behavior close() { update { copied = links.links } }
        invariant Consistent { no copied or all n: Node | n.copied = n.links.links }
        reachable TwoHops { some copied }
        scope Node = 2, steps = 4",
        "model Sum
        sig A
        sig B
        type Party = B + A
        rel input: one Party
        var selected: lone Party
        init { no selected }
        behavior copy() { update { selected = input } }
        reachable FoundA { some selected & A }
        scope A = 1, B = 1, steps = 4",
    ] {
        agree(source, &[0, 1, 15, 97, 1_000_000]);
    }
}

#[test]
fn shipment_separates_reachable_differences_from_absence_and_budget() {
    let budgets = [0, 10, 145, 1_000_000];
    let normal = agree(SHIPMENT, &budgets);
    assert_eq!(outcome(&normal, "KeepsTracking"), Holds);
    assert_eq!(outcome(&normal, "ConfirmCourier"), Reachable { depth: 2 });

    let faulty = SHIPMENT.replace(
        "acknowledged = acknowledged + o }",
        "acknowledged = acknowledged + o tracking = (tracking - o -> Carrier) + o -> Post }",
    );
    let report = agree(&faulty, &budgets);
    assert_eq!(outcome(&report, "KeepsTracking"), Violated { depth: 2 });
    assert_eq!(outcome(&report, "ConfirmCourier"), Unreachable);

    let restricted = SHIPMENT.replace(
        "guard { o in shipped }",
        "guard { o in shipped Post in o.tracking }",
    );
    let report = agree(&restricted, &budgets);
    assert_eq!(outcome(&report, "KeepsTracking"), Holds);
    assert_eq!(outcome(&report, "ConfirmCourier"), Unreachable);
    assert_eq!(outcome(&report, "ConfirmPost"), Reachable { depth: 2 });

    let no_initial = SHIPMENT.replace("no shipped no tracking no acknowledged", "false");
    let report = agree(&no_initial, &budgets);
    assert_eq!(report.scope, Empty);
    assert_eq!(outcome(&report, "KeepsTracking"), Empty);
    let report = agree(SHIPMENT, &[1]);
    assert_eq!(report.scope, Unknown);
    assert_eq!(outcome(&report, "ConfirmCourier"), Unknown);

    let broken = SHIPMENT.replace(
        "tracking = tracking + o -> c",
        "tracking = tracking + o -> Carrier",
    );
    let report = agree(&broken, &budgets);
    assert_eq!(report.declarations, Violated { depth: 1 });
    assert_eq!(report.scope, Holds);
    assert_eq!(outcome(&report, "ConfirmPost"), Unreachable);
}

#[test]
fn unsupported_models_do_not_produce_a_verdict() {
    let value_model = include_str!("../../../fixtures/rcheck/structured/shipped-transition.model");
    let temporal_model = include_str!("../../../fixtures/rcheck/structured/retry-delivery.model");
    let oversized =
        include_str!("../../../fixtures/rprob/edges.model").replace("Left = 3", "Left = 5");
    for source in [value_model, temporal_model, &oversized] {
        let model = rmodel::parse(source).unwrap();
        assert!(rprob::Compiled::new(&model).is_err());
    }
}

#[test]
fn cli_accepts_a_model_and_refuses_an_unsupported_one() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let binary = env!("CARGO_BIN_EXE_rprob");
    let output = std::process::Command::new(binary)
        .arg(directory.join("rprob/shipment.model"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("KeepsTracking: Holds")
            && text.contains("ConfirmCourier: Reachable { depth: 2 }")
    );

    let output = std::process::Command::new(binary)
        .arg(directory.join("rcheck/structured/shipped-transition.model"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unsupported")
    );
}

#[test]
fn versioned_adoption_rejects_old_success_and_accepts_current_evidence() {
    let source = include_str!("../../../spec/models/evidence-adoption.model");
    let report = agree(source, &[0, 75, 100_000]);
    for property in [
        "AdoptedIsCurrent",
        "AdoptionNeedsCurrentReview",
        "AdoptionNeedsCurrentAgreement",
        "AdoptionNeedsCurrentImprovement",
    ] {
        assert_eq!(
            outcome(&report, property),
            Holds,
            "{report_property}",
            report_property = property
        );
    }
    assert_eq!(
        outcome(&report, "OldAgreementArrives"),
        Reachable { depth: 2 }
    );
    assert_eq!(
        outcome(&report, "CurrentEvidenceCanAdopt"),
        Reachable { depth: 5 }
    );
    let mutant = source.replace(
        "current in equivalent current in improved",
        "some equivalent current in improved",
    );
    assert_ne!(mutant, source);
    let report = agree(&mutant, &[100_000]);
    assert_eq!(
        outcome(&report, "AdoptionNeedsCurrentAgreement"),
        Violated { depth: 5 }
    );
}

#[test]
fn comparison_requires_complete_agreement_on_current_results() {
    let source = include_str!("../../../spec/models/checker-comparison.model");
    let report = agree(source, &[0, 300_000]);
    assert_eq!(outcome(&report, "PublishedNeedsCurrentResults"), Holds);
    assert_eq!(outcome(&report, "PublishedNeedsAgreement"), Holds);
    assert_eq!(
        outcome(&report, "OneCompleteResult"),
        Reachable { depth: 2 }
    );
    assert_eq!(outcome(&report, "OldResultsOnly"), Reachable { depth: 3 });
    assert_eq!(
        outcome(&report, "CurrentAgreementPublished"),
        Reachable { depth: 4 }
    );
    let ready = "guard { no published current.completed = Checker and (current.valueA = Checker or no current.valueA) }";
    assert_eq!(source.matches(ready).count(), 1);
    for (weakened, property, depth) in [
        (
            "guard { no published some current.completed and (current.valueA = Checker or no current.valueA) }",
            "PublishedNeedsCurrentResults",
            2,
        ),
        (
            "guard { no published and (current.completed = Checker or First.completed = Checker) and (current.valueA = Checker or no current.valueA) }",
            "PublishedNeedsCurrentResults",
            4,
        ),
        (
            "guard { no published current.completed = Checker }",
            "PublishedNeedsAgreement",
            3,
        ),
    ] {
        let mutant = source.replace(ready, weakened);
        assert_eq!(
            outcome(&agree(&mutant, &[300_000]), property),
            Violated { depth }
        );
    }
}

#[test]
fn goal_improvement_requires_the_pinned_run_stable_samples_and_threshold() {
    let source = include_str!("../../../spec/models/measurement-gate.model");
    let report = agree(source, &[0, 100_000]);
    let policy = "guard { no published confirmed = Prerequisite }";
    assert_eq!(source.matches(policy).count(), 1);
    assert_eq!(
        outcome(&report, "ImprovementPublished"),
        Reachable { depth: 4 }
    );
    assert_eq!(
        outcome(&report, "UnsettledMeasurement"),
        Reachable { depth: 1 }
    );
    for (omitted, check) in [
        ("PinnedRun", "ImprovementNeedsPinnedRun"),
        ("StableSamples", "ImprovementNeedsStableSamples"),
        ("ThresholdMet", "ImprovementNeedsThreshold"),
    ] {
        assert_eq!(outcome(&report, check), Holds);
        let changed = source.replace(
            policy,
            &format!("guard {{ no published and (Prerequisite - {omitted}) in confirmed }}"),
        );
        assert_eq!(
            outcome(&agree(&changed, &[100_000]), check),
            Violated { depth: 3 }
        );
    }
}

#[test]
fn a_wide_total_function_uses_legal_rows_and_keeps_the_budget_boundary() {
    let source = include_str!("../../../fixtures/rprob/wide-function.model");
    let model = rmodel::parse(source).unwrap();
    let compiled = rprob::Compiled::new(&model).expect("six legal choices per row");
    let candidates = 6usize.pow(6);
    let incomplete = compiled.analyze(candidates + 1);
    assert_eq!(incomplete.scope, Unknown);
    assert_eq!(outcome(&incomplete, "Initialized"), Unknown);
    let complete = compiled.analyze(candidates + 2);
    assert_eq!(complete.scope, Holds);
    assert_eq!(outcome(&complete, "Initialized"), Reachable { depth: 0 });
}

#[test]
fn compact_frontier_keeps_distinct_edges_and_checks_known_destinations() {
    let report = agree(
        "model CompactFrontier
         sig Left
         sig Right
         var links: Left -> set Right
         init { no links }
         behavior add(l: Left, r: Right) {
           guard { r not in l.links }
           update { links = links + l -> r }
         }
         behavior repeat() { guard { some links } update { links = links } }
         transition StrictChange { before(links) != after(links) }
         reachable Full { links = Left -> Right }
         scope Left = 2, Right = 3, steps = 8",
        &[0, 1, 65, 66, 68, 128, 129],
    );
    assert_eq!(outcome(&report, "StrictChange"), Violated { depth: 2 });
    assert_eq!(outcome(&report, "Full"), Reachable { depth: 6 });
}

#[test]
fn frontier_modes_match_at_the_budget_boundary() {
    let function = include_str!("../../../fixtures/rprob/function-choice.model");
    assert_eq!(
        outcome(&agree(function, &[257, 258, 1024, 1025]), "Initialized"),
        Reachable { depth: 0 },
    );
    let split =
        include_str!("../../../fixtures/rprob/split-sets.model").replace("Item = 7", "Item = 3");
    assert_eq!(
        outcome(&agree(&split, &[65, 66, 128, 129]), "Full"),
        Reachable { depth: 6 },
    );
}

#[test]
fn cached_guards_require_a_reached_state_to_count_as_enabled() {
    let source = "model Guard
        enum Flag = Only
        rel setting: one Flag
        var selected: lone Flag
        init { selected = none }
        behavior turnOn() { guard { Only in setting } update { selected = Flag } }
        behavior disabled() { guard { setting = none } update { selected = none } }
        reachable On { selected = Flag }
        scope steps = 3";
    let limited = agree(source, &[3]);
    assert_eq!(limited.scope, Unknown);
    assert_eq!(
        limited.enabled,
        [("turnOn".into(), None), ("disabled".into(), None)]
    );
    let complete = agree(source, &[4, 5]);
    assert_eq!(
        complete.enabled,
        [
            ("turnOn".into(), Some(true)),
            ("disabled".into(), Some(false))
        ]
    );
    assert_eq!(outcome(&complete, "On"), Reachable { depth: 1 });
}

#[test]
fn full_updates_read_the_old_state_and_partial_updates_preserve_other_slots() {
    let report = agree(
        "model ParallelUpdates
         enum Token = A | B
         var left: one Token
         var right: one Token
         init { left = A right = B }
         behavior swap() {
           update { left = right right = left }
         }
         behavior restoreLeft() {
           guard { left = B and right = A }
           update { left = A }
         }
         reachable Swapped { left = B and right = A }
         reachable LeftRestored { left = A and right = A }
         invariant NoBothB { not (left = B and right = B) }
         scope steps = 4",
        &[5, 6, 7, 8],
    );
    assert_eq!(outcome(&report, "Swapped"), Reachable { depth: 1 });
    assert_eq!(outcome(&report, "LeftRestored"), Reachable { depth: 2 });
    assert_eq!(outcome(&report, "NoBothB"), Holds);
}

#[test]
fn prepaid_initial_candidates_preserve_empty_and_partial_reports() {
    let source = "model InitialBudget
        enum Token = A | B
        rel setting: lone Token
        var left: set Token
        var right: one Token
        fact { some setting }
        init { left = none right = B setting = A }
        reachable Initialized { left = none and right = B }
        scope steps = 3";
    let budgets = [0, 1, 2, 9, 10, 11, 12, 19, 20, 22, 23, 24];
    let report = agree(source, &budgets);
    assert_eq!(report.scope, Holds);
    assert_eq!(outcome(&report, "Initialized"), Reachable { depth: 0 });
    assert_eq!(
        agree(
            &source.replace("right = B setting", "no right setting"),
            &budgets
        )
        .scope,
        Empty
    );
    assert_eq!(
        agree(
            &source.replace("left = none right", "left in Token right"),
            &budgets
        )
        .scope,
        Holds
    );
    let no_state = "model NoState
        enum Token = A
        reachable Initialized { some Token }
        scope steps = 2";
    assert_eq!(
        outcome(&agree(no_state, &[0, 1, 2, 3]), "Initialized"),
        Reachable { depth: 0 }
    );
}

#[test]
fn an_overflowing_candidate_product_only_blocks_accepted_configurations() {
    let source = include_str!("../../../fixtures/rprob/candidates-large.model");
    let run = |source: &str| {
        rprob::Compiled::new(&rmodel::parse(source).unwrap())
            .unwrap()
            .analyze(usize::MAX)
    };
    assert_eq!(run(source).scope, Unknown);
    assert_eq!(
        run(&source.replace("init {", "fact { false }\ninit {")).scope,
        Empty
    );
}

#[test]
fn grouped_bit_updates_keep_the_original_action_order() {
    let source = "model ToggleOrder
        sig Row
        enum A = X | Y
        enum B = Z
        type Column = A + B
        var links: Row -> set Column
        init { no links }
        behavior add(r: Row, c: Column) {
          guard { c not in r.links }
          update { links = links + r -> c }
        }
        behavior remove(r: Row, c: Column) {
          guard { c in r.links }
          update { links = links - r -> c }
        }
        transition KeepsX { Row -> X in before(links) implies Row -> X in after(links) }
        reachable HasX { Row -> X in links }
        reachable HasZ { Row -> Z in links }
        scope Row = 1, steps = 4";
    for model in [source.to_owned(), source.replace("A + B", "B + A")] {
        let report = agree(&model, &[0, 8, 9, 10, 11, 12, 13, 17]);
        assert_eq!(report.scope, Holds);
        assert_eq!(outcome(&report, "KeepsX"), Violated { depth: 2 });
    }
}
