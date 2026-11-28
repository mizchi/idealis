// Command sessions implements fixtures/rmodel/sessions.model (plan 0104).
// The first argument "login" skips the capacity check.
package main

import (
	"fmt"
	"os"

	"github.com/mizchi/idealis/tests/idealis-check/go/sessions"
)

type domain struct {
	admins sessions.Set[sessions.User]
	active sessions.Set[sessions.User]
	locked *sessions.User
	fault  string
}

func (d *domain) State() sessions.State {
	return sessions.State{Active: d.active, Locked: d.locked}
}

func (d *domain) Login(u sessions.User) bool {
	if _, ok := d.active[u]; ok || (len(d.active) >= 2 && d.fault != "login") {
		return false
	}
	d.active[u] = struct{}{}
	return true
}

func (d *domain) Logout(u sessions.User) bool {
	if _, ok := d.active[u]; !ok {
		return false
	}
	delete(d.active, u)
	return true
}

func (d *domain) Lock(u sessions.User) bool {
	_, admin := d.admins[u]
	_, active := d.active[u]
	if d.locked != nil || !admin || !active {
		return false
	}
	d.locked = &u
	return true
}

func (d *domain) Unlock(u sessions.User) bool {
	if d.locked == nil || *d.locked != u {
		return false
	}
	d.locked = nil
	return true
}

func main() {
	fault := ""
	if len(os.Args) > 1 {
		fault = os.Args[1]
	}
	create := func(config sessions.Config) sessions.Sessions {
		return &domain{admins: config.Admins, active: sessions.Set[sessions.User]{}, fault: fault}
	}
	if err := sessions.Serve(create, os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
