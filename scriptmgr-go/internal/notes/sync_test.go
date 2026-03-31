package notes

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestSyncDirection(t *testing.T) {
	if SyncBidirectional != "bidirectional" {
		t.Errorf("expected SyncBidirectional to be 'bidirectional', got %s", SyncBidirectional)
	}
	if SyncDBToFile != "db_to_file" {
		t.Errorf("expected SyncDBToFile to be 'db_to_file', got %s", SyncDBToFile)
	}
	if SyncFileToDB != "file_to_db" {
		t.Errorf("expected SyncFileToDB to be 'file_to_db', got %s", SyncFileToDB)
	}
}

func TestFileHandlerReadWriteNote(t *testing.T) {
	// Create temp directory
	tmpDir, err := os.MkdirTemp("", "notes-test-*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	handler := NewFileHandler(tmpDir)

	// Create a test note
	now := time.Now()
	note := &Note{
		ID:        "test-note-123",
		Title:     "Test Note",
		Content:   "# Test Note\n\nThis is test content.",
		Tags:      "test,example",
		CreatedAt: now,
		UpdatedAt: now,
		IsPinned:  false,
	}

	// Write the note
	if err := handler.WriteNote(note); err != nil {
		t.Fatalf("failed to write note: %v", err)
	}

	// Read it back
	readNote, err := handler.ReadNote(note.ID)
	if err != nil {
		t.Fatalf("failed to read note: %v", err)
	}

	if readNote.ID != note.ID {
		t.Errorf("expected ID %s, got %s", note.ID, readNote.ID)
	}
	if readNote.Title != note.Title {
		t.Errorf("expected Title %s, got %s", note.Title, readNote.Title)
	}
	if readNote.Content != note.Content {
		t.Errorf("expected Content %s, got %s", note.Content, readNote.Content)
	}
	if readNote.Tags != note.Tags {
		t.Errorf("expected Tags %s, got %s", note.Tags, readNote.Tags)
	}
}

func TestFileHandlerListNoteFiles(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "notes-test-*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	handler := NewFileHandler(tmpDir)

	// Initially should be empty
	ids, err := handler.ListNoteFiles()
	if err != nil {
		t.Fatalf("failed to list notes: %v", err)
	}
	if len(ids) != 0 {
		t.Errorf("expected empty list, got %d items", len(ids))
	}

	// Create some notes
	now := time.Now()
	for i := 1; i <= 3; i++ {
		note := &Note{
			ID:        string(rune('a' + i)),
			Title:     "Test",
			Content:   "Content",
			CreatedAt: now,
			UpdatedAt: now,
		}
		if err := handler.WriteNote(note); err != nil {
			t.Fatalf("failed to write note: %v", err)
		}
	}

	// Should list all 3
	ids, err = handler.ListNoteFiles()
	if err != nil {
		t.Fatalf("failed to list notes: %v", err)
	}
	if len(ids) != 3 {
		t.Errorf("expected 3 items, got %d", len(ids))
	}
}

func TestFileHandlerSearchNotes(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "notes-test-*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	handler := NewFileHandler(tmpDir)

	now := time.Now()
	notes := []*Note{
		{ID: "note-1", Title: "Apple Pie", Content: "Recipe for apple pie", Tags: "dessert", CreatedAt: now, UpdatedAt: now},
		{ID: "note-2", Title: "Banana Bread", Content: "Recipe for banana bread", Tags: "breakfast", CreatedAt: now, UpdatedAt: now},
		{ID: "note-3", Title: "Orange Juice", Content: "Fresh squeezed oranges", Tags: "drink,fruit", CreatedAt: now, UpdatedAt: now},
	}

	for _, n := range notes {
		if err := handler.WriteNote(n); err != nil {
			t.Fatalf("failed to write note: %v", err)
		}
	}

	// Search for "apple"
	results, err := handler.SearchNotes("apple")
	if err != nil {
		t.Fatalf("failed to search: %v", err)
	}
	if len(results) != 1 {
		t.Errorf("expected 1 result for 'apple', got %d", len(results))
	}

	// Search for "fruit"
	results, err = handler.SearchNotes("fruit")
	if err != nil {
		t.Fatalf("failed to search: %v", err)
	}
	if len(results) != 1 {
		t.Errorf("expected 1 result for 'fruit', got %d", len(results))
	}

	// Search for empty string should return all
	results, err = handler.SearchNotes("")
	if err != nil {
		t.Fatalf("failed to search: %v", err)
	}
	if len(results) != 3 {
		t.Errorf("expected 3 results for empty search, got %d", len(results))
	}
}

func TestParseNoteMarkdown(t *testing.T) {
	content := `---
id: test-id
title: Test Title
tags: tag1, tag2
created_at: 2026-03-19T10:00:00Z
updated_at: 2026-03-19T12:00:00Z
is_pinned: true
---

# Test Title

This is the content.`

	note, err := ParseNoteMarkdown("test-id", content)
	if err != nil {
		t.Fatalf("failed to parse: %v", err)
	}

	if note.ID != "test-id" {
		t.Errorf("expected ID 'test-id', got %s", note.ID)
	}
	if note.Title != "Test Title" {
		t.Errorf("expected Title 'Test Title', got %s", note.Title)
	}
	if note.Tags != "tag1, tag2" {
		t.Errorf("expected Tags 'tag1, tag2', got %s", note.Tags)
	}
	if !note.IsPinned {
		t.Error("expected IsPinned to be true")
	}
	if !note.ContentContains("This is the content") {
		t.Errorf("content should contain 'This is the content', got %s", note.Content)
	}
}

func TestParseNoteMarkdownNoFrontmatter(t *testing.T) {
	content := `# Hello World

This is a note without frontmatter.`

	note, err := ParseNoteMarkdown("auto-id", content)
	if err != nil {
		t.Fatalf("failed to parse: %v", err)
	}

	if note.Title != "Hello World" {
		t.Errorf("expected Title 'Hello World', got %s", note.Title)
	}
	if note.ID != "auto-id" {
		t.Errorf("expected ID 'auto-id', got %s", note.ID)
	}
}

func TestFormatNoteMarkdown(t *testing.T) {
	now := time.Date(2026, 3, 19, 10, 0, 0, 0, time.UTC)
	note := &Note{
		ID:        "test-123",
		Title:     "Test Title",
		Content:   "# Test Title\n\nContent here.",
		Tags:      "a, b",
		CreatedAt: now,
		UpdatedAt: now,
		IsPinned:  true,
	}

	result := FormatNoteMarkdown(note)

	if !contains(result, "id: test-123") {
		t.Error("formatted markdown should contain id")
	}
	if !contains(result, "title: Test Title") {
		t.Error("formatted markdown should contain title")
	}
	if !contains(result, "tags: a, b") {
		t.Error("formatted markdown should contain tags")
	}
	if !contains(result, "is_pinned: true") {
		t.Error("formatted markdown should contain is_pinned")
	}
	if !contains(result, "Content here.") {
		t.Error("formatted markdown should contain content")
	}
}

func TestConfigManager(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "notes-config-test-*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	cm := NewConfigManager(tmpDir)

	// Load config first
	if err := cm.Load(); err != nil {
		t.Fatalf("failed to load config: %v", err)
	}

	// Check default path
	defaultPath := cm.GetRepoPath()
	expectedDefault := filepath.Join(tmpDir, "notes")
	if defaultPath != expectedDefault {
		t.Errorf("expected default path %s, got %s", expectedDefault, defaultPath)
	}

	// Set new path
	newPath := filepath.Join(tmpDir, "custom-notes")
	if err := cm.SetRepoPath(newPath); err != nil {
		t.Fatalf("failed to set repo path: %v", err)
	}

	// Verify it was saved
	if cm.GetRepoPath() != newPath {
		t.Errorf("expected path %s, got %s", newPath, cm.GetRepoPath())
	}
}

// Helper functions
func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsHelper(s, substr))
}

func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func (n *Note) ContentContains(substr string) bool {
	return contains(n.Content, substr)
}
