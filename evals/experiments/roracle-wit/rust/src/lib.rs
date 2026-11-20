wit_bindgen::generate!({ path: "wit", world: "oracle" });
pub mod batch;
pub mod buffer;
#[cfg(feature = "component")]
mod component;
pub mod tape;
#[cfg(feature = "component")]
use component::Component;
#[cfg(feature = "component")]
export!(Component);

#[cfg(feature = "node")]
#[napi_derive::napi]
pub fn check_buffer(data: napi::bindgen_prelude::BufferSlice<'_>) -> String {
    buffer::dispatch(data.as_ref())
}
