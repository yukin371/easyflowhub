package extensions

import (
	"os"
	"path/filepath"
	"testing"
)

func TestRegistryListLoadsValidAndInvalidExtensions(t *testing.T) {
	root := t.TempDir()

	writeManifest(t, filepath.Join(root, "good", "plugin.json"), `{
  "id": "relay-openai",
  "name": "Relay OpenAI",
  "version": "0.1.0",
  "contributions": {
    "relay_providers": [
      { "id": "openai", "name": "OpenAI", "base_url": "https://api.openai.com" }
    ]
  }
}`)
	writeManifest(t, filepath.Join(root, "bad", "plugin.json"), `{"name":"Bad"}`)

	registry := NewRegistryWithRoots([]string{root})
	items, err := registry.List()
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}

	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	foundValid := false
	foundInvalid := false
	for _, item := range items {
		if item.Manifest != nil && item.Manifest.ID == "relay-openai" && item.Status == "loaded" {
			foundValid = true
		}
		if item.Status == "invalid" {
			foundInvalid = true
			if item.Error == "" {
				t.Fatal("expected invalid entry to include an error")
			}
		}
	}
	if !foundValid {
		t.Fatal("expected one valid relay-openai extension")
	}
	if !foundInvalid {
		t.Fatal("expected one invalid extension")
	}
}

func TestRegistryListMarksDuplicateIDsInvalid(t *testing.T) {
	rootA := t.TempDir()
	rootB := t.TempDir()

	content := `{"id":"duplicate","name":"One","version":"1.0.0"}`
	writeManifest(t, filepath.Join(rootA, "one", "plugin.json"), content)
	writeManifest(t, filepath.Join(rootB, "two", "plugin.json"), content)

	registry := NewRegistryWithRoots([]string{rootA, rootB})
	items, err := registry.List()
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}

	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	invalidCount := 0
	for _, item := range items {
		if item.Status == "invalid" {
			invalidCount++
		}
	}
	if invalidCount != 1 {
		t.Fatalf("expected 1 invalid duplicate, got %d", invalidCount)
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
