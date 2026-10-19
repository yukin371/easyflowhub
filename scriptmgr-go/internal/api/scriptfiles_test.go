package api

import (
	"os"
	"path/filepath"
	"testing"

	"scriptmgr/internal/config"
	"scriptmgr/internal/discovery"
	"scriptmgr/internal/executor"
	"scriptmgr/internal/model"
	"scriptmgr/internal/store"
)

func newScriptFSTestAPI(t *testing.T) (*API, string) {
	t.Helper()
	stateDir := t.TempDir()
	cfg := config.Config{StateDir: stateDir}
	s := store.New(cfg)
	d := discovery.NewWithExtensionsRegistry(s, nil)
	e := executor.New(s, d)
	o, err := store.NewOverrideStore(stateDir)
	if err != nil {
		t.Fatalf("override store: %v", err)
	}
	instance := New(d, e, s, o)

	root := filepath.Join(t.TempDir(), "scripts")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("create root: %v", err)
	}
	if _, err := instance.AddRoot(root); err != nil {
		t.Fatalf("add root: %v", err)
	}
	return instance, root
}

func TestCreateReadAndWriteScript(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)

	record, err := instance.CreateScript(model.CreateScriptRequest{
		Root:        root,
		Filename:    "hello.py",
		Content:     "print('hi')\n",
		Name:        "Hello",
		Category:    "demo",
		Description: "demo script",
	})
	if err != nil {
		t.Fatalf("create script: %v", err)
	}
	target := filepath.Join(root, "hello.py")
	if !samePathFold(record.Path, target) {
		t.Fatalf("created path = %q, want %q", record.Path, target)
	}
	if record.Name != "Hello" || record.Category != "demo" {
		t.Fatalf("metadata not applied: %+v", record)
	}
	if !fileExists(metadataPathFor(target)) {
		t.Fatal("metadata sidecar was not written")
	}

	source, err := instance.ReadScriptSource(record.ID)
	if err != nil {
		t.Fatalf("read source: %v", err)
	}
	if source.Content != "print('hi')\n" || source.Language != "python" {
		t.Fatalf("unexpected source: %+v", source)
	}

	if _, err := instance.WriteScriptSource(record.ID, "print('bye')\n"); err != nil {
		t.Fatalf("write source: %v", err)
	}
	updated, err := instance.ReadScriptSource(record.ID)
	if err != nil {
		t.Fatalf("read updated source: %v", err)
	}
	if updated.Content != "print('bye')\n" {
		t.Fatalf("content not saved: %q", updated.Content)
	}
}

func TestCreateScriptRejectsUnsafeInput(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)

	cases := []struct {
		name string
		req  model.CreateScriptRequest
	}{
		{"path traversal", model.CreateScriptRequest{Root: root, Filename: "../escape.py", Content: "x"}},
		{"unsupported extension", model.CreateScriptRequest{Root: root, Filename: "payload.exe", Content: "x"}},
		{"unknown root", model.CreateScriptRequest{Root: filepath.Join(root, "nested"), Filename: "ok.py", Content: "x"}},
		{"empty filename", model.CreateScriptRequest{Root: root, Filename: "  ", Content: "x"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := instance.CreateScript(tc.req); err == nil {
				t.Fatalf("expected rejection for %s", tc.name)
			}
		})
	}

	if _, err := instance.CreateScript(model.CreateScriptRequest{Root: root, Filename: "dup.py", Content: "x"}); err != nil {
		t.Fatalf("first create: %v", err)
	}
	if _, err := instance.CreateScript(model.CreateScriptRequest{Root: root, Filename: "dup.py", Content: "y"}); err == nil {
		t.Fatal("expected duplicate filename rejection")
	}
}

