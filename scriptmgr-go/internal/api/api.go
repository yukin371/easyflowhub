package api

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"scriptmgr/internal/discovery"
	"scriptmgr/internal/executor"
	"scriptmgr/internal/model"
	"scriptmgr/internal/store"
)

type API struct {
	discovery *discovery.Service
	executor  *executor.Service
	store     *store.Store
	overrides *store.OverrideStore
}

func New(d *discovery.Service, e *executor.Service, s *store.Store, o *store.OverrideStore) *API {
	return &API{
		discovery: d,
		executor:  e,
		store:     s,
		overrides: o,
	}
}

func (a *API) ListScripts(search string) ([]model.ScriptRecord, []string, error) {
	scripts, err := a.discovery.DiscoverScripts(search)
	if err != nil {
		return nil, nil, err
	}
	// Apply category overrides
	if a.overrides != nil {
		for i := range scripts {
			if override := a.overrides.Get(scripts[i].ID); override.Category != "" {
				scripts[i].Category = override.Category
			}
		}
	}
	roots, err := a.discovery.ScriptRoots()
	if err != nil {
		return nil, nil, err
	}
	return scripts, roots, nil
}

// ListCategories returns all script categories with their counts
func (a *API) ListCategories() (map[string]int, error) {
	scripts, err := a.discovery.DiscoverScripts("")
	if err != nil {
		return nil, err
	}

	categories := make(map[string]int)
	for _, script := range scripts {
		cat := script.Category
		if cat == "" {
			cat = "uncategorized"
		}
		categories[cat]++
	}

	return categories, nil
}

// ListScriptsByCategory returns scripts filtered by category
func (a *API) ListScriptsByCategory(category string) ([]model.ScriptRecord, error) {
	all, err := a.discovery.DiscoverScripts("")
	if err != nil {
		return nil, err
	}

	// Normalize empty category
	if category == "" {
		category = "uncategorized"
	}

	var filtered []model.ScriptRecord
	for _, script := range all {
		cat := script.Category
		if cat == "" {
			cat = "uncategorized"
		}
		if strings.EqualFold(cat, category) {
			filtered = append(filtered, script)
		}
	}

	return filtered, nil
}

func (a *API) DescribeScript(scriptID string) (model.ScriptRecord, error) {
	script, err := a.discovery.FindScript(scriptID)
	if err != nil {
		return script, err
	}
	// Apply category override
	if a.overrides != nil {
		if override := a.overrides.Get(script.ID); override.Category != "" {
			script.Category = override.Category
		}
	}
	return script, nil
}

// UpdateScriptCategory sets or clears the category override for a script
func (a *API) UpdateScriptCategory(scriptID, category string) error {
	// Verify script exists
	_, err := a.discovery.FindScript(scriptID)
	if err != nil {
		return err
	}
	if a.overrides == nil {
		return fmt.Errorf("override store not initialized")
	}
	return a.overrides.Set(scriptID, store.ScriptOverride{Category: category})
}

func (a *API) RunScript(scriptID string, scriptArgs []string, opts model.RunOptions) (any, error) {
	script, err := a.discovery.FindScript(scriptID)
	if err != nil {
		return nil, err
	}
	validatedArgs, err := a.executor.ValidateScriptArgs(script, scriptArgs)
	if err != nil {
		return nil, err
	}
	command, err := a.executor.BuildCommand(script, validatedArgs)
	if err != nil {
		return nil, err
	}

	if opts.DryRun {
		return map[string]any{
			"script_id": script.ID,
			"args":      validatedArgs,
			"command":   command,
		}, nil
	}

	if opts.Detach {
		return a.executor.StartDetachedSession(script, validatedArgs, command)
	}

	return a.executor.RunForeground(script, command, opts.CaptureOutput || opts.AsJSON)
}

func (a *API) RunWorker(sessionID, scriptID string, scriptArgs []string) error {
	script, err := a.discovery.FindScript(scriptID)
	if err != nil {
		return err
	}
	validatedArgs, err := a.executor.ValidateScriptArgs(script, scriptArgs)
	if err != nil {
		return err
	}
	return a.executor.RunWorker(sessionID, scriptID, validatedArgs)
}

func (a *API) History(limit int) ([]model.HistoryEntry, error) {
	history, err := a.store.LoadHistory()
	if err != nil {
		return nil, err
	}
	sort.Slice(history, func(i, j int) bool { return history[i].StartedAt > history[j].StartedAt })
	if limit > 0 && len(history) > limit {
		history = history[:limit]
	}
	return history, nil
}

func (a *API) FavoriteScripts() ([]model.ScriptRecord, error) {
	favorites, err := a.store.LoadFavorites()
	if err != nil {
		return nil, err
	}
	all, err := a.discovery.DiscoverScripts("")
	if err != nil {
		return nil, err
	}
	byID := map[string]model.ScriptRecord{}
	for _, script := range all {
		byID[strings.ToLower(script.ID)] = script
	}

	var result []model.ScriptRecord
	for _, id := range favorites.IDs {
		if script, ok := byID[strings.ToLower(id)]; ok {
			result = append(result, script)
		}
	}
	return result, nil
}

