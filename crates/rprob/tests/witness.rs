use rmodel::reference::{Data, Outcome, Runtime};

const BARRIER: &str = include_str!("../../../examples/model-patterns/worker-barrier/reuse.model");

#[test]
fn barrier_witness_replays_a_five_step_violation_with_its_arguments() {
    check_barrier(BARRIER);
    check_barrier(
        &BARRIER
            .replace("enum Worker", "sig Padding\nenum Worker")
            .replace("scope steps = 9", "scope Padding = 4, steps = 9"),
    );
}

fn check_barrier(baseline: &str) {
    let source = baseline.replace(
        "if { generation != worker.rounds }",
        "if { generation != R1 }",
    );
    assert_ne!(source, baseline);
    let model = rmodel::parse(&source).unwrap();
    let result = rprob::witness(&model, "CannotPassCurrentRound", 20_000).unwrap();
    assert!(result.complete);
    assert_eq!(result.outcome, Outcome::Violated { depth: 5 });
    let trace = result.witness.unwrap();
    assert_eq!(trace.commands.len(), 5);
    assert_eq!(trace.commands.last().unwrap().action, "tryPass");
    let runtime = Runtime::new(&model);
    let values = |data: &[(String, Vec<Vec<Data>>)], name: &str| {
        data.iter()
            .find(|(key, _)| key == name)
            .map(|(_, tuples)| tuples.clone())
    };
    let config = runtime
        .read_configuration(|rel| values(&trace.config, &rel.name))
        .unwrap();
    let mut state = runtime
        .read_initial(&config, |rel| values(&trace.initial, &rel.name))
        .unwrap();
    for (step, command) in trace.commands.iter().enumerate() {
        let bound = runtime
            .read_command(&command.action, |param, _| {
                command
                    .args
                    .iter()
                    .find(|(name, _)| name == param)
                    .map(|(_, data)| data.clone())
            })
            .unwrap();
        let Some(rmodel::reference::Next::State(next)) = runtime.execute(&config, &state, &bound)
        else {
            panic!("unreplayable step {step}");
        };
        let violated = runtime.violated_command_transition(&config, &state, &next, &bound);
        assert_eq!(violated, (step == 4).then_some("CannotPassCurrentRound"));
        state = next;
    }
    assert_eq!(runtime.state_data(&state), trace.final_state);
    assert!(rprob::witness(&model, "BothRoundsFinish", 20_000).is_err());
    let result = rprob::witness(&model, "CannotPassCurrentRound", 1).unwrap();
    assert!(!result.complete);
    assert_eq!(result.outcome, Outcome::Unknown);
    assert!(result.witness.is_none());
    let normal = rmodel::parse(baseline).unwrap();
    let result = rprob::witness(&normal, "CannotPassCurrentRound", 20_000).unwrap();
    assert_eq!(result.outcome, Outcome::Holds);
    assert!(result.witness.is_none());
}

#[test]
fn later_configurations_can_shorten_an_already_found_path() {
    let model = rmodel::parse(
        "model Configured
         enum Mode = Longer | Shorter
         enum Level = Start | Middle | End
         rel mode: one Mode
         var level: one Level
         init { level = Start }
         behavior step() {
           guard { level = Start }
           update { level = if { mode = Longer } then Middle else End }
         }
         behavior finish() { guard { level = Middle } update { level = End } }
         reachable Reached { level = End }
         scope steps = 3",
    )
    .unwrap();
    let partial = rprob::witness(&model, "Reached", 7).unwrap();
    assert_eq!(partial.outcome, Outcome::Reachable { depth: 2 });
    assert!(!partial.complete);
    assert_eq!(partial.witness.unwrap().commands.len(), 2);
    let complete = rprob::witness(&model, "Reached", 20).unwrap();
    assert_eq!(complete.outcome, Outcome::Reachable { depth: 1 });
    assert!(complete.complete);
    let trace = complete.witness.unwrap();
    assert_eq!(trace.commands.len(), 1);
    assert_eq!(
        trace.config,
        vec![(
            "mode".to_owned(),
            vec![vec![Data::Atom("Shorter".to_owned())]]
        )]
    );
}

#[test]
fn declaration_witness_keeps_all_assignments_from_the_invalid_step() {
    let model = rmodel::parse(
        "model Broken
         enum Entry = A
         var required: one Entry
         var toggled: one Bool
         init { required = A not toggled }
         behavior breakState() { update { required = none toggled = true } }
         scope steps = 2",
    )
    .unwrap();
    let result = rprob::witness(&model, rmodel::DECLARATIONS, 100).unwrap();
    assert_eq!(result.outcome, Outcome::Violated { depth: 1 });
    let path = result.witness.unwrap();
    assert_eq!(path.commands[0].action, "breakState");
    assert_eq!(path.final_state[0].1, Vec::<Vec<Data>>::new());
    assert_eq!(path.final_state[1].1, vec![vec![Data::Atom("true".into())]]);
}

#[test]
fn a_single_word_toggle_keeps_the_selected_atom() {
    let model = rmodel::parse(
        "model Members
         enum Item = A | B
         var picked: set Item
         init { no picked }
         behavior pick(item: Item) {
           guard { item not in picked }
           update { picked = picked + item }
         }
         reachable Found { B in picked }
         scope steps = 2",
    )
    .unwrap();
    let fast = rprob::Compiled::new(&model).unwrap().analyze(100);
    let result = rprob::witness(&model, "Found", 100).unwrap();
    assert_eq!(result.outcome, fast.verdicts[0].outcome);
    assert_eq!(result.outcome, Outcome::Reachable { depth: 1 });
    let trace = result.witness.unwrap();
    assert_eq!(
        trace.commands[0].args,
        vec![("item".into(), Data::Atom("B".into()))]
    );
}
