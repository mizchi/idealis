use rcheck::conform::{Budget, Status};

#[rustfmt::skip]
#[path = "generated/withdrawal.rs"]
mod model;
use model::{Amount, Config, State, Withdrawal, WithdrawalResult};

struct Account(State);

impl Withdrawal for Account {
    fn new(_: &Config) -> Self {
        Self(State {
            balance: Amount { units: 2 },
            beforeBalance: Amount { units: 2 },
            requested: Amount { units: 0 },
            result: WithdrawalResult::Idle,
        })
    }

    fn state(&self) -> State {
        self.0.clone()
    }

    fn requestWithdrawal(&mut self, amount: Amount) -> bool {
        if self.0.result == WithdrawalResult::Pending || amount.units <= 0 {
            return false;
        }
        self.0.requested = amount;
        self.0.result = WithdrawalResult::Pending;
        true
    }

    fn withdraw(&mut self) -> bool {
        if self.0.result != WithdrawalResult::Pending {
            return false;
        }
        let remaining = self.0.balance.units - self.0.requested.units;
        if remaining < 0 {
            return false;
        }
        self.0.beforeBalance = self.0.balance;
        self.0.balance = Amount { units: remaining };
        self.0.result = WithdrawalResult::Withdrawn;
        true
    }

    fn rejectWithdrawal(&mut self) -> bool {
        if self.0.result != WithdrawalResult::Pending
            || self.0.balance.units >= self.0.requested.units
        {
            return false;
        }
        self.0.beforeBalance = self.0.balance;
        self.0.result = WithdrawalResult::InsufficientFunds;
        true
    }
}

#[test]
fn the_rust_domain_conforms_to_the_linked_model() {
    let report = model::conformance::<Account>(Budget::default());
    assert_eq!(report.status, Status::Matched, "{report:?}");
}

#[test]
fn the_rust_domain_replays_the_same_boundary_snapshots_as_typescript() {
    use rcheck::conform::Driver;

    let spec = rmodel::parse(model::MODEL).unwrap();
    let corpus = rcheck::conform::boundary_cases(&spec, rcheck::Budget::default().states).unwrap();
    for pair in corpus["pairs"].as_array().unwrap() {
        for example in pair["examples"].as_array().unwrap() {
            let mut adapter = model::Driver::<Account>(None);
            adapter.reset(&pair["config"]).unwrap();
            for earlier in example["prefix"].as_array().unwrap() {
                assert!(adapter.apply(earlier).unwrap().0, "{earlier}");
            }
            let (accepted, state) = adapter.apply(&example["command"]).unwrap();
            assert_eq!(accepted, example["accepted"], "{example}");
            assert_eq!(state, example["state"], "{example}");
        }
    }
}
