package relay

import (
	"bytes"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"
)

type Store struct {
	path string
	mu   sync.RWMutex
}

func NewStore(stateDir string) *Store {
	return &Store{path: filepath.Join(stateDir, "relay.json")}
}

func (s *Store) Path() string {
	return s.path
}

func (s *Store) Load() (Config, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	content, err := os.ReadFile(s.path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return DefaultConfig(), nil
		}
		return Config{}, err
	}
	if len(bytes.TrimSpace(content)) == 0 {
		return DefaultConfig(), nil
	}

	cfg := DefaultConfig()
	if err := json.Unmarshal(content, &cfg); err != nil {
		return Config{}, err
	}
	cfg = cfg.Normalize()
	return cfg, cfg.Validate()
}

func (s *Store) Save(cfg Config) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	cfg = cfg.Normalize()
	if err := cfg.Validate(); err != nil {
		return err
	}

	content, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(s.path, content, 0o644)
}
