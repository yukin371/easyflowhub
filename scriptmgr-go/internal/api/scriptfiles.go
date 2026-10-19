package api

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"scriptmgr/internal/model"
)

// maxScriptSourceBytes bounds what the editor will read or write. Scripts are
// text; a multi-megabyte file is a mistake, not a script.
const maxScriptSourceBytes = 2 << 20

var scriptLanguages = map[string]string{
	".ps1": "powershell",
	".py":  "python",
	".bat": "batch",
	".cmd": "batch",
}

func scriptLanguageFor(name string) (string, bool) {
	language, ok := scriptLanguages[strings.ToLower(filepath.Ext(name))]
	return language, ok
}

func metadataPathFor(scriptPath string) string {
	return strings.TrimSuffix(scriptPath, filepath.Ext(scriptPath)) + ".json"
}

func samePathFold(a, b string) bool {
	return strings.EqualFold(filepath.Clean(a), filepath.Clean(b))
}

// pathInsideRoot resolves a path only when it lives inside one of the
// configured script roots. This is the write barrier for every file mutation.
func pathInsideRoot(path string, roots []string) (string, bool) {
	cleaned := filepath.Clean(path)
	for _, root := range roots {
		rel, err := filepath.Rel(filepath.Clean(root), cleaned)
		if err != nil {
			continue
		}
		if rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) {
			continue
		}
		return cleaned, true
	}
	return "", false
}

// ListRootDetails returns every discovered root plus the user-pinned subset.
// Auto-discovered roots follow the process working directory; only pinned roots
// survive a different launch context.
func (a *API) ListRootDetails() ([]string, []string, error) {
	roots, err := a.discovery.ScriptRoots()
	if err != nil {
		return nil, nil, err
	}
	custom, err := a.store.LoadRoots()
	if err != nil {
		return nil, nil, err
	}
	return roots, custom.Roots, nil
}

// ReadScriptSource returns the editable text of a discovered script.
func (a *API) ReadScriptSource(scriptID string) (model.ScriptSource, error) {
	record, err := a.DescribeScript(scriptID)
	if err != nil {
		return model.ScriptSource{}, err
	}
	roots, err := a.discovery.ScriptRoots()
	if err != nil {
		return model.ScriptSource{}, err
	}
	path, ok := pathInsideRoot(record.Path, roots)
	if !ok {
		return model.ScriptSource{}, fmt.Errorf("script is outside the configured roots: %s", record.Path)
	}
	info, err := os.Stat(path)
	if err != nil {
		return model.ScriptSource{}, err
	}
	if !info.Mode().IsRegular() {
		return model.ScriptSource{}, errors.New("script is not a regular file")
	}
	if info.Size() > maxScriptSourceBytes {
		return model.ScriptSource{}, fmt.Errorf("script exceeds %d bytes", maxScriptSourceBytes)
	}
	content, err := os.ReadFile(path)
	if err != nil {
		return model.ScriptSource{}, err
	}

	source := model.ScriptSource{
		ID:         record.ID,
		Path:       path,
		Content:    string(content),
		Language:   record.ScriptType,
		SizeBytes:  info.Size(),
		ModifiedAt: info.ModTime().UTC().Format(time.RFC3339),
	}
	if meta := metadataPathFor(path); fileExists(meta) {
		source.MetadataPath = meta
	}
	return source, nil
}

// WriteScriptSource overwrites a script in place. The write is atomic so a
// failure cannot leave a truncated script behind.
func (a *API) WriteScriptSource(scriptID, content string) (model.ScriptSource, error) {
	if len(content) > maxScriptSourceBytes {
		return model.ScriptSource{}, fmt.Errorf("content exceeds %d bytes", maxScriptSourceBytes)
	}
	record, err := a.DescribeScript(scriptID)
	if err != nil {
		return model.ScriptSource{}, err
	}
	roots, err := a.discovery.ScriptRoots()
	if err != nil {
		return model.ScriptSource{}, err
	}
	path, ok := pathInsideRoot(record.Path, roots)
	if !ok {
		return model.ScriptSource{}, fmt.Errorf("script is outside the configured roots: %s", record.Path)
	}
	info, err := os.Stat(path)
	if err != nil {
		return model.ScriptSource{}, err
	}
	perm := info.Mode().Perm()
	if perm == 0 {
		perm = 0o644
	}

	tempPath := path + ".tmp"
	if err := os.WriteFile(tempPath, []byte(content), perm); err != nil {
		return model.ScriptSource{}, err
	}
	if err := os.Rename(tempPath, path); err != nil {
		_ = os.Remove(tempPath)
		return model.ScriptSource{}, err
	}
	return a.ReadScriptSource(scriptID)
}

