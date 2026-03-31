package notes

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"
)

// FileHandler handles reading and writing note files
type FileHandler struct {
	repoPath string
}

// NewFileHandler creates a new file handler
func NewFileHandler(repoPath string) *FileHandler {
	return &FileHandler{repoPath: repoPath}
}

// EnsureRepoDir ensures the repo directory exists
func (h *FileHandler) EnsureRepoDir() error {
	return os.MkdirAll(h.repoPath, 0755)
}

// NoteFilePath returns the file path for a note
func (h *FileHandler) NoteFilePath(noteID string) string {
	return filepath.Join(h.repoPath, noteID+".md")
}

// ReadNote reads a note from a markdown file
func (h *FileHandler) ReadNote(noteID string) (*Note, error) {
	path := h.NoteFilePath(noteID)
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	return ParseNoteMarkdown(noteID, string(data))
}

// WriteNote writes a note to a markdown file
func (h *FileHandler) WriteNote(note *Note) error {
	if err := h.EnsureRepoDir(); err != nil {
		return err
	}

	content := FormatNoteMarkdown(note)
	path := h.NoteFilePath(note.ID)
	return os.WriteFile(path, []byte(content), 0644)
}

// DeleteNote deletes a note file
func (h *FileHandler) DeleteNote(noteID string) error {
	path := h.NoteFilePath(noteID)
	return os.Remove(path)
}

// ListNoteFiles lists all note files in the repo
func (h *FileHandler) ListNoteFiles() ([]string, error) {
	entries, err := os.ReadDir(h.repoPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	var ids []string
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".md") {
			id := strings.TrimSuffix(entry.Name(), ".md")
			ids = append(ids, id)
		}
	}
	return ids, nil
}

// ParseNoteMarkdown parses a markdown file with YAML frontmatter
func ParseNoteMarkdown(noteID, content string) (*Note, error) {
	note := &Note{
		ID: noteID,
	}

	// Check for frontmatter
	if !strings.HasPrefix(content, "---\n") {
		// No frontmatter, treat entire content as note body
		note.Content = strings.TrimSpace(content)
		note.Title = extractTitle(content)
		note.CreatedAt = time.Now()
		note.UpdatedAt = time.Now()
		return note, nil
	}

	// Find end of frontmatter
	endIdx := strings.Index(content[4:], "\n---\n")
	if endIdx == -1 {
		return nil, fmt.Errorf("invalid frontmatter: missing closing ---")
	}
	endIdx += 4 // offset for the first 4 characters

	frontmatter := content[4:endIdx]
	body := strings.TrimSpace(content[endIdx+5:])

	// Parse frontmatter
	parseFrontmatter(frontmatter, note)

	// Parse body
	note.Content = body
	if note.Title == "" {
		note.Title = extractTitle(body)
	}

	return note, nil
}

// parseFrontmatter parses YAML frontmatter into a note
func parseFrontmatter(frontmatter string, note *Note) {
	lines := strings.Split(frontmatter, "\n")
	for _, line := range lines {
		if idx := strings.Index(line, ":"); idx != -1 {
			key := strings.TrimSpace(line[:idx])
			value := strings.TrimSpace(line[idx+1:])

			switch key {
			case "id":
				if value != "" && note.ID == "" {
					note.ID = value
				}
			case "title":
				note.Title = value
			case "tags":
				note.Tags = value
			case "created_at":
				if t, err := time.Parse(time.RFC3339, value); err == nil {
					note.CreatedAt = t
				}
			case "updated_at":
				if t, err := time.Parse(time.RFC3339, value); err == nil {
					note.UpdatedAt = t
				}
			case "is_pinned":
				note.IsPinned = value == "true"
			}
		}
	}
}

// FormatNoteMarkdown formats a note as markdown with YAML frontmatter
func FormatNoteMarkdown(note *Note) string {
	var sb strings.Builder

	sb.WriteString("---\n")
	sb.WriteString(fmt.Sprintf("id: %s\n", note.ID))
	sb.WriteString(fmt.Sprintf("title: %s\n", note.Title))
	sb.WriteString(fmt.Sprintf("tags: %s\n", note.Tags))
	sb.WriteString(fmt.Sprintf("created_at: %s\n", note.CreatedAt.Format(time.RFC3339)))
	sb.WriteString(fmt.Sprintf("updated_at: %s\n", note.UpdatedAt.Format(time.RFC3339)))
	sb.WriteString(fmt.Sprintf("is_pinned: %v\n", note.IsPinned))
	sb.WriteString("---\n\n")

	if note.Content != "" {
		sb.WriteString(note.Content)
		sb.WriteString("\n")
	}

	return sb.String()
}

// extractTitle extracts the title from the first heading or first line
func extractTitle(content string) string {
	scanner := bufio.NewScanner(strings.NewReader(content))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}

		// Check for heading
		if strings.HasPrefix(line, "# ") {
			return strings.TrimSpace(line[2:])
		}

		// Use first non-empty line as title
		if len(line) > 50 {
			return line[:50] + "..."
		}
		return line
	}
	return "Untitled"
}

// RepoExists checks if the repo directory exists
func (h *FileHandler) RepoExists() bool {
	info, err := os.Stat(h.repoPath)
	return err == nil && info.IsDir()
}

// ReadAllNotes reads all notes from the repo
func (h *FileHandler) ReadAllNotes() ([]*Note, error) {
	ids, err := h.ListNoteFiles()
	if err != nil {
		return nil, err
	}

	var notes []*Note
	for _, id := range ids {
		note, err := h.ReadNote(id)
		if err != nil {
			continue // skip problematic files
		}
		notes = append(notes, note)
	}
	return notes, nil
}

// SearchNotes searches notes by title, content or tags
func (h *FileHandler) SearchNotes(query string) ([]*Note, error) {
	notes, err := h.ReadAllNotes()
	if err != nil {
		return nil, err
	}

	if query == "" {
		return notes, nil
	}

	query = strings.ToLower(query)
	var results []*Note
	for _, note := range notes {
		if strings.Contains(strings.ToLower(note.Title), query) ||
			strings.Contains(strings.ToLower(note.Content), query) ||
			strings.Contains(strings.ToLower(note.Tags), query) {
			results = append(results, note)
		}
	}
	return results, nil
}

// used for title extraction
var headingRegex = regexp.MustCompile(`^#\s+(.+)$`)
