use rcheck::conform::Driver;
use serde_json::{Value, json};

#[rustfmt::skip]
#[path = "generated/shipment.rs"]
mod model;
use model::{Carrier, Config, Shipment, State, Status};

struct Order<const FAULTY: bool>(State);

impl<const FAULTY: bool> Shipment for Order<FAULTY> {
    fn new(_: &Config) -> Self {
        Self(State {
            status: Status::Ready,
            acknowledged: false,
        })
    }

    fn state(&self) -> State {
        self.0.clone()
    }

    fn ship(&mut self, method: Carrier) -> bool {
        if self.0.status != Status::Ready {
            return false;
        }
        self.0.status = Status::Shipped(method);
        true
    }

    fn acknowledge(&mut self) -> bool {
        if !matches!(self.0.status, Status::Shipped(_)) {
            return false;
        }
        if FAULTY {
            self.0.status = Status::Shipped(Carrier::Post);
        }
        self.0.acknowledged = true;
        true
    }
}

const COURIER: &str = include_str!("../../../fixtures/rcheck/shipment/courier.itf.json");
const POST: &str = include_str!("../../../fixtures/rcheck/shipment/post.itf.json");

fn replay<const FAULTY: bool>(source: &str) -> Option<(usize, Value)> {
    let trace: Value = serde_json::from_str(source).unwrap();
    let frames = trace["states"].as_array().unwrap();
    assert_eq!(trace["#meta"]["format"], "ITF");
    assert_eq!(trace["vars"], json!(["config", "command", "observation"]));
    let [initial, operations @ ..] = frames.as_slice() else {
        panic!("missing initial frame");
    };
    assert!(!operations.is_empty());
    assert_eq!(initial["#meta"]["index"], 0);
    assert_eq!(initial["command"]["kind"], "init");

    let mut driver = model::Driver::<Order<FAULTY>>(None);
    assert_eq!(
        driver.reset(&initial["config"]).unwrap(),
        initial["observation"]
    );
    for (offset, frame) in operations.iter().enumerate() {
        let index = offset + 1;
        assert_eq!(frame["#meta"]["index"], index);
        assert_eq!(frame["config"], initial["config"]);
        let (accepted, observed) = driver.apply(&frame["command"]).unwrap();
        assert!(accepted, "operation {index}: {}", frame["command"]);
        if observed != frame["observation"] {
            return Some((index, observed));
        }
    }
    None
}

#[test]
fn shipment_replays_the_common_itf_and_detects_the_first_changed_snapshot() {
    for source in [COURIER, POST] {
        assert_eq!(replay::<false>(source), None);
    }
    assert_eq!(replay::<true>(POST), None);
    assert_eq!(
        replay::<true>(COURIER),
        Some((
            2,
            json!({"acknowledged":true,"status":{"tag":"Shipped","value":"Post"}})
        ))
    );
}
