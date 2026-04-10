package discovery

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"scriptmgr/internal/extensions"
	"scriptmgr/internal/model"
	"scriptmgr/internal/store"
)

type Service struct {
	store       *store.Store
	extensions  *extensions.Registry
	scriptTypes map[string]string
}

func New(s *store.Store) *Service {
	return NewWithExtensionsRegistry(s, nil)
}

func NewWithExtensionsRegistry(s *store.Store, registry *extensions.Registry) *Service {
	return &Service{
		store:      s,
		extensions: registry,
		scriptTypes: map[string]string{
			".ps1": "powershell",
			".py":  "python",
			".bat": "batch",
			".cmd": "batch",
		},
	}
}

func (s *Service) DiscoverScripts(search string) ([]model.ScriptRecord, error) {
	roots, err := s.ScriptRoots()
	if err != nil {
		return nil, err
	}

	search = strings.TrimSpace(strings.ToLower(search))
	var records []model.ScriptRecord
	seenPaths := map[string]struct{}{}

	for _, root := range roots {
		err := filepath.WalkDir(root, func(path string, d os.DirEntry, walkErr error) error {
			if walkErr != nil {
				return nil
			}
			if d.IsDir() {
				name := d.Name()
				if strings.HasPrefix(name, ".") || name == "__pycache__" || name == "node_modules" || name == "bin" || name == "obj" {
					return filepath.SkipDir
				}
				return nil
			}

			scriptType, ok := s.scriptTypes[strings.ToLower(filepath.Ext(path))]
			if !ok {
				return nil
			}

			record := s.buildRecord(path, scriptType, root)
			cleanedPath := filepath.Clean(record.Path)
			if _, ok := seenPaths[cleanedPath]; ok {
				return nil
			}
			if search != "" && !matchesSearch(record, search) {
				return nil
			}
			seenPaths[cleanedPath] = struct{}{}
			records = append(records, record)
			return nil
		})
		if err != nil {
			return nil, err
		}
	}

	sort.Slice(records, func(i, j int) bool {
		if strings.EqualFold(records[i].Name, records[j].Name) {
			return strings.ToLower(records[i].Path) < strings.ToLower(records[j].Path)
		}
		return strings.ToLower(records[i].Name) < strings.ToLower(records[j].Name)
	})
	return records, nil
}

func (s *Service) FindScript(scriptID string) (model.ScriptRecord, error) {
	scripts, err := s.DiscoverScripts("")
	if err != nil {
		return model.ScriptRecord{}, err
	}
	for _, script := range scripts {
		if strings.EqualFold(script.ID, scriptID) {
			return script, nil
		}
	}
	return model.ScriptRecord{}, fmt.Errorf("script not found: %s", scriptID)
}

func (s *Service) ScriptRoots() ([]string, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return nil, err
	}

	seen := map[string]struct{}{}
	var roots []string

	for _, start := range []string{cwd, executableDir()} {
		dir := filepath.Clean(start)
		for {
			for _, name := range []string{"PowerShell", "Python"} {
				path := filepath.Join(dir, name)
				if info, err := os.Stat(path); err == nil && info.IsDir() {
					cleaned := filepath.Clean(path)
					if _, ok := seen[cleaned]; !ok {
						seen[cleaned] = struct{}{}
						roots = append(roots, cleaned)
					}
				}
			}
			parent := filepath.Dir(dir)
			if parent == dir {
				break
			}
			dir = parent
		}
	}

	custom, err := s.store.LoadRoots()
	if err != nil {
		return nil, err
	}
	for _, root := range custom.Roots {
		roots = appendExistingRoot(roots, seen, root)
	}

	if s.extensions != nil {
		contributions, err := s.extensions.EffectiveContributions()
		if err == nil {
			for _, item := range contributions.ScriptRoots {
				roots = appendExistingRoot(roots, seen, resolveExtensionScriptRoot(item))
			}
		}
	}

	if len(roots) == 0 {
		return nil, errors.New("no script directories found; expected PowerShell or Python under the repo root")
	}

	sort.Slice(roots, func(i, j int) bool {
		return strings.ToLower(roots[i]) < strings.ToLower(roots[j])
	})

	var normalized []string
	for _, root := range roots {
		skip := false
		for _, kept := range normalized {
			if isSameOrNestedRoot(root, kept) {
				skip = true
				break
			}
		}
		if !skip {
			normalized = append(normalized, root)
		}
	}
	return normalized, nil
}

