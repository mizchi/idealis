package withdrawal

import (
	"encoding/json"
	"strings"
	"testing"
)

type account struct {
	state            State
	corruptRejection bool
}

var _ Withdrawal = (*account)(nil)

func newAccount(_ Config, corruptRejection bool) Withdrawal {
	return &account{
		state: State{
			Balance:       Amount{Units: 2},
			BeforeBalance: Amount{Units: 2},
			Result:        WithdrawalResultIdle,
		},
		corruptRejection: corruptRejection,
	}
}

func (a *account) State() State { return a.state }

func (a *account) RequestWithdrawal(amount Amount) bool {
	if a.state.Result == WithdrawalResultPending || amount.Units <= 0 {
		return false
	}
	a.state.Requested = amount
	a.state.Result = WithdrawalResultPending
	return true
}

func (a *account) Withdraw() bool {
	if a.state.Result != WithdrawalResultPending {
		return false
	}
	remaining := a.state.Balance.Units - a.state.Requested.Units
	if remaining < 0 {
		return false
	}
	a.state.BeforeBalance = a.state.Balance
	a.state.Balance = Amount{Units: remaining}
	a.state.Result = WithdrawalResultWithdrawn
	return true
}

func (a *account) RejectWithdrawal() bool {
	if a.state.Result != WithdrawalResultPending || a.state.Balance.Units >= a.state.Requested.Units {
		return false
	}
	a.state.BeforeBalance = a.state.Balance
	a.state.Result = WithdrawalResultInsufficientFunds
	if a.corruptRejection {
		a.state.Balance = Amount{}
	}
	return true
}

func FuzzWithdrawalBoundaries(f *testing.F) {
	CheckBoundaryCases(f, func(config Config) Withdrawal {
		return newAccount(config, false)
	})
}

func TestBoundaryReplayDetectsCorruptRejection(t *testing.T) {
	var corpus struct{ Pairs []boundaryPair }
	if err := json.Unmarshal([]byte(boundaryCorpus), &corpus); err != nil {
		t.Fatal(err)
	}
	for _, pair := range corpus.Pairs {
		if pair.Action == "rejectWithdrawal" && pair.Kind == "continuation" {
			err := replayBoundary(pair, func(config Config) Withdrawal {
				return newAccount(config, true)
			})
			if err == nil || !strings.Contains(err.Error(), "continuation boundary for rejectWithdrawal") ||
				!strings.Contains(err.Error(), `state {"balance":{"units":0}`) {
				t.Fatalf("expected the balance mismatch after rejection, got %v", err)
			}
			return
		}
	}
	t.Fatal("missing the modeled rejection boundary")
}
