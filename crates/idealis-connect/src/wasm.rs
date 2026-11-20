use crate::Engine;
use std::cell::RefCell;
struct Runtime {
    engine: Engine,
    input: Vec<u8>,
    output: Vec<u8>,
}
thread_local! { static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime {engine: Engine::default(), input: Vec::new(), output: Vec::new()}); }
#[unsafe(no_mangle)]
pub extern "C" fn testing_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn testing_input_resize(length: u32) -> *mut u8 {
    RUNTIME.with_borrow_mut(|runtime| {
        runtime.input.resize(length as usize, 0);
        runtime.input.as_mut_ptr()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn testing_dispatch() {
    RUNTIME.with_borrow_mut(|runtime| {
        runtime.output = serde_json::to_vec(&runtime.engine.dispatch(&runtime.input))
            .expect("serializable response");
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn testing_output_ptr() -> *const u8 {
    RUNTIME.with_borrow(|runtime| runtime.output.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn testing_output_len() -> u32 {
    RUNTIME.with_borrow(|runtime| runtime.output.len() as u32)
}
