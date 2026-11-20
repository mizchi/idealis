#![no_main]
use idealis_fuzz_eval::{Mode, exercise};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    exercise(data, Mode::Structured, false);
});
