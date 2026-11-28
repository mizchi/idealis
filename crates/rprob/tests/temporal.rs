use idealis_model::reference::{Data, Outcome};
use rprob::TemporalOutcome;

#[test]
fn strong_fairness_can_leave_a_smaller_cycle_where_progress_is_disabled() {
    let model = idealis_model::parse(
        "model OptionalService
enum Phase = Ready | Paused | Done
var phase: one Phase
init { phase = Ready }
behavior pause() { guard { phase = Ready } update { phase = Paused } }
behavior resume() { guard { phase = Paused } update { phase = Ready } }
behavior finish() { guard { phase = Ready } update { phase = Done } }
behavior poll() {}
eventually Entered { phase = Ready }
leads_to Finished with { strong finish weak poll }
  { phase = Ready } { phase = Done }
scope steps = 6",
    )
    .unwrap();
    assert!(matches!(
        rprob::temporal_witness(&model, "Entered", 100).unwrap(),
        TemporalOutcome::Holds
    ));
    let TemporalOutcome::Violated(trace) =
        rprob::temporal_witness(&model, "Finished", 100).unwrap()
    else {
        panic!("paused states can stutter without enabling finish");
    };
    assert_eq!(trace.loop_start, 1);
    assert_eq!(trace.stutters, [false, true]);
    assert_eq!(trace.witness.commands[0].action, "pause");
    assert_eq!(
        trace.witness.final_state,
        vec![("phase".into(), vec![vec![Data::Atom("Paused".into())]])]
    );
}

#[test]
fn empty_and_malformed_state_graphs_are_distinguished() {
    let source = "model BrokenGraph
enum Phase = Ready | Done
var phase: one Phase
init { phase = Ready }
behavior drop() { update { phase = none } }
eventually Finished { phase = Done }
scope steps = 4";
    let model = idealis_model::parse(source).unwrap();
    assert!(matches!(
        rprob::temporal_witness(&model, "Finished", 100).unwrap(),
        TemporalOutcome::Unknown {
            declarations: Outcome::Violated { depth: 1 },
            ..
        }
    ));
    let empty =
        idealis_model::parse(&source.replace("phase = Ready", "phase = Ready phase = Done"))
            .unwrap();
    assert!(matches!(
        rprob::temporal_witness(&empty, "Finished", 100).unwrap(),
        TemporalOutcome::Empty
    ));
}
