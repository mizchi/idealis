// Command permissions implements fixtures/rmodel/permissions.model against
// the generated interface and serves it to rcheck (plan 0104). The first
// argument selects a fault: "read" skips the grant check, "close" leaves
// the document open.
package main

import (
	"fmt"
	"os"

	"github.com/mizchi/idealis/tests/idealis-check/go/permissions"
)

type domain struct {
	config permissions.Config
	open   map[permissions.User]permissions.Set[permissions.Doc]
	fault  string
}

func (d *domain) State() permissions.State {
	return permissions.State{Open: d.open}
}

func (d *domain) Read(u permissions.User, doc permissions.Doc) bool {
	granted := false
	for role := range d.config.Grants[u] {
		if _, ok := d.config.Reads[role][doc]; ok {
			granted = true
		}
	}
	if !granted && d.fault != "read" {
		return false
	}
	if d.open[u] == nil {
		d.open[u] = permissions.Set[permissions.Doc]{}
	}
	d.open[u][doc] = struct{}{}
	return true
}

func (d *domain) Close(u permissions.User, doc permissions.Doc) bool {
	if _, ok := d.open[u][doc]; !ok {
		return false
	}
	if d.fault != "close" {
		delete(d.open[u], doc)
	}
	return true
}

func main() {
	fault := ""
	if len(os.Args) > 1 {
		fault = os.Args[1]
	}
	create := func(config permissions.Config) permissions.Permissions {
		return &domain{config: config, open: map[permissions.User]permissions.Set[permissions.Doc]{}, fault: fault}
	}
	if err := permissions.Serve(create, os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
