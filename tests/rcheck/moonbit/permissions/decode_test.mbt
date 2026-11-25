///|
/// Plan 0105: the generated decoders reject unknown and missing fields and
/// atoms outside the scope, and accept the JSON form rcheck sends.
test "decoders are strict" {
  let good = @json.parse(
    "{\"grants\":{\"User0\":[\"Role0\"],\"User1\":[]},\"reads\":{\"Role0\":[\"Doc1\"],\"Role1\":[]},\"secret\":[\"Doc1\"],\"cleared\":[\"User0\"]}",
  )
  let config = @permissions.decode_config(good)
  assert_true(
    config.reads
    .get(@permissions.Role(0))
    .unwrap()
    .contains(@permissions.Doc(1)),
  )
  for
    bad in [
      "{\"grants\":{\"User0\":[],\"User1\":[]},\"reads\":{\"Role0\":[],\"Role1\":[]},\"secret\":[],\"cleared\":[],\"extra\":[]}",
      "{\"grants\":{\"User0\":[],\"User1\":[]},\"reads\":{\"Role0\":[],\"Role1\":[]},\"secret\":[]}",
      "{\"grants\":{\"User0\":[],\"User1\":[]},\"reads\":{\"Role0\":[],\"Role1\":[]},\"secret\":[\"Doc2\"],\"cleared\":[]}",
      "{\"grants\":{\"User0\":[]},\"reads\":{\"Role0\":[],\"Role1\":[]},\"secret\":[],\"cleared\":[]}",
    ] {
    let rejected = try {
      ignore(@permissions.decode_config(@json.parse(bad)))
      false
    } catch {
      _ => true
    }
    assert_true(rejected, msg="accepted \{bad}")
  }
  ignore(
    @permissions.decode_command(
      @json.parse("{\"kind\":\"read\",\"u\":\"User1\",\"d\":\"Doc0\"}"),
    ),
  )
  for
    bad in [
      "{\"kind\":\"read\",\"u\":\"User1\"}", "{\"kind\":\"read\",\"u\":\"User1\",\"d\":\"Doc0\",\"x\":1}",
      "{\"kind\":\"read\",\"u\":\"User7\",\"d\":\"Doc0\"}", "{\"kind\":\"write\",\"u\":\"User1\",\"d\":\"Doc0\"}",
      "{\"kind\":\"read\",\"u\":\"User01\",\"d\":\"Doc0\"}",
    ] {
    let rejected = try {
      ignore(@permissions.decode_command(@json.parse(bad)))
      false
    } catch {
      _ => true
    }
    assert_true(rejected, msg="accepted \{bad}")
  }
}
