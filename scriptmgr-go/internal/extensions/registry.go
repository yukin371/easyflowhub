package extensions

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

const manifestName = "plugin.json"

type Registry struct {
	roots []string
}

func NewRegistry(stateDir string) (*Registry, error) {
	stateRoot := filepath.Join(stateDir, "extensions")
	if err := os.MkdirAll(stateRoot, 0o755); err != nil {
		return nil, fmt.Errorf("create state extensions dir: %w", err)
	}

	return &Registry{roots: uniqueExistingRoots(defaultRoots(stateRoot))}, nil
}

func NewRegistryWithRoots(roots []string) *Registry {
	return &Registry{roots: uniqueExistingRoots(roots)}
}

func (r *Registry) Roots() []string {
	return append([]string(nil), r.roots...)
}

func (r *Registry) List() ([]ListedExtension, error) {
	var listed []ListedExtension
	for _, root := range r.roots {
		items, err := os.ReadDir(root)
		if err != nil {
			if errors.Is(err, os.ErrNotExist) {
				continue
			}
			return nil, err
		}
		for _, item := range items {
			if !item.IsDir() {
				continue
			}
			manifestPath := filepath.Join(root, item.Name(), manifestName)
			listed = append(listed, loadManifest(root, manifestPath))
		}
	}

	sort.Slice(listed, func(i, j int) bool {
		leftID := manifestID(listed[i])
		rightID := manifestID(listed[j])
		if leftID == rightID {
			return listed[i].ManifestPath < listed[j].ManifestPath
		}
		return leftID < rightID
	})

	seen := map[string]string{}
	for i := range listed {
		id := manifestID(listed[i])
		if id == "" {
			continue
		}
		if first, exists := seen[id]; exists {
			listed[i].Status = "invalid"
			listed[i].Error = fmt.Sprintf("duplicate extension id %q, already declared in %s", id, first)
			continue
		}
		seen[id] = listed[i].ManifestPath
	}

	return listed, nil
}

func (r *Registry) EffectiveContributions() (EffectiveContributions, error) {
	items, err := r.List()
	if err != nil {
		return EffectiveContributions{}, err
	}
	return NewContributionAggregator().Merge(items), nil
}

func loadManifest(root, manifestPath string) ListedExtension {
	item := ListedExtension{
		ManifestPath: manifestPath,
		Root:         root,
		Status:       "loaded",
	}

	content, err := os.ReadFile(manifestPath)
	if err != nil {
		item.Status = "invalid"
		item.Error = err.Error()
		return item
	}

	var manifest Manifest
	if err := json.Unmarshal(content, &manifest); err != nil {
		item.Status = "invalid"
		item.Error = fmt.Sprintf("invalid JSON: %v", err)
		return item
	}

	if err := validateManifest(manifest); err != nil {
		item.Status = "invalid"
		item.Error = err.Error()
		item.Manifest = &manifest
		return item
	}

	item.Manifest = &manifest
	return item
}

func validateManifest(manifest Manifest) error {
	if strings.TrimSpace(manifest.ID) == "" {
		return errors.New("missing id")
	}
	if strings.TrimSpace(manifest.Name) == "" {
		return errors.New("missing name")
	}
	if strings.TrimSpace(manifest.Version) == "" {
		return errors.New("missing version")
	}
	return nil
}

func manifestID(item ListedExtension) string {
	if item.Manifest == nil {
		return filepath.Base(filepath.Dir(item.ManifestPath))
	}
	return item.Manifest.ID
}

func defaultRoots(stateRoot string) []string {
	roots := []string{stateRoot}

	if exePath, err := os.Executable(); err == nil && exePath != "" {
		roots = append(roots, filepath.Join(filepath.Dir(exePath), "extensions"))
	}

	if raw := strings.TrimSpace(os.Getenv("SCRIPTMGR_EXTENSION_DIRS")); raw != "" {
		for _, item := range filepath.SplitList(raw) {
			item = strings.TrimSpace(item)
			if item != "" {
				roots = append(roots, item)
			}
		}
	}

	return roots
}

func uniqueExistingRoots(roots []string) []string {
	seen := map[string]bool{}
	var result []string
	for _, root := range roots {
		root = strings.TrimSpace(root)
		if root == "" {
			continue
		}
		abs, err := filepath.Abs(root)
		if err != nil {
			continue
		}
		if seen[abs] {
			continue
		}
		seen[abs] = true
		if _, err := os.Stat(abs); err == nil {
			result = append(result, abs)
		}
	}
	sort.Strings(result)
	return result
}
