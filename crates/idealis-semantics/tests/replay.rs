use idealis_semantics::reference::{Data, Outcome, witness};
use idealis_syntax::parse_located_checked;

#[test]
fn reference_evaluation_uses_the_checked_model_directly() {
    let source = "model Library
entity Copy
var held: lone Copy
init { no held }
behavior borrow(c: Copy) {
  guard { no held }
  update { held = c }
}
reachable Borrowed { some held }
scope Copy = 1";
    let model = parse_located_checked(source).unwrap().model;
    let result = witness(&model, "Borrowed", 100).unwrap();
    assert_eq!(result.outcome, Outcome::Reachable { depth: 1 });
    let trace = result.witness.unwrap();
    assert_eq!(trace.commands[0].action, "borrow");
    assert_eq!(
        trace.commands[0].args,
        vec![("c".into(), Data::Atom("Copy0".into()))]
    );
    assert_eq!(
        trace.final_state,
        vec![("held".into(), vec![vec![Data::Atom("Copy0".into())]])]
    );
}
