use crate::Engine;
use std::cell::RefCell;
#[derive(Default)]
struct Instance {
    engine: Engine,
    input: Vec<u8>,
    output: Vec<u8>,
}
thread_local! {static INSTANCE:RefCell<Instance>=RefCell::new(Instance::default());}
#[unsafe(no_mangle)]
pub extern "C" fn smt_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn smt_input_resize(length: u32) -> *mut u8 {
    INSTANCE.with_borrow_mut(|value| {
        value.input.resize(length as usize, 0);
        value.input.as_mut_ptr()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn smt_dispatch() {
    INSTANCE.with_borrow_mut(|value| {
        let result = value.engine.dispatch(&value.input);
        value.output = serde_json::to_vec(&result).expect("serializable SMT response");
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn smt_output_ptr() -> *const u8 {
    INSTANCE.with_borrow(|value| value.output.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn smt_output_len() -> u32 {
    INSTANCE.with_borrow(|value| value.output.len() as u32)
}