func TestDeleteRestoreAndPurgeScript(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)
	record, err := instance.CreateScript(model.CreateScriptRequest{Root: root, Filename: "trash-me.py", Content: "print(1)\n"})
	if err != nil {
		t.Fatalf("create script: %v", err)
	}

	entry, err := instance.DeleteScript(record.ID)
	if err != nil {
		t.Fatalf("delete script: %v", err)
	}
	if fileExists(record.Path) {
		t.Fatal("script file should have been moved to trash")
	}
	entries, err := instance.ListScriptTrash()
	if err != nil {
		t.Fatalf("list trash: %v", err)
	}
	if len(entries) != 1 || entries[0].ID != entry.ID {
		t.Fatalf("unexpected trash entries: %+v", entries)
	}

	if _, err := instance.RestoreScriptTrash(entry.ID); err != nil {
		t.Fatalf("restore: %v", err)
	}
	if !fileExists(record.Path) {
		t.Fatal("script file was not restored")
	}

	restored, err := instance.DescribeScript(record.ID)
	if err != nil {
		t.Fatalf("describe restored script: %v", err)
	}
	entry, err = instance.DeleteScript(restored.ID)
	if err != nil {
		t.Fatalf("delete again: %v", err)
	}
	if err := instance.PurgeScriptTrash(entry.ID); err != nil {
		t.Fatalf("purge: %v", err)
	}
	entries, err = instance.ListScriptTrash()
	if err != nil {
		t.Fatalf("list trash after purge: %v", err)
	}
	if len(entries) != 0 {
		t.Fatalf("trash should be empty, got %+v", entries)
	}
}

func TestEmptyScriptTrashReportsRemovedCount(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)
	created := make([]model.ScriptRecord, 0, 2)
	for _, name := range []string{"one.py", "two.ps1"} {
		record, err := instance.CreateScript(model.CreateScriptRequest{Root: root, Filename: name, Content: "# x"})
		if err != nil {
			t.Fatalf("create %s: %v", name, err)
		}
		created = append(created, record)
	}
	// Only touch the scripts this test created: discovery also reports the
	// user's real roots, which must never be modified by a test.
	for _, record := range created {
		if _, err := instance.DeleteScript(record.ID); err != nil {
			t.Fatalf("delete %s: %v", record.ID, err)
		}
	}
	removed, err := instance.EmptyScriptTrash()
	if err != nil {
		t.Fatalf("empty trash: %v", err)
	}
	if removed != 2 {
		t.Fatalf("removed = %d, want 2", removed)
	}
}

func TestImportScriptsCopiesAndSkipsConflicts(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)
	sourceDir := t.TempDir()

	fresh := filepath.Join(sourceDir, "imported.ps1")
	if err := os.WriteFile(fresh, []byte("Write-Host hi\n"), 0o644); err != nil {
		t.Fatalf("write source: %v", err)
	}
	conflict := filepath.Join(sourceDir, "taken.ps1")
	if err := os.WriteFile(conflict, []byte("Write-Host other\n"), 0o644); err != nil {
		t.Fatalf("write conflict source: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "taken.ps1"), []byte("existing\n"), 0o644); err != nil {
		t.Fatalf("write existing target: %v", err)
	}
	unsupported := filepath.Join(sourceDir, "notes.txt")
	if err := os.WriteFile(unsupported, []byte("plain text"), 0o644); err != nil {
		t.Fatalf("write unsupported: %v", err)
	}

	response, err := instance.ImportScripts(model.ImportScriptsRequest{
		Root:  root,
		Paths: []string{fresh, conflict, unsupported},
	})
	if err != nil {
		t.Fatalf("import: %v", err)
	}
	if len(response.Imported) != 1 {
		t.Fatalf("imported = %d, want 1 (%+v)", len(response.Imported), response.Failed)
	}
	if len(response.Failed) != 2 {
		t.Fatalf("failed = %d, want 2", len(response.Failed))
	}
	content, err := os.ReadFile(filepath.Join(root, "imported.ps1"))
	if err != nil {
		t.Fatalf("read imported: %v", err)
	}
	if string(content) != "Write-Host hi\n" {
		t.Fatalf("imported content = %q", content)
	}
	existing, err := os.ReadFile(filepath.Join(root, "taken.ps1"))
	if err != nil {
		t.Fatalf("read existing: %v", err)
	}
	if string(existing) != "existing\n" {
		t.Fatal("import must not overwrite an existing file")
	}
}

func TestRootDetailsSeparateCustomFromDiscovered(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)

	roots, custom, err := instance.ListRootDetails()
	if err != nil {
		t.Fatalf("list roots: %v", err)
	}
	if !rootAllowed(root, roots) {
		t.Fatalf("discovered roots %v should contain %q", roots, root)
	}
	if !rootAllowed(root, custom) {
		t.Fatalf("custom roots %v should contain %q", custom, root)
	}

	if err := instance.RemoveRoot(root); err != nil {
		t.Fatalf("remove root: %v", err)
	}
	_, custom, err = instance.ListRootDetails()
	if err != nil {
		t.Fatalf("list roots after removal: %v", err)
	}
	if rootAllowed(root, custom) {
		t.Fatalf("root should be unpinned, got %v", custom)
	}
}
