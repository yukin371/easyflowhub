package store

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sync"
)

// ScriptOverride contains user-defined overrides for a script
type ScriptOverride struct {
	Category string `json:"category,omitempty"`
}

// OverrideStore manages script metadata overrides
type OverrideStore struct {
	mu        sync.RWMutex
	filePath  string
	overrides map[string]ScriptOverride
}

// NewOverrideStore creates a new override store
func NewOverrideStore(stateDir string) (*OverrideStore, error) {
	filePath := filepath.Join(stateDir, "script-overrides.json")
	s := &OverrideStore{
		filePath:  filePath,
		overrides: make(map[string]ScriptOverride),
	}
	if err := s.load(); err != nil && !os.IsNotExist(err) {
		return nil, err
	}
	return s, nil
}

// load reads overrides from disk
func (s *OverrideStore) load() error {
	data, err := os.ReadFile(s.filePath)
	if err != nil {
		return err
	}
	return json.Unmarshal(data, &s.overrides)
}

// save writes overrides to disk
func (s *OverrideStore) save() error {
	data, err := json.MarshalIndent(s.overrides, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(s.filePath, data, 0o644)
}

// Get returns the override for a script, or empty if none
func (s *OverrideStore) Get(scriptID string) ScriptOverride {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.overrides[scriptID]
}

// Set updates the override for a script
func (s *OverrideStore) Set(scriptID string, override ScriptOverride) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if override.Category == "" {
		delete(s.overrides, scriptID)
	} else {
		s.overrides[scriptID] = override
	}

	return s.save()
}

// GetAll returns all overrides
func (s *OverrideStore) GetAll() map[string]ScriptOverride {
	s.mu.RLock()
	defer s.mu.RUnlock()

	result := make(map[string]ScriptOverride, len(s.overrides))
	for k, v := range s.overrides {
		result[k] = v
	}
	return result
}
