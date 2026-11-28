package permissions

import (
	"encoding/json"
	"testing"
)

// Plan 0104: the generated decoders reject unknown and missing fields and
// atoms outside the scope, and accept the JSON form rcheck sends.
func TestDecodersAreStrict(t *testing.T) {
	good := `{"grants":{"User0":["Role0"],"User1":[]},"reads":{"Role0":["Doc1"],"Role1":[]},"secret":["Doc1"],"cleared":["User0"]}`
	config, err := DecodeConfig(json.RawMessage(good))
	if err != nil {
		t.Fatalf("a valid config: %v", err)
	}
	if _, ok := config.Reads[Role(0)][Doc(1)]; !ok {
		t.Fatalf("reads not decoded: %+v", config)
	}
	for _, bad := range []string{
		`{"grants":{"User0":[],"User1":[]},"reads":{"Role0":[],"Role1":[]},"secret":[],"cleared":[],"extra":[]}`,
		`{"grants":{"User0":[],"User1":[]},"reads":{"Role0":[],"Role1":[]},"secret":[]}`,
		`{"grants":{"User0":[],"User1":[]},"reads":{"Role0":[],"Role1":[]},"secret":["Doc2"],"cleared":[]}`,
		`{"grants":{"User0":[]},"reads":{"Role0":[],"Role1":[]},"secret":[],"cleared":[]}`,
	} {
		if _, err := DecodeConfig(json.RawMessage(bad)); err == nil {
			t.Errorf("accepted %s", bad)
		}
	}
	if _, err := DecodeCommand(json.RawMessage(`{"kind":"read","u":"User1","d":"Doc0"}`)); err != nil {
		t.Fatalf("a valid command: %v", err)
	}
	for _, bad := range []string{
		`{"kind":"read","u":"User1"}`,
		`{"kind":"read","u":"User1","d":"Doc0","x":1}`,
		`{"kind":"read","u":"User7","d":"Doc0"}`,
		`{"kind":"write","u":"User1","d":"Doc0"}`,
		`{"kind":"read","u":"User01","d":"Doc0"}`,
	} {
		if _, err := DecodeCommand(json.RawMessage(bad)); err == nil {
			t.Errorf("accepted %s", bad)
		}
	}
}
