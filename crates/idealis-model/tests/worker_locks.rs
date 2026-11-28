use idealis_model::reference::{Machine, Next, Outcome, evaluate};
use idealis_model::{CheckKind, Formula};
use std::collections::{BTreeSet, VecDeque};

#[test]
fn deadlocked_reachability_matches_absence_of_an_enabled_worker_action() {
    let base = include_str!("../../../examples/model-patterns/worker-locks/locks.ideal");
    for (order, expected) in [
        (
            include_str!("../../../examples/model-patterns/worker-locks/ordered.ideal"),
            Outcome::Unreachable,
        ),
        (
            include_str!("../../../examples/model-patterns/worker-locks/reversed.ideal"),
            Outcome::Reachable { depth: 2 },
        ),
    ] {
        let mut model = idealis_model::parse(&format!("{base}\n{order}")).unwrap();
        let answers = evaluate(&model, 20_000);
        let outcome = |name| {
            answers
                .iter()
                .find(|check| check.check == name)
                .unwrap()
                .outcome
        };
        assert_eq!(outcome("Deadlocked"), expected);
        assert_eq!(outcome("BothFinished"), Outcome::Reachable { depth: 6 });

        // Negate the reachability predicate so we can query it in each state.
        let query = model
            .checks
            .iter_mut()
            .find(|query| query.name == "Deadlocked")
            .unwrap();
        query.kind = CheckKind::Invariant;
        query.formula = Formula::Not(Box::new(query.formula.clone()));
        let machine = Machine::new(&model).unwrap();
        let configs: Vec<_> = machine.configurations().collect();
        assert_eq!(configs.len(), 1);
        let config = &configs[0];
        let initial = machine.initial(config);
        assert_eq!(initial.len(), 1);
        let mut queue = VecDeque::from(initial);
        let mut visited = BTreeSet::new();
        let mut deadlocks = 0;
        let mut terminals = 0;
        while let Some(state) = queue.pop_front() {
            if !visited.insert(state.clone()) {
                continue;
            }
            let mut enabled = 0;
            for step in machine.steps(config, &state) {
                if let Some(next) = step.next {
                    let Next::State(next) = next else {
                        panic!("undeclared state from {}", step.action);
                    };
                    enabled += 1;
                    queue.push_back(next);
                }
            }
            let data = machine.state_data(&state);
            let finished = data
                .iter()
                .find(|(name, _)| name == "finished")
                .unwrap()
                .1
                .len()
                == 2;
            let blocked = !finished && enabled == 0;
            assert_eq!(
                machine.violated_invariant(config, &state),
                blocked.then_some("Deadlocked"),
                "{data:?}",
            );
            deadlocks += usize::from(blocked);
            terminals += usize::from(finished && enabled == 0);
        }
        assert_eq!(
            deadlocks,
            usize::from(matches!(expected, Outcome::Reachable { .. }))
        );
        assert_eq!(terminals, 1);
    }
}

#[test]
fn tla_groups_the_cartesian_products_in_the_reversed_order() {
    let model = idealis_model::parse(concat!(
        include_str!("../../../examples/model-patterns/worker-locks/locks.ideal"),
        "\n",
        include_str!("../../../examples/model-patterns/worker-locks/reversed.ideal"),
    ))
    .unwrap();
    let (source, _) = idealis_model::to_tla(&model, "Deadlocked").unwrap();
    let init = source
        .lines()
        .find(|line| line.starts_with("Init =="))
        .unwrap();
    // SANY rejects adjacency of prefix UNION and infix \cup.
    assert!(init.contains("r_first = ((UNION "), "{init}");
}
