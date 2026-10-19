package api

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

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

func TestDeleteScriptManifestFailureRestoresOriginal(t *testing.T) {
	// F02: if writing entry.json fails after the script has already been
	// moved out of the user's root, the script must be moved back to its
	// original path. The original code used `os.RemoveAll(entryDir)` which
	// permanently lost the only copy.
	instance, root := newScriptFSTestAPI(t)
	record, err := instance.CreateScript(model.CreateScriptRequest{Root: root, Filename: "keepme.py", Content: "print(1)\n"})
	if err != nil {
		t.Fatalf("create: %v", err)
	}
	originalContent, err := os.ReadFile(record.Path)
	if err != nil {
		t.Fatalf("read original: %v", err)
	}

	// Inject a manifest write failure for the duration of this test.
	original := writeTrashManifest
	writeTrashManifest = func(string, model.ScriptTrashEntry) error {
		return errors.New("injected manifest write failure")
	}
	t.Cleanup(func() { writeTrashManifest = original })

	if _, err := instance.DeleteScript(record.ID); err == nil {
		t.Fatal("expected injected failure to surface")
	}

	// The original file must still exist with the original content.
	got, err := os.ReadFile(record.Path)
	if err != nil {
		t.Fatalf("script was not restored to its original path: %v", err)
	}
	if string(got) != string(originalContent) {
		t.Fatalf("restored content mismatch: got %q want %q", got, originalContent)
	}
	// Trash must not contain an orphan entry without its manifest.
	entries, err := instance.ListScriptTrash()
	if err != nil {
		t.Fatalf("list trash: %v", err)
	}
	if len(entries) != 0 {
		t.Fatalf("trash should be empty after rollback, got %+v", entries)
	}
	// Restore the writer so the retry below uses the real implementation.
	writeTrashManifest = original
	entry, err := instance.DeleteScript(record.ID)
	if err != nil {
		t.Fatalf("retry delete: %v", err)
	}
	if !fileExists(filepath.Join(instance.scriptTrashRoot(), entry.ID, filepath.Base(record.Path))) {
		t.Fatal("script should be in trash after retry")
	}
}

func TestDeleteScriptManifestFailurePreservesStagingWhenRollbackFails(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)
	record, err := instance.CreateScript(model.CreateScriptRequest{
		Root:     root,
		Filename: "keep-staged.py",
		Content:  "print(3)\n",
	})
	if err != nil {
		t.Fatalf("create: %v", err)
	}

	originalManifestWriter := writeTrashManifest
	writeTrashManifest = func(string, model.ScriptTrashEntry) error {
		return errors.New("injected manifest write failure")
	}
	originalMoveFile := moveFile
	moveCalls := 0
	moveFile = func(source, destination string) error {
		moveCalls++
		// The first call moves the script into staging. The second call is
		// the rollback; force it to fail so the staging copy must survive.
		if moveCalls == 2 {
			return errors.New("injected rollback failure")
		}
		return originalMoveFile(source, destination)
	}
	t.Cleanup(func() {
		writeTrashManifest = originalManifestWriter
		moveFile = originalMoveFile
	})

	if _, err := instance.DeleteScript(record.ID); err == nil {
		t.Fatal("expected manifest and rollback failures to surface")
	}
	if fileExists(record.Path) {
		t.Fatal("original path should remain absent when rollback fails")
	}

	dirs, err := os.ReadDir(instance.scriptTrashRoot())
	if err != nil {
		t.Fatalf("read trash root: %v", err)
	}
	if len(dirs) != 1 || !dirs[0].IsDir() {
		t.Fatalf("staging entry should be preserved, got %+v", dirs)
	}
	stagingScript := filepath.Join(instance.scriptTrashRoot(), dirs[0].Name(), filepath.Base(record.Path))
	content, err := os.ReadFile(stagingScript)
	if err != nil {
		t.Fatalf("staged script should remain recoverable: %v", err)
	}
	if string(content) != "print(3)\n" {
		t.Fatalf("staged content = %q", content)
	}
	t.Cleanup(func() { _ = os.RemoveAll(filepath.Join(instance.scriptTrashRoot(), dirs[0].Name())) })
}

func TestDeleteScriptMetadataFailurePreservesStagingWhenRollbackFails(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)
	record, err := instance.CreateScript(model.CreateScriptRequest{
		Root:     root,
		Filename: "keep-metadata-staged.py",
		Content:  "print(5)\n",
		Name:     "metadata",
	})
	if err != nil {
		t.Fatalf("create: %v", err)
	}

	originalMoveFile := moveFile
	moveCalls := 0
	moveFile = func(source, destination string) error {
		moveCalls++
		// Script move succeeds, metadata move fails, and the script rollback
		// fails. Both the script and metadata must remain recoverable.
		if moveCalls == 2 || moveCalls == 3 {
			return errors.New("injected move failure")
		}
		return originalMoveFile(source, destination)
	}
	t.Cleanup(func() { moveFile = originalMoveFile })

	if _, err := instance.DeleteScript(record.ID); err == nil {
		t.Fatal("expected metadata and rollback failures to surface")
	}
	if fileExists(record.Path) {
		t.Fatal("original path should remain absent when rollback fails")
	}
	if !fileExists(metadataPathFor(record.Path)) {
		t.Fatal("original metadata should remain untouched when metadata move fails")
	}

	dirs, err := os.ReadDir(instance.scriptTrashRoot())
	if err != nil {
		t.Fatalf("read trash root: %v", err)
	}
	if len(dirs) != 1 || !dirs[0].IsDir() {
		t.Fatalf("staging entry should be preserved, got %+v", dirs)
	}
	entryDir := filepath.Join(instance.scriptTrashRoot(), dirs[0].Name())
	stagingScript := filepath.Join(entryDir, filepath.Base(record.Path))
	if !fileExists(stagingScript) {
		t.Fatal("staged script should remain recoverable")
	}
	t.Cleanup(func() { _ = os.RemoveAll(entryDir) })
}

