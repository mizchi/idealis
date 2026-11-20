use idealis_fuzz_eval::{Mode, exercise, known_fault};

#[test]
fn structured_feedback_reaches_shrink_and_confirmation() {
    let report = exercise(&[1, 0, 1, 4, 8, 15], Mode::Structured, true);
    assert!(report.phases.contains_key("shrink"));
    assert!(report.phases.contains_key("confirmation"));
    assert!(report.results.contains_key("failed"));
    assert!(report.engine_calls <= 256);
}

#[test]
fn malformed_raw_inputs_are_rejected_without_losing_the_session() {
    let input = b"{\"version\":1,\"request\":{\"kind\":\"replay\",\"case\":{\"format\":\"idealis.test-case\",\"version\":1,\"rng\":\"xorshift32-v1\",\"generatorId\":\"g\",\"generatorVersion\":\"1\",\"seed\":1,\"index\":0,\"actions\":[\"bad\"]}}}\nnot-json\n";
    let report = exercise(input, Mode::Raw, true);
    assert!(report.errors > 0);
    assert!(report.results.contains_key("failed"));
    assert!(report.trace.len() > 2);
}

#[test]
#[should_panic(expected = "protocol state differs")]
fn audit_detects_a_known_pending_session_reset() {
    known_fault();
}
