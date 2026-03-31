package notes

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sync"
)

// NoteRepo represents a single notes repository
type NoteRepo struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	Path string `json:"path"`
}

// Config holds notes sync configuration
type Config struct {
	Repos         []NoteRepo `json:"repos"`
	CurrentRepoID string     `json:"current_repo_id"`
}

// ConfigManager manages notes configuration
type ConfigManager struct {
	configPath string
	config     Config
	mu         sync.RWMutex
}

// NewConfigManager creates a new config manager
func NewConfigManager(appDataDir string) *ConfigManager {
	return &ConfigManager{
		configPath: filepath.Join(appDataDir, "notes_mcp_config.json"),
		config: Config{
			Repos: []NoteRepo{
				{
					ID:   "default",
					Name: "Default",
					Path: filepath.Join(appDataDir, "notes"),
				},
			},
			CurrentRepoID: "default",
		},
	}
}

// Load loads configuration from file
func (m *ConfigManager) Load() error {
	m.mu.Lock()
	defer m.mu.Unlock()

	data, err := os.ReadFile(m.configPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil // use defaults
		}
		return err
	}

	return json.Unmarshal(data, &m.config)
}

// saveInternal saves without acquiring lock (must be called with lock held)
func (m *ConfigManager) saveInternal() error {
	// Ensure directory exists
	dir := filepath.Dir(m.configPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	data, err := json.MarshalIndent(m.config, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(m.configPath, data, 0644)
}

// Save saves configuration to file
func (m *ConfigManager) Save() error {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.saveInternal()
}

// ListRepos returns all repos and the current one
func (m *ConfigManager) ListRepos() []NoteRepo {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.config.Repos
}

// GetCurrentRepo returns the current active repo
func (m *ConfigManager) GetCurrentRepo() *NoteRepo {
	m.mu.RLock()
	defer m.mu.RUnlock()
	for i := range m.config.Repos {
		if m.config.Repos[i].ID == m.config.CurrentRepoID {
			return &m.config.Repos[i]
		}
	}
	// fallback to first
	if len(m.config.Repos) > 0 {
		return &m.config.Repos[0]
	}
	return nil
}

// GetRepoPath returns the current repo path (for backward compatibility)
func (m *ConfigManager) GetRepoPath() string {
	m.mu.RLock()
	defer m.mu.RUnlock()
	for i := range m.config.Repos {
		if m.config.Repos[i].ID == m.config.CurrentRepoID {
			return m.config.Repos[i].Path
		}
	}
	if len(m.config.Repos) > 0 {
		return m.config.Repos[0].Path
	}
	return ""
}

// SetRepoPath sets a new repo path (for backward compatibility - sets path on current repo)
func (m *ConfigManager) SetRepoPath(path string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	for i := range m.config.Repos {
		if m.config.Repos[i].ID == m.config.CurrentRepoID {
			m.config.Repos[i].Path = path
			return m.saveInternal()
		}
	}
	return nil
}

// AddRepo adds a new repo
func (m *ConfigManager) AddRepo(id, name, path string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Check if ID already exists
	for i := range m.config.Repos {
		if m.config.Repos[i].ID == id {
			m.config.Repos[i].Name = name
			m.config.Repos[i].Path = path
			return m.saveInternal()
		}
	}

	m.config.Repos = append(m.config.Repos, NoteRepo{
		ID:   id,
		Name: name,
		Path: path,
	})
	return m.saveInternal()
}

// RemoveRepo removes a repo by ID
func (m *ConfigManager) RemoveRepo(id string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	for i := range m.config.Repos {
		if m.config.Repos[i].ID == id {
			m.config.Repos = append(m.config.Repos[:i], m.config.Repos[i+1:]...)
			// If we removed the current repo, switch to first available
			if m.config.CurrentRepoID == id && len(m.config.Repos) > 0 {
				m.config.CurrentRepoID = m.config.Repos[0].ID
			}
			return m.saveInternal()
		}
	}
	return nil
}

// SetCurrentRepo sets the current active repo by ID
func (m *ConfigManager) SetCurrentRepo(id string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Verify repo exists
	for i := range m.config.Repos {
		if m.config.Repos[i].ID == id {
			m.config.CurrentRepoID = id
			return m.saveInternal()
		}
	}
	return nil
}
