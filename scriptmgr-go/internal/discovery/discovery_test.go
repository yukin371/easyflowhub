package discovery

import (
	"os"
	"path/filepath"
	"testing"

	"scriptmgr/internal/config"
	"scriptmgr/internal/extensions"
	"scriptmgr/internal/model"
	"scriptmgr/internal/store"
)

func TestMatchesSearch(t *testing.T) {
	tests := []struct {
		name   string
		script model.ScriptRecord
		search string
		want   bool
	}{
		{
			name: "matches id",
			script: model.ScriptRecord{
				ID:   "test-script",
				Name: "Test Script",
			},
			search: "test",
			want:   true,
		},
		{
			name: "matches name",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Backup Script",
			},
			search: "backup",
			want:   true,
		},
		{
			name: "matches description",
			script: model.ScriptRecord{
				ID:          "script1",
				Name:        "Script",
				Description: "This script backs up files",
			},
			search: "backs",
			want:   true,
		},
		{
			name: "matches category",
			script: model.ScriptRecord{
				ID:       "script1",
				Name:     "Script",
				Category: "utilities",
			},
			search: "util",
			want:   true,
		},
		{
			name: "matches tags",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Script",
				Tags: []string{"backup", "automation"},
			},
			search: "backup",
			want:   true,
		},
		{
			name: "no match",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Script",
			},
			search: "xyz123",
			want:   false,
		},
		{
			name: "case insensitive",
			script: model.ScriptRecord{
				ID:   "test-script",
				Name: "Test Script",
			},
			search: "TEST",
			want:   true,
		},
		{
			name: "empty search matches all",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Script",
			},
			search: "",
			want:   true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := matchesSearch(tt.script, tt.search)
			if got != tt.want {
				t.Errorf("matchesSearch() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestIsSameOrNestedRoot(t *testing.T) {
	tests := []struct {
		name      string
		root      string
		candidate string
		want      bool
	}{
		{
			name:      "same path",
			root:      "C:\\Users\\Test",
			candidate: "C:\\Users\\Test",
			want:      true,
		},
		{
			name:      "nested path",
			root:      "C:\\Users\\Test\\Scripts",
			candidate: "C:\\Users\\Test",
			want:      true,
		},
		{
			name:      "not nested",
			root:      "C:\\Users\\Other",
			candidate: "C:\\Users\\Test",
			want:      false,
		},
		{
			name:      "case insensitive windows",
			root:      "C:\\Users\\Test",
			candidate: "c:\\users\\test",
			want:      true,
		},
		{
			name:      "different drive",
			root:      "D:\\Users\\Test",
			candidate: "C:\\Users\\Test",
			want:      false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := isSameOrNestedRoot(tt.root, tt.candidate)
			if got != tt.want {
				t.Errorf("isSameOrNestedRoot() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestScriptRootsIncludesExtensionRootsAndDedupesNestedEntries(t *testing.T) {
	workdir := t.TempDir()
	restore := chdirForTest(t, workdir)
	defer restore()

	stateDir := t.TempDir()
	s := store.New(config.Config{StateDir: stateDir})

	userRoot := filepath.Join(workdir, "user-scripts")
	if err := os.MkdirAll(userRoot, 0o755); err != nil {
		t.Fatalf("MkdirAll userRoot failed: %v", err)
	}
	if err := s.SaveRoots(model.ScriptRootsStore{Roots: []string{userRoot}}); err != nil {
		t.Fatalf("SaveRoots failed: %v", err)
	}

	extensionsRoot := filepath.Join(workdir, "extensions")
	extensionScriptRoot := filepath.Join(extensionsRoot, "sample", "scripts")
	if err := os.MkdirAll(extensionScriptRoot, 0o755); err != nil {
		t.Fatalf("MkdirAll extensionScriptRoot failed: %v", err)
	}

	writeManifest(t, filepath.Join(extensionsRoot, "sample", "plugin.json"), `{
  "id": "sample-pack",
  "name": "Sample Pack",
  "version": "1.0.0",
  "contributions": {
    "script_roots": [
      { "path": "scripts" },
      { "path": "scripts/nested" },
      { "path": "missing" }
    ]
  }
}`)

	service := NewWithExtensionsRegistry(s, extensions.NewRegistryWithRoots([]string{extensionsRoot}))
	roots, err := service.ScriptRoots()
	if err != nil {
		t.Fatalf("ScriptRoots failed: %v", err)
	}

	if len(roots) != 2 {
		t.Fatalf("expected 2 roots, got %d: %+v", len(roots), roots)
	}
	if roots[0] != filepath.Clean(extensionScriptRoot) || roots[1] != filepath.Clean(userRoot) {
		t.Fatalf("unexpected roots %+v", roots)
	}
}

func TestDiscoverScriptsScansExtensionScriptRoots(t *testing.T) {
	workdir := t.TempDir()
	restore := chdirForTest(t, workdir)
	defer restore()

	stateDir := t.TempDir()
	s := store.New(config.Config{StateDir: stateDir})

	extensionsRoot := filepath.Join(workdir, "extensions")
	extensionScriptRoot := filepath.Join(extensionsRoot, "sample", "scripts")
	if err := os.MkdirAll(extensionScriptRoot, 0o755); err != nil {
		t.Fatalf("MkdirAll extensionScriptRoot failed: %v", err)
	}
	if err := os.WriteFile(filepath.Join(extensionScriptRoot, "hello.ps1"), []byte("Write-Output 'hi'"), 0o644); err != nil {
		t.Fatalf("WriteFile script failed: %v", err)
	}

	writeManifest(t, filepath.Join(extensionsRoot, "sample", "plugin.json"), `{
  "id": "sample-pack",
  "name": "Sample Pack",
  "version": "1.0.0",
  "contributions": {
    "script_roots": [
      { "path": "scripts" }
    ]
  }
}`)

	service := NewWithExtensionsRegistry(s, extensions.NewRegistryWithRoots([]string{extensionsRoot}))
	scripts, err := service.DiscoverScripts("")
	if err != nil {
		t.Fatalf("DiscoverScripts failed: %v", err)
	}

	if len(scripts) != 1 {
		t.Fatalf("expected 1 script, got %d: %+v", len(scripts), scripts)
	}
	if scripts[0].SourceRoot != filepath.Clean(extensionScriptRoot) {
		t.Fatalf("unexpected source root %q", scripts[0].SourceRoot)
	}
}

func chdirForTest(t *testing.T, dir string) func() {
	t.Helper()

	previous, err := os.Getwd()
	if err != nil {
		t.Fatalf("Getwd failed: %v", err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatalf("Chdir failed: %v", err)
	}

	return func() {
		if err := os.Chdir(previous); err != nil {
			t.Fatalf("restore Chdir failed: %v", err)
		}
	}
}

func writeManifest(t *testing.T, path string, content string) {
	t.Helper()

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
}