// CreateScript writes a new script into a configured root and returns the
// discovered record so the UI can select it immediately.
func (a *API) CreateScript(req model.CreateScriptRequest) (model.ScriptRecord, error) {
	root := filepath.Clean(strings.TrimSpace(req.Root))
	if root == "" {
		return model.ScriptRecord{}, errors.New("root is required")
	}
	roots, err := a.discovery.ScriptRoots()
	if err != nil {
		return model.ScriptRecord{}, err
	}
	if !rootAllowed(root, roots) {
		return model.ScriptRecord{}, fmt.Errorf("root is not a configured script root: %s", root)
	}

	filename := strings.TrimSpace(req.Filename)
	if filename == "" {
		return model.ScriptRecord{}, errors.New("filename is required")
	}
	if filepath.Base(filename) != filename {
		return model.ScriptRecord{}, errors.New("filename must not contain path separators")
	}
	if _, ok := scriptLanguageFor(filename); !ok {
		return model.ScriptRecord{}, fmt.Errorf("unsupported script extension: %s", filepath.Ext(filename))
	}
	if len(req.Content) > maxScriptSourceBytes {
		return model.ScriptRecord{}, fmt.Errorf("content exceeds %d bytes", maxScriptSourceBytes)
	}
	if err := os.MkdirAll(root, 0o755); err != nil {
		return model.ScriptRecord{}, err
	}

	target := filepath.Join(root, filename)
	if fileExists(target) {
		return model.ScriptRecord{}, fmt.Errorf("file already exists: %s", filename)
	}
	if err := os.WriteFile(target, []byte(req.Content), 0o644); err != nil {
		return model.ScriptRecord{}, err
	}

	name := strings.TrimSpace(req.Name)
	category := strings.TrimSpace(req.Category)
	description := strings.TrimSpace(req.Description)
	if name != "" || category != "" || description != "" {
		metadata := model.ScriptMetadata{
			Name:        name,
			Category:    category,
			Description: description,
		}
		if err := writeJSONFile(metadataPathFor(target), metadata); err != nil {
			_ = os.Remove(target)
			return model.ScriptRecord{}, err
		}
	}

	record, err := a.recordForPath(target)
	if err != nil {
		return model.ScriptRecord{}, err
	}
	return record, nil
}

// ImportScripts copies external script files (and their sidecar metadata) into
// a configured root. Existing files are never overwritten.
func (a *API) ImportScripts(req model.ImportScriptsRequest) (model.ImportScriptsResponse, error) {
	root := filepath.Clean(strings.TrimSpace(req.Root))
	roots, err := a.discovery.ScriptRoots()
	if err != nil {
		return model.ImportScriptsResponse{}, err
	}
	if !rootAllowed(root, roots) {
		return model.ImportScriptsResponse{}, fmt.Errorf("root is not a configured script root: %s", root)
	}
	if err := os.MkdirAll(root, 0o755); err != nil {
		return model.ImportScriptsResponse{}, err
	}

	response := model.ImportScriptsResponse{
		OK:          true,
		Imported:    []model.ScriptRecord{},
		Failed:      []model.ImportFailure{},
		GeneratedAt: nowRFC3339(),
	}

	for _, raw := range req.Paths {
		source := filepath.Clean(strings.TrimSpace(raw))
		if source == "" {
			continue
		}
		if _, ok := scriptLanguageFor(source); !ok {
			response.Failed = append(response.Failed, model.ImportFailure{Path: raw, Error: "unsupported script extension"})
			continue
		}
		info, err := os.Stat(source)
		if err != nil || !info.Mode().IsRegular() {
			response.Failed = append(response.Failed, model.ImportFailure{Path: raw, Error: "not a regular file"})
			continue
		}
		destination := filepath.Join(root, filepath.Base(source))
		if samePathFold(source, destination) {
			response.Failed = append(response.Failed, model.ImportFailure{Path: raw, Error: "file already lives in this root"})
			continue
		}
		if fileExists(destination) {
			response.Failed = append(response.Failed, model.ImportFailure{Path: raw, Error: "target file already exists"})
			continue
		}
		if err := copyFile(source, destination); err != nil {
			response.Failed = append(response.Failed, model.ImportFailure{Path: raw, Error: err.Error()})
			continue
		}
		if meta := metadataPathFor(source); fileExists(meta) {
			_ = copyFile(meta, metadataPathFor(destination))
		}
		if record, err := a.recordForPath(destination); err == nil {
			response.Imported = append(response.Imported, record)
		}
	}

	return response, nil
}

