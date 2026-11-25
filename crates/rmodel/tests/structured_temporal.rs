#[test]
fn fairness_and_temporal_requirements_are_separate_from_reachability() {
    let model = rmodel::parse(
        "model Delivery
         enum Status = Waiting | Delivered
         var status: one Status
         init { status = Waiting }
         behavior deliver() { guard { status = Waiting } update { status = Delivered } }
         reachable CanDeliver { status = Delivered }
         fairness weak deliver
         eventually Delivers { status = Delivered }
         scope steps = 3",
    )
    .unwrap();
    let (source, config) = rmodel::to_tla(&model, "Delivers").unwrap();
    assert!(source.contains("WF_"));
    assert!(config.contains("PROPERTY c_Delivers"));
}

#[test]
fn a_list_preserves_order_and_a_variant_carries_its_payload() {
    let model = rmodel::parse(
        "model Log
         enum Entry = A | B
         sequence Journal = Entry, max = 2
         variant Result = Pending | Added(Entry)
         var log: one Journal
         var result: one Result
         init { log = Journal[] result = Pending() }
         behavior add(e: Entry) {
           guard { len(log) < 2 }
           update { log = append(log, e) result = Added(e) }
         }
         reachable Ordered { log = Journal[A, B] result = Added(B) }
         reachable Repeated { log = Journal[A, A] }
         scope Int = 0..2, steps = 3",
    )
    .unwrap();
    let report = rmodel::reference::analyze(&model, 100_000);
    for verdict in report.verdicts {
        assert_eq!(
            verdict.outcome,
            rmodel::reference::Outcome::Reachable { depth: 2 },
            "{verdict:?}"
        );
    }
}
