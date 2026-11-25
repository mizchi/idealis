package shipment

import (
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

type order struct {
	state     State
	overwrite bool
}

var _ Shipment = (*order)(nil)

func (o *order) State() State { return o.state }

func (o *order) Ship(method Carrier) bool {
	if _, ready := o.state.Status.(StatusReady); !ready {
		return false
	}
	o.state.Status = StatusShipped{Value: method}
	return true
}

func (o *order) Acknowledge() bool {
	if _, shipped := o.state.Status.(StatusShipped); !shipped {
		return false
	}
	if o.overwrite {
		o.state.Status = StatusShipped{Value: CarrierPost}
	}
	o.state.Acknowledged = true
	return true
}

type frame struct {
	Meta        struct{ Index int } `json:"#meta"`
	Config      json.RawMessage
	Command     json.RawMessage
	Observation json.RawMessage
}

func load(t testing.TB, method string) []frame {
	t.Helper()
	source, err := os.ReadFile("../../../../fixtures/rcheck/shipment/" + method + ".itf.json")
	if err != nil {
		t.Fatal(err)
	}
	var trace struct {
		Meta   struct{ Format string } `json:"#meta"`
		Vars   []string
		States []frame
	}
	if err := json.Unmarshal(source, &trace); err != nil {
		t.Fatal(err)
	}
	if trace.Meta.Format != "ITF" || !reflect.DeepEqual(trace.Vars, []string{"config", "command", "observation"}) || len(trace.States) < 2 {
		t.Fatal("invalid shipment trace")
	}
	return trace.States
}

type mismatch struct {
	step   int
	actual State
}

func replay(t testing.TB, frames []frame, overwrite bool) *mismatch {
	t.Helper()
	domain := &order{state: State{Status: StatusReady{}}, overwrite: overwrite}
	for step, frame := range frames {
		if frame.Meta.Index != step {
			t.Fatalf("incorrect frame index %d", frame.Meta.Index)
		}
		if _, err := DecodeConfig(frame.Config); err != nil {
			t.Fatal(err)
		}
		if step == 0 {
			var initial struct{ Kind string }
			if err := json.Unmarshal(frame.Command, &initial); err != nil || initial.Kind != "init" {
				t.Fatalf("invalid initial command: %s", frame.Command)
			}
		} else {
			command, err := DecodeCommand(frame.Command)
			if err != nil {
				t.Fatal(err)
			}
			if !Apply(domain, command) {
				t.Fatalf("rejected command at step %d: %s", step, frame.Command)
			}
		}
		expected, err := DecodeState(frame.Observation)
		if err != nil {
			t.Fatal(err)
		}
		if actual := domain.State(); actual != expected {
			return &mismatch{step, actual}
		}
	}
	return nil
}

func FuzzShipmentReplay(f *testing.F) {
	traces := [2][]frame{load(f, "courier"), load(f, "post")}
	f.Add(false)
	f.Add(true)
	f.Fuzz(func(t *testing.T, post bool) {
		index := 0
		if post {
			index = 1
		}
		if found := replay(t, traces[index], false); found != nil {
			t.Fatalf("unexpected mismatch: %+v", found)
		}
	})
}

func TestShipmentReplayDetectsFirstCarrierOverwrite(t *testing.T) {
	if found := replay(t, load(t, "post"), true); found != nil {
		t.Fatalf("Post should still match: %+v", found)
	}
	found := replay(t, load(t, "courier"), true)
	want := mismatch{2, State{Acknowledged: true, Status: StatusShipped{Value: CarrierPost}}}
	if found == nil || *found != want {
		t.Fatalf("first mismatch = %+v, want %+v", found, want)
	}
}