// DeleteScript moves a script (and its metadata) into the trash directory.
func (a *API) DeleteScript(scriptID string) (model.ScriptTrashEntry, error) {
	record, err := a.DescribeScript(scriptID)
	if err != nil {
		return model.ScriptTrashEntry{}, err
	}
	roots, err := a.discovery.ScriptRoots()
	if err != nil {
		return model.ScriptTrashEntry{}, err
	}
	path, ok := pathInsideRoot(record.Path, roots)
	if !ok {
		return model.ScriptTrashEntry{}, fmt.Errorf("script is outside the configured roots: %s", record.Path)
	}
	info, err := os.Stat(path)
	if err != nil {
		return model.ScriptTrashEntry{}, err
	}

	entryID, err := newTrashID()
	if err != nil {
		return model.ScriptTrashEntry{}, err
	}
	entryDir := filepath.Join(a.scriptTrashRoot(), entryID)
	if err := os.MkdirAll(entryDir, 0o755); err != nil {
		return model.ScriptTrashEntry{}, err
	}

	entry := model.ScriptTrashEntry{
		ID:           entryID,
		Name:         record.Name,
		OriginalPath: path,
		ScriptType:   record.ScriptType,
		DeletedAt:    nowRFC3339(),
		SizeBytes:    info.Size(),
	}

	fileName := filepath.Base(path)
	if err := moveFile(path, filepath.Join(entryDir, fileName)); err != nil {
		_ = os.RemoveAll(entryDir)
		return model.ScriptTrashEntry{}, err
	}
	if meta := metadataPathFor(path); fileExists(meta) {
		_ = moveFile(meta, filepath.Join(entryDir, filepath.Base(meta)))
	}
	if err := writeJSONFile(filepath.Join(entryDir, "entry.json"), entry); err != nil {
		_ = os.RemoveAll(entryDir)
		return model.ScriptTrashEntry{}, err
	}
	return entry, nil
}

// ListScriptTrash returns trashed scripts, newest first.
func (a *API) ListScriptTrash() ([]model.ScriptTrashEntry, error) {
	root := a.scriptTrashRoot()
	dirs, err := os.ReadDir(root)
	if err != nil {
		if os.IsNotExist(err) {
			return []model.ScriptTrashEntry{}, nil
		}
		return nil, err
	}

	entries := make([]model.ScriptTrashEntry, 0, len(dirs))
	for _, dir := range dirs {
		if !dir.IsDir() {
			continue
		}
		entry, err := readTrashEntry(filepath.Join(root, dir.Name(), "entry.json"))
		if err != nil {
			continue
		}
		entries = append(entries, entry)
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].DeletedAt > entries[j].DeletedAt })
	return entries, nil
}