func (s *Service) buildRecord(path, scriptType, sourceRoot string) model.ScriptRecord {
	metadata := loadMetadata(path)
	name := metadata.Name
	if name == "" {
		name = strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	}
	id := metadata.ID
	if id == "" {
		id = strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	}
	return model.ScriptRecord{
		ID:          id,
		Name:        name,
		Path:        path,
		ScriptType:  scriptType,
		SourceRoot:  filepath.Clean(sourceRoot),
		Description: metadata.Description,
		Category:    metadata.Category,
		Tags:        metadata.Tags,
		Author:      metadata.Author,
		Version:     metadata.Version,
		Icon:        metadata.Icon,
		Parameters:  metadata.Parameters,
	}
}

func loadMetadata(scriptPath string) model.ScriptMetadata {
	metadataPath := strings.TrimSuffix(scriptPath, filepath.Ext(scriptPath)) + ".json"
	content, err := os.ReadFile(metadataPath)
	if err != nil {
		return model.ScriptMetadata{}
	}
	var metadata model.ScriptMetadata
	if err := json.Unmarshal(content, &metadata); err != nil {
		return model.ScriptMetadata{}
	}
	return metadata
}

func matchesSearch(script model.ScriptRecord, search string) bool {
	searchLower := strings.ToLower(search)
	values := []string{
		script.ID,
		script.Name,
		script.Description,
		script.Category,
		script.SourceRoot,
		strings.Join(script.Tags, " "),
	}
	for _, value := range values {
		if strings.Contains(strings.ToLower(value), searchLower) {
			return true
		}
	}
	return false
}

func executableDir() string {
	exePath, err := os.Executable()
	if err != nil {
		return "."
	}
	return filepath.Dir(exePath)
}

func appendExistingRoot(roots []string, seen map[string]struct{}, root string) []string {
	root = strings.TrimSpace(root)
	if root == "" {
		return roots
	}

	if info, err := os.Stat(root); err == nil && info.IsDir() {
		cleaned := filepath.Clean(root)
		if _, ok := seen[cleaned]; !ok {
			seen[cleaned] = struct{}{}
			roots = append(roots, cleaned)
		}
	}

	return roots
}

func resolveExtensionScriptRoot(item extensions.EffectiveScriptRootContribution) string {
	path := strings.TrimSpace(item.Path)
	if path == "" {
		return ""
	}
	if filepath.IsAbs(path) {
		return filepath.Clean(path)
	}

	baseDir := filepath.Dir(item.Source.ManifestPath)
	if baseDir == "" || baseDir == "." {
		baseDir = item.Source.Root
	}
	return filepath.Clean(filepath.Join(baseDir, path))
}

func isSameOrNestedRoot(root, candidate string) bool {
	// Normalize backslashes to forward slashes for cross-platform consistency
	root = strings.ReplaceAll(root, "\\", "/")
	candidate = strings.ReplaceAll(candidate, "\\", "/")
	// Lowercase for case-insensitive comparison
	root = strings.ToLower(root)
	candidate = strings.ToLower(candidate)
	// Remove trailing/leading slashes for consistent comparison
	root = strings.Trim(root, "/")
	candidate = strings.Trim(candidate, "/")
	if root == candidate {
		return true
	}
	// Check if root is nested within candidate (candidate is a prefix of root)
	return strings.HasPrefix(root, candidate+"/")
}
