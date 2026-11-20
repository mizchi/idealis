use napi_derive::napi;

#[napi]
pub fn abi_version() -> u32 {
    1
}

#[napi]
pub fn dispatch_json(input: String) -> String {
    String::from_utf8(roracle::dispatch_json(input.as_bytes())).expect("UTF-8 JSON response")
}

#[napi(
    ts_args_type = "input: import('../../../../packages/idealis/src/generated/roracle.js').OracleRequest"
)]
pub fn dispatch_object(input: serde_json::Value) -> String {
    roracle::dispatch_value_to_json(input)
}
