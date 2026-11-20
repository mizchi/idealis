mod bindings;
use bindings::*;
use num_bigint::BigInt;
struct Store(BigInt);
impl Driver for Store {
    type Error = std::convert::Infallible;
    fn handle_init(&mut self) -> Result<(), Self::Error> {
        self.0 = 0.into();
        Ok(())
    }
    fn handle_put(&mut self, input: PutInput) -> Result<(), Self::Error> {
        self.0 = input.selected;
        Ok(())
    }
    fn observe(&self) -> Result<Observation, Self::Error> {
        Ok(Observation {
            value: self.0.clone(),
        })
    }
}
fn main() {
    let mut store = Store(0.into());
    Command::Init.apply(&mut store).unwrap();
    let selected: BigInt = BigInt::from(1u8) << 100;
    Command::Put(PutInput {
        selected: selected.clone(),
    })
    .apply(&mut store)
    .unwrap();
    assert_eq!(store.observe().unwrap().value, selected);
}
