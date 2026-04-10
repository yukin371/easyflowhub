package main

import (
	"log/slog"
	"os"

	"scriptmgr/internal/api"
	"scriptmgr/internal/cli"
	"scriptmgr/internal/config"
	"scriptmgr/internal/discovery"
	"scriptmgr/internal/executor"
	"scriptmgr/internal/extensions"
	"scriptmgr/internal/store"
)

func main() {
	// Initialize default structured logger for packages that use slog
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{
		Level: slog.LevelInfo,
	})))

	cfg, err := config.Load()
	if err != nil {
		slog.Error("failed to load config", "error", err)
		os.Exit(1)
	}

	s := store.New(cfg)
	registry, err := extensions.NewRegistry(cfg.StateDir)
	if err != nil {
		slog.Error("failed to init extensions registry", "error", err)
		os.Exit(1)
	}
	d := discovery.NewWithExtensionsRegistry(s, registry)
	e := executor.New(s, d)

	// Initialize override store
	o, err := store.NewOverrideStore(cfg.StateDir)
	if err != nil {
		slog.Error("failed to init override store", "error", err)
		os.Exit(1)
	}

	a := api.New(d, e, s, o)

	if err := cli.Run(os.Args[1:], a); err != nil {
		slog.Error("CLI error", "error", err)
		os.Exit(1)
	}
}