func TestListScriptTrashReportsCorruptEntriesWithoutSilentlyDroppingValidOnes(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)
	record, err := instance.CreateScript(model.CreateScriptRequest{
		Root:     root,
		Filename: "valid.py",
		Content:  "print(4)\n",
	})
	if err != nil {
		t.Fatalf("create: %v", err)
	}
	valid, err := instance.DeleteScript(record.ID)
	if err != nil {
		t.Fatalf("delete valid script: %v", err)
	}

	corruptID := "corrupt-entry"
	corruptDir := filepath.Join(instance.scriptTrashRoot(), corruptID)
	if err := os.MkdirAll(corruptDir, 0o755); err != nil {
		t.Fatalf("create corrupt entry: %v", err)
	}
	if err := os.WriteFile(filepath.Join(corruptDir, "entry.json"), []byte("{broken"), 0o644); err != nil {
		t.Fatalf("write corrupt manifest: %v", err)
	}
	t.Cleanup(func() { _ = os.RemoveAll(corruptDir) })

	entries, err := instance.ListScriptTrash()
	if err == nil || !strings.Contains(err.Error(), corruptID) {
		t.Fatalf("expected explicit corrupt-entry error, got %v", err)
	}
	if len(entries) != 1 || entries[0].ID != valid.ID {
		t.Fatalf("valid entries should remain available to callers, got %+v", entries)
	}
}

func TestWriteTrashManifestIsAtomic(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "entry.json")
	entry := model.ScriptTrashEntry{
		ID:           "atomic-entry",
		Name:         "atomic",
		OriginalPath: filepath.Join(root, "atomic.py"),
		ScriptType:   "python",
		DeletedAt:    nowRFC3339(),
	}
	if err := writeTrashManifest(path, entry); err != nil {
		t.Fatalf("write manifest: %v", err)
	}
	decoded, err := readTrashEntry(path)
	if err != nil {
		t.Fatalf("read manifest: %v", err)
	}
	if decoded.ID != entry.ID || decoded.OriginalPath != entry.OriginalPath {
		t.Fatalf("manifest = %+v, want %+v", decoded, entry)
	}
	items, err := os.ReadDir(root)
	if err != nil {
		t.Fatalf("read manifest directory: %v", err)
	}
	if len(items) != 1 || items[0].Name() != "entry.json" {
		t.Fatalf("atomic temp file leaked: %+v", items)
	}
}

func TestRestoreScriptTrashMetadataFailureLeavesEntryIntact(t *testing.T) {
	// F02: when restoring a script whose metadata cannot be moved back to
	// its original path, the entry directory must remain on disk so the
	// metadata is recoverable. The script itself is already back in place.
	instance, root := newScriptFSTestAPI(t)
	record, err := instance.CreateScript(model.CreateScriptRequest{
		Root:     root,
		Filename: "restore.py",
		Content:  "print(2)\n",
		Name:     "restorable",
	})
	if err != nil {
		t.Fatalf("create: %v", err)
	}
	metaPath := metadataPathFor(record.Path)
	if !fileExists(metaPath) {
		t.Fatalf("expected metadata at %s after CreateScript", metaPath)
	}

	entry, err := instance.DeleteScript(record.ID)
	if err != nil {
		t.Fatalf("delete: %v", err)
	}

	// Replace the original metadata path with a directory so the restore's
	// moveFile falls back to copy+remove and both paths fail: the rename
	// target is a directory and the copy+remove path uses O_EXCL.
	// DeleteScript already moved the file away, so Remove may not find it.
	if err := os.Remove(metaPath); err != nil && !os.IsNotExist(err) {
		t.Fatalf("remove meta file: %v", err)
	}
	if err := os.MkdirAll(metaPath, 0o755); err != nil {
		t.Fatalf("seed meta path as a directory: %v", err)
	}
	t.Cleanup(func() { _ = os.RemoveAll(metaPath) })

	_, err = instance.RestoreScriptTrash(entry.ID)
	if err == nil {
		t.Fatal("expected metadata move failure to surface")
	}
	// Script must have been moved back to its original path even though
	// the metadata move failed.
	if !fileExists(record.Path) {
		t.Fatal("script should have been restored to its original path")
	}
	// Entry directory must still exist so the metadata is recoverable.
	entryDir := filepath.Join(instance.scriptTrashRoot(), entry.ID)
	if info, err := os.Stat(entryDir); err != nil || !info.IsDir() {
		t.Fatalf("entry directory should be preserved when metadata restore fails (stat=%v)", err)
	}
	// This test deliberately leaves files behind; remove them with a retry so
	// a transient Windows handle does not fail the TempDir cleanup instead.
	t.Cleanup(func() {
		for attempt := 0; attempt < 5; attempt++ {
			if err := os.RemoveAll(entryDir); err == nil {
				return
			}
			time.Sleep(50 * time.Millisecond)
		}
		_ = os.RemoveAll(entryDir)
	})
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
