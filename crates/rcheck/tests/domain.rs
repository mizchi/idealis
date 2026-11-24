//! Plan 0102: Rust implementations of the corpus models, written against
//! the generated traits, conform to the reference semantics; three faults
//! are found with the shortest command sequence; the generated bindings
//! are fresh.
use rcheck::conform::{Budget, Status};
use std::collections::{BTreeMap, BTreeSet};

#[rustfmt::skip]
#[path = "generated/permissions.rs"]
mod permissions;
#[rustfmt::skip]
#[path = "generated/sessions.rs"]
mod sessions;
#[rustfmt::skip]
#[path = "generated/bank.rs"]
mod bank;

/// `FAULT`: 0 none, 1 `read` does not check the grant, 2 `close` leaves
/// the document open.
struct Permissions<const FAULT: u8> {
    config: permissions::Config,
    open: BTreeMap<permissions::User, BTreeSet<permissions::Doc>>,
}

impl<const FAULT: u8> permissions::Permissions for Permissions<FAULT> {
    fn new(config: &permissions::Config) -> Self {
        Self {
            config: config.clone(),
            open: BTreeMap::new(),
        }
    }
    fn state(&self) -> permissions::State {
        permissions::State {
            open: self.open.clone(),
        }
    }
    fn read(&mut self, u: permissions::User, d: permissions::Doc) -> bool {
        let granted = self
            .config
            .grants
            .get(&u)
            .into_iter()
            .flatten()
            .any(|role| {
                self.config
                    .reads
                    .get(role)
                    .is_some_and(|docs| docs.contains(&d))
            });
        if !granted && FAULT != 1 {
            return false;
        }
        self.open.entry(u).or_default().insert(d);
        true
    }
    fn close(&mut self, u: permissions::User, d: permissions::Doc) -> bool {
        let Some(docs) = self.open.get_mut(&u) else {
            return false;
        };
        if !docs.contains(&d) {
            return false;
        }
        if FAULT != 2 {
            docs.remove(&d);
        }
        true
    }
}

/// `FAULT`: 3 `login` does not check the capacity.
struct Sessions<const FAULT: u8> {
    admins: BTreeSet<sessions::User>,
    active: BTreeSet<sessions::User>,
    locked: Option<sessions::User>,
}

impl<const FAULT: u8> sessions::Sessions for Sessions<FAULT> {
    fn new(config: &sessions::Config) -> Self {
        Self {
            admins: config.admins.clone(),
            active: BTreeSet::new(),
            locked: None,
        }
    }
    fn state(&self) -> sessions::State {
        sessions::State {
            active: self.active.clone(),
            locked: self.locked,
        }
    }
    fn login(&mut self, u: sessions::User) -> bool {
        if self.active.contains(&u) || (self.active.len() >= 2 && FAULT != 3) {
            return false;
        }
        self.active.insert(u);
        true
    }
    fn logout(&mut self, u: sessions::User) -> bool {
        self.active.remove(&u)
    }
    fn lock(&mut self, u: sessions::User) -> bool {
        if self.locked.is_some() || !self.admins.contains(&u) || !self.active.contains(&u) {
            return false;
        }
        self.locked = Some(u);
        true
    }
    fn unlock(&mut self, u: sessions::User) -> bool {
        if self.locked != Some(u) {
            return false;
        }
        self.locked = None;
        true
    }
}

struct Bank {
    balance: BTreeMap<bank::Account, i64>,
}

impl bank::Bank for Bank {
    fn new(_: &bank::Config) -> Self {
        Self {
            balance: bank::Account::ALL.iter().map(|a| (*a, 0)).collect(),
        }
    }
    fn state(&self) -> bank::State {
        bank::State {
            balance: self.balance.clone(),
        }
    }
    fn deposit(&mut self, a: bank::Account, n: i64) -> bool {
        let balance = self.balance[&a];
        if n <= 0 || balance + n > 4 {
            return false;
        }
        self.balance.insert(a, balance + n);
        true
    }
    fn withdraw(&mut self, a: bank::Account, n: i64) -> bool {
        let balance = self.balance[&a];
        if n <= 0 || balance < n {
            return false;
        }
        self.balance.insert(a, balance - n);
        true
    }
}

