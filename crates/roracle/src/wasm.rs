use std::cell::RefCell;

#[derive(Default)]
struct Runtime {
    input: Vec<u8>,
    output: Vec<u8>,
}
thread_local! { static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::default()); }
#[unsafe(no_mangle)]
pub extern "C" fn oracle_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn oracle_input_resize(length: u32) -> *mut u8 {
    if length > 16 * 1024 * 1024 {
        RUNTIME.with_borrow_mut(|runtime| runtime.input.clear());
        return std::ptr::null_mut();
    }
    RUNTIME.with_borrow_mut(|runtime| {
        runtime.input.resize(length as usize, 0);
        runtime.input.as_mut_ptr()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn oracle_dispatch() {
    RUNTIME.with_borrow_mut(|runtime| {
        crate::wire::dispatch_into(&runtime.input, &mut runtime.output);
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn oracle_output_ptr() -> *const u8 {
    RUNTIME.with_borrow(|runtime| runtime.output.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn oracle_output_len() -> u32 {
    RUNTIME.with_borrow(|runtime| runtime.output.len() as u32)
}