func (a *API) AddFavorite(scriptID string) (string, error) {
	script, err := a.discovery.FindScript(scriptID)
	if err != nil {
		return "", err
	}
	favorites, err := a.store.LoadFavorites()
	if err != nil {
		return "", err
	}
	if !containsFold(favorites.IDs, script.ID) {
		favorites.IDs = append(favorites.IDs, script.ID)
	}
	sort.Slice(favorites.IDs, func(i, j int) bool {
		return strings.ToLower(favorites.IDs[i]) < strings.ToLower(favorites.IDs[j])
	})
	favorites.UpdatedAt = nowRFC3339()
	return script.ID, a.store.SaveFavorites(favorites)
}

func (a *API) RemoveFavorite(scriptID string) error {
	favorites, err := a.store.LoadFavorites()
	if err != nil {
		return err
	}
	filtered := favorites.IDs[:0]
	for _, id := range favorites.IDs {
		if !strings.EqualFold(id, scriptID) {
			filtered = append(filtered, id)
		}
	}
	favorites.IDs = filtered
	favorites.UpdatedAt = nowRFC3339()
	return a.store.SaveFavorites(favorites)
}

func (a *API) ListRoots() ([]string, error) {
	roots, err := a.store.LoadRoots()
	if err != nil {
		return nil, err
	}
	return roots.Roots, nil
}

func (a *API) AddRoot(rootPath string) (string, error) {
	cleaned, err := normalizeRootPath(rootPath)
	if err != nil {
		return "", err
	}
	roots, err := a.store.LoadRoots()
	if err != nil {
		return "", err
	}
	if !containsPath(roots.Roots, cleaned) {
		roots.Roots = append(roots.Roots, cleaned)
		sort.Slice(roots.Roots, func(i, j int) bool {
			return strings.ToLower(roots.Roots[i]) < strings.ToLower(roots.Roots[j])
		})
	}
	roots.UpdatedAt = nowRFC3339()
	return cleaned, a.store.SaveRoots(roots)
}

func (a *API) RemoveRoot(rootPath string) error {
	roots, err := a.store.LoadRoots()
	if err != nil {
		return err
	}
	cleaned := filepath.Clean(strings.TrimSpace(rootPath))
	filtered := roots.Roots[:0]
	for _, root := range roots.Roots {
		if !strings.EqualFold(filepath.Clean(root), cleaned) {
			filtered = append(filtered, root)
		}
	}
	roots.Roots = filtered
	roots.UpdatedAt = nowRFC3339()
	return a.store.SaveRoots(roots)
}

func (a *API) Sessions() ([]model.SessionRecord, error) {
	return a.executor.ListSessions()
}

func (a *API) CancelSession(sessionID string) (model.SessionRecord, error) {
	return a.executor.CancelSession(sessionID)
}

func (a *API) CancelTask(taskID string) (model.SessionRecord, error) {
	return a.executor.CancelTask(taskID)
}

func (a *API) GetTask(taskID string) (*model.Task, error) {
	return a.store.GetTask(taskID)
}

func (a *API) ListTasks(status string, limit int) ([]model.Task, error) {
	return a.store.ListTasks(status, limit)
}

func (a *API) ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error) {
	return a.executor.ReadTaskLog(taskID, offset, limit, tail)
}

// StateDir returns the base state directory path
func (a *API) StateDir() string {
	return a.store.StateDir()
}

// SetBroadcaster sets the broadcaster for task status notifications
func (a *API) SetBroadcaster(b executor.StatusBroadcaster) {
	a.executor.SetBroadcaster(b)
}

func normalizeRootPath(rootPath string) (string, error) {
	trimmed := strings.TrimSpace(rootPath)
	if trimmed == "" {
		return "", fmt.Errorf("root path cannot be empty")
	}
	absolute, err := filepath.Abs(trimmed)
	if err != nil {
		return "", err
	}
	info, err := osStat(absolute)
	if err != nil {
		return "", err
	}
	if !info.IsDir() {
		return "", fmt.Errorf("root path is not a directory: %s", absolute)
	}
	return filepath.Clean(absolute), nil
}

var osStat = func(path string) (statInfo, error) {
	return stat(path)
}

type statInfo interface {
	IsDir() bool
}

func stat(path string) (statInfo, error) {
	return os.Stat(path)
}

func containsFold(values []string, target string) bool {
	for _, value := range values {
		if strings.EqualFold(value, target) {
			return true
		}
	}
	return false
}

func containsPath(values []string, target string) bool {
	for _, value := range values {
		if strings.EqualFold(filepath.Clean(value), filepath.Clean(target)) {
			return true
		}
	}
	return false
}

func nowRFC3339() string {
	return time.Now().Format(time.RFC3339)
}
