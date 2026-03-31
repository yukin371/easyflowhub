package notes

import (
	"os"
	"path/filepath"
)

// API provides a unified interface for notes operations
type API struct {
	configManager *ConfigManager
	fileHandler   *FileHandler
	syncer        *Syncer
}

// NewAPI creates a new notes API
func NewAPI(appDataDir string) (*API, error) {
	configManager := NewConfigManager(appDataDir)
	if err := configManager.Load(); err != nil {
		// Non-fatal, use defaults
	}

	repoPath := configManager.GetRepoPath()
	fileHandler := NewFileHandler(repoPath)
	syncer := NewSyncer(appDataDir, fileHandler)

	return &API{
		configManager: configManager,
		fileHandler:   fileHandler,
		syncer:        syncer,
	}, nil
}

// GetRepoPath returns the current repository path
func (a *API) GetRepoPath() string {
	return a.configManager.GetRepoPath()
}

// SetRepoPath sets a new repository path
func (a *API) SetRepoPath(path string) error {
	// Update config
	if err := a.configManager.SetRepoPath(path); err != nil {
		return err
	}
	// Update file handler with new path
	a.fileHandler = NewFileHandler(path)
	a.syncer = NewSyncer(filepath.Dir(a.configManager.configPath), a.fileHandler)
	return nil
}

// Sync performs synchronization
func (a *API) Sync(direction SyncDirection) (*SyncReport, error) {
	return a.syncer.Sync(direction)
}

// ListNotes lists notes from the file repository
func (a *API) ListNotes(search string) ([]*Note, error) {
	return a.fileHandler.SearchNotes(search)
}

// GetNote gets a single note from the file repository
func (a *API) GetNote(noteID string) (*Note, error) {
	return a.fileHandler.ReadNote(noteID)
}

// RepoExists checks if the repository exists
func (a *API) RepoExists() bool {
	return a.fileHandler.RepoExists()
}

// EnsureRepo ensures the repository directory exists
func (a *API) EnsureRepo() error {
	return a.fileHandler.EnsureRepoDir()
}

// GetAppDataDir returns the default app data directory for DeskFlow
func GetAppDataDir() string {
	// On Windows, DeskFlow uses Tauri's app data dir which is based on identifier
	// identifier: com.yukin.deskflow -> %APPDATA%/com.yukin.deskflow
	appData := os.Getenv("APPDATA")
	if appData == "" {
		// Fallback to home directory
		home, _ := os.UserHomeDir()
		return filepath.Join(home, ".deskflow-app")
	}
	// Tauri uses the bundle identifier for the app data directory
	return filepath.Join(appData, "com.yukin.deskflow")
}

// ===== Multi-repo support =====

// NoteRepoInfo represents repo info for external use
type NoteRepoInfo struct {
	ID            string `json:"id"`
	Name          string `json:"name"`
	Path          string `json:"path"`
	IsCurrent     bool   `json:"is_current"`
	NoteCount     int    `json:"note_count"`
}

// ListRepos returns all repos with their info
func (a *API) ListRepos() []NoteRepoInfo {
	repos := a.configManager.ListRepos()
	currentID := a.configManager.GetCurrentRepo().ID

	result := make([]NoteRepoInfo, 0, len(repos))
	for _, repo := range repos {
		info := NoteRepoInfo{
			ID:        repo.ID,
			Name:      repo.Name,
			Path:      repo.Path,
			IsCurrent: repo.ID == currentID,
		}
		// Count notes in this repo
		fh := NewFileHandler(repo.Path)
		if notes, err := fh.ReadAllNotes(); err == nil {
			info.NoteCount = len(notes)
		}
		result = append(result, info)
	}
	return result
}

// AddRepo adds a new repo
func (a *API) AddRepo(id, name, path string) error {
	return a.configManager.AddRepo(id, name, path)
}

// RemoveRepo removes a repo by ID
func (a *API) RemoveRepo(id string) error {
	return a.configManager.RemoveRepo(id)
}

// SetCurrentRepo sets the current active repo by ID
func (a *API) SetCurrentRepo(id string) error {
	if err := a.configManager.SetCurrentRepo(id); err != nil {
		return err
	}
	// Update file handler to use new repo
	newPath := a.configManager.GetRepoPath()
	a.fileHandler = NewFileHandler(newPath)
	a.syncer = NewSyncer(filepath.Dir(a.configManager.configPath), a.fileHandler)
	return nil
}

// GetCurrentRepo returns the current active repo
func (a *API) GetCurrentRepo() *NoteRepo {
	return a.configManager.GetCurrentRepo()
}