// RestoreScriptTrash moves a trashed script back to its original path.
func (a *API) RestoreScriptTrash(entryID string) (model.ScriptTrashEntry, error) {
	if !validTrashID(entryID) {
		return model.ScriptTrashEntry{}, errors.New("invalid trash entry id")
	}
	entryDir := filepath.Join(a.scriptTrashRoot(), entryID)
	entry, err := readTrashEntry(filepath.Join(entryDir, "entry.json"))
	if err != nil {
		return model.ScriptTrashEntry{}, fmt.Errorf("trash entry not found: %w", err)
	}
	if fileExists(entry.OriginalPath) {
		return model.ScriptTrashEntry{}, fmt.Errorf("a file already exists at %s", entry.OriginalPath)
	}
	if err := os.MkdirAll(filepath.Dir(entry.OriginalPath), 0o755); err != nil {
		return model.ScriptTrashEntry{}, err
	}

	fileName := filepath.Base(entry.OriginalPath)
	if err := moveFile(filepath.Join(entryDir, fileName), entry.OriginalPath); err != nil {
		return model.ScriptTrashEntry{}, err
	}
	metaName := filepath.Base(metadataPathFor(entry.OriginalPath))
	if fileExists(filepath.Join(entryDir, metaName)) {
		_ = moveFile(filepath.Join(entryDir, metaName), metadataPathFor(entry.OriginalPath))
	}
	_ = os.RemoveAll(entryDir)
	return entry, nil
}

// PurgeScriptTrash permanently deletes one trashed script.
func (a *API) PurgeScriptTrash(entryID string) error {
	if !validTrashID(entryID) {
		return errors.New("invalid trash entry id")
	}
	entryDir := filepath.Join(a.scriptTrashRoot(), entryID)
	if !fileExists(filepath.Join(entryDir, "entry.json")) {
		return errors.New("trash entry not found")
	}
	return os.RemoveAll(entryDir)
}

// EmptyScriptTrash permanently deletes every trashed script and reports how
// many entries were removed.
func (a *API) EmptyScriptTrash() (int, error) {
	entries, err := a.ListScriptTrash()
	if err != nil {
		return 0, err
	}
	if err := os.RemoveAll(a.scriptTrashRoot()); err != nil {
		return 0, err
	}
	return len(entries), nil
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

func (a *API) scriptTrashRoot() string {
	return filepath.Join(a.StateDir(), "trash", "scripts")
}

func (a *API) recordForPath(path string) (model.ScriptRecord, error) {
	scripts, err := a.discovery.DiscoverScripts("")
	if err != nil {
		return model.ScriptRecord{}, err
	}
	cleaned := filepath.Clean(path)
	for _, script := range scripts {
		if strings.EqualFold(filepath.Clean(script.Path), cleaned) {
			return script, nil
		}
	}
	return model.ScriptRecord{}, errors.New("script is not discoverable from the configured roots")
}

func rootAllowed(root string, roots []string) bool {
	for _, candidate := range roots {
		if samePathFold(candidate, root) {
			return true
		}
	}
	return false
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular()
}

func copyFile(source, destination string) error {
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	defer input.Close()

	output, err := os.OpenFile(destination, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o644)
	if err != nil {
		return err
	}
	if _, err := io.Copy(output, input); err != nil {
		output.Close()
		_ = os.Remove(destination)
		return err
	}
	return output.Close()
}

// moveFile renames when possible and falls back to copy+remove when the source
// and destination live on different volumes (script roots are frequently on a
// different drive than %APPDATA%).
func moveFile(source, destination string) error {
	if err := os.Rename(source, destination); err == nil {
		return nil
	}
	if err := copyFile(source, destination); err != nil {
		return err
	}
	return os.Remove(source)
}

func writeJSONFile(path string, value any) error {
	content, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, content, 0o644)
}

func readTrashEntry(path string) (model.ScriptTrashEntry, error) {
	var entry model.ScriptTrashEntry
	content, err := os.ReadFile(path)
	if err != nil {
		return entry, err
	}
	if err := json.Unmarshal(content, &entry); err != nil {
		return entry, err
	}
	if entry.ID == "" {
		return entry, errors.New("trash entry is missing an id")
	}
	return entry, nil
}

func newTrashID() (string, error) {
	buffer := make([]byte, 6)
	if _, err := rand.Read(buffer); err != nil {
		return "", err
	}
	return time.Now().Format("20060102-150405") + "-" + hex.EncodeToString(buffer), nil
}

func validTrashID(id string) bool {
	return id != "" && filepath.Base(id) == id && !strings.ContainsAny(id, "\\/")
}