#[test]
fn correct_implementations_conform() {
    for (name, report) in [
        (
            "permissions",
            permissions::conformance::<Permissions<0>>(Budget::default()),
        ),
        (
            "sessions",
            sessions::conformance::<Sessions<0>>(Budget::default()),
        ),
        ("bank", bank::conformance::<Bank>(Budget::default())),
    ] {
        assert_eq!(report.status, Status::Matched, "{name}: {report:?}");
        assert!(
            report.configurations > 0 && report.transitions > 0,
            "{name}: {report:?}"
        );
        println!(
            "{name}: {} configurations, {} states, {} transitions, {} calls",
            report.configurations, report.states, report.transitions, report.calls
        );
    }
}

#[test]
fn faults_are_found_with_the_shortest_path() {
    for (name, report, length) in [
        (
            "read without grant",
            permissions::conformance::<Permissions<1>>(Budget::default()),
            1,
        ),
        (
            "close keeps open",
            permissions::conformance::<Permissions<2>>(Budget::default()),
            2,
        ),
        (
            "login over capacity",
            sessions::conformance::<Sessions<3>>(Budget::default()),
            3,
        ),
    ] {
        assert_eq!(report.status, Status::Mismatch, "{name}: {report:?}");
        let failure = report.failure.expect("a failure");
        assert_eq!(failure.path.len(), length, "{name}: {failure:?}");
        println!("{name}: {} after {:?}", failure.reason, failure.path);
    }
}

#[test]
fn a_bounded_check_says_so() {
    let report = permissions::conformance::<Permissions<0>>(Budget {
        configurations: 3,
        ..Budget::default()
    });
    assert_eq!(report.status, Status::Bounded, "{report:?}");
    let report = permissions::conformance::<Permissions<0>>(Budget {
        calls: 50,
        ..Budget::default()
    });
    assert_eq!(report.status, Status::Unknown, "{report:?}");
}

#[test]
fn generated_bindings_are_fresh() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (model, name) in [
        ("fixtures/rmodel/permissions.model", "permissions"),
        ("fixtures/rmodel/sessions.model", "sessions"),
        ("fixtures/rcheck/bank.model", "bank"),
    ] {
        let source = std::fs::read_to_string(root.join(model)).unwrap();
        let parsed = rmodel::parse(&source).unwrap();
        let schema: serde_json::Value =
            serde_json::from_str(&rmodel::bindings::json_schema(&parsed)).unwrap();
        for (path, text) in [
            (
                format!("crates/rcheck/tests/generated/{name}.rs"),
                rmodel::bindings::rust(&parsed, &source),
            ),
            (
                format!("tests/rcheck/generated/{name}.ts"),
                rmodel::bindings::typescript(&parsed),
            ),
            (
                format!("tests/rcheck/go/{name}/bindings.go"),
                rmodel::bindings::go(&parsed, name),
            ),
            (
                format!("fixtures/rcheck/{name}.schema.json"),
                format!("{}\n", serde_json::to_string_pretty(&schema).unwrap()),
            ),
            (
                format!("fixtures/rcheck/{name}.states.json"),
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(
                        &rcheck::conform::reachable_states(&parsed).unwrap()
                    )
                    .unwrap()
                ),
            ),
        ] {
            let stored = std::fs::read_to_string(root.join(&path)).unwrap();
            assert!(stored == text, "{path} is stale: run rcheck generate");
        }
    }
}

/// The schema accepts every state the model reaches and rejects the three
/// states that break a multiplicity. tests/rcheck/schema.test.ts checks the
/// same files with ajv.
#[test]
fn the_schema_accepts_reachable_states_and_rejects_broken_ones() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |path: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(root.join(path)).unwrap()).unwrap()
    };
    for name in ["permissions", "sessions", "bank"] {
        let schema = read(&format!("fixtures/rcheck/{name}.schema.json"));
        for state in read(&format!("fixtures/rcheck/{name}.states.json"))
            .as_array()
            .unwrap()
        {
            rcheck::schema::validate(&schema, "State", state)
                .unwrap_or_else(|error| panic!("{name}: {error}: {state}"));
        }
    }
    let broken = read("fixtures/rcheck/broken.json");
    assert_eq!(broken.as_array().unwrap().len(), 3);
    for case in broken.as_array().unwrap() {
        let schema = read(&format!(
            "fixtures/rcheck/{}.schema.json",
            case["model"].as_str().unwrap()
        ));
        assert!(
            rcheck::schema::validate(&schema, "State", &case["state"]).is_err(),
            "{}",
            case["why"]
        );
    }
}
