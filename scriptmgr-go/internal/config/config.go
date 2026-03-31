package config

import (
	"os"
	"path/filepath"
)

type Config struct {
	StateDir string
}

func Load() (Config, error) {
	base, err := os.UserConfigDir()
	if err != nil {
		return Config{}, err
	}
	stateDir := filepath.Join(base, "scriptmgr")
	if err := os.MkdirAll(stateDir, 0o755); err != nil {
		return Config{}, err
	}
	return Config{StateDir: stateDir}, nil
}
