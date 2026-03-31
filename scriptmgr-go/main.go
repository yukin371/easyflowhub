package main

import (
	"fmt"
	"os"

	"scriptmgr/internal/api"
	"scriptmgr/internal/cli"
	"scriptmgr/internal/config"
	"scriptmgr/internal/discovery"
	"scriptmgr/internal/executor"
	"scriptmgr/internal/store"
)

func main() {
	cfg, err := config.Load()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}

	s := store.New(cfg)
	d := discovery.New(s)
	e := executor.New(s, d)

	// Initialize override store
	o, err := store.NewOverrideStore(cfg.StateDir)
	if err != nil {
		fmt.Fprintln(os.Stderr, "failed to init override store:", err)
		os.Exit(1)
	}

	a := api.New(d, e, s, o)

	if err := cli.Run(os.Args[1:], a); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
