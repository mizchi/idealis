use roracle_typed_probe::tape::Tape;
use serde_json::json;

fn token(tag: u8, data: &[u8]) -> Vec<u8> {
    let mut bytes = ((data.len() + 5) as u32).to_le_bytes().to_vec();
    bytes.push(tag);
    bytes.extend_from_slice(data);
    bytes
}

#[test]
fn borrowed_values_serialize_and_compare_like_json() {
    for (tag, bytes, expected) in [
        (0, "", json!(null)),
        (1, "", json!(false)),
        (2, "", json!(true)),
        (3, "7", json!(7)),
        (4, "雪😀", json!("雪😀")),
    ] {
        let input = token(tag, bytes.as_bytes());
        let tape = Tape::new(&input).unwrap();
        let value = tape.view(0).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
        assert_eq!(value, value);
        assert!(tape.view(1).is_err());
    }
}

#[test]
fn malformed_payloads_and_nonfinite_numbers_are_errors() {
    for input in [
        vec![],
        vec![0; 5],
        token(9, &[]),
        token(0, &[1]),
        token(3, b"NaN"),
        token(3, b"1e400"),
        token(4, &[255]),
        token(5, &1u32.to_le_bytes()),
    ] {
        assert!(Tape::new(&input).is_err());
    }
}
