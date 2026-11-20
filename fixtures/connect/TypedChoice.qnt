module TypedChoice {
  type Command = Init | Put({ selected: int })
  type Observation = { value: int }

  var command: Command
  var observation: Observation

  action init = all {
    command' = Init,
    observation' = { value: 0 },
  }

  action step = {
    nondet selected = Set(0, 1, 2).oneOf()
    all {
      command' = Put({ selected: selected }),
      observation' = { value: selected },
    }
  }

  val storedSelection = match command {
    | Init => observation.value == 0
    | Put(input) => observation.value == input.selected
  }
}
