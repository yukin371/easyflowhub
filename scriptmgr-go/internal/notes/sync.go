package notes

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"time"

	_ "modernc.org/sqlite"
)

// Syncer handles synchronization between DB and file repo
type Syncer struct {
	dbPath     string
	fileHandler *FileHandler
}

// NewSyncer creates a new syncer
func NewSyncer(appDataDir string, fileHandler *FileHandler) *Syncer {
	return &Syncer{
		dbPath:      filepath.Join(appDataDir, "notes.db"),
		fileHandler: fileHandler,
	}
}

// Sync performs synchronization based on direction
func (s *Syncer) Sync(direction SyncDirection) (*SyncReport, error) {
	report := &SyncReport{}

	switch direction {
	case SyncDBToFile:
		return s.syncDBToFile(report)
	case SyncFileToDB:
		return s.syncFileToDB(report)
	case SyncBidirectional:
		return s.syncBidirectional(report)
	default:
		return nil, fmt.Errorf("unknown sync direction: %s", direction)
	}
}

// syncDBToFile exports all notes from DB to files
func (s *Syncer) syncDBToFile(report *SyncReport) (*SyncReport, error) {
	notes, err := s.ReadAllNotesFromDB()
	if err != nil {
		return nil, fmt.Errorf("failed to read notes from DB: %w", err)
	}

	for _, note := range notes {
		// Skip deleted notes
		if note.DeletedAt != nil {
			continue
		}

		existing, err := s.fileHandler.ReadNote(note.ID)
		if err != nil {
			// File doesn't exist, export
			if err := s.fileHandler.WriteNote(note); err != nil {
				report.Errors = append(report.Errors, fmt.Sprintf("export %s: %v", note.ID, err))
				continue
			}
			report.Exported++
			continue
		}

		// Compare timestamps
		if note.UpdatedAt.After(existing.UpdatedAt) {
			if err := s.fileHandler.WriteNote(note); err != nil {
				report.Errors = append(report.Errors, fmt.Sprintf("update %s: %v", note.ID, err))
				continue
			}
			report.Updated++
		} else {
			report.Skipped++
		}
	}

	return report, nil
}

// syncFileToDB imports all notes from files to DB
func (s *Syncer) syncFileToDB(report *SyncReport) (*SyncReport, error) {
	notes, err := s.fileHandler.ReadAllNotes()
	if err != nil {
		return nil, fmt.Errorf("failed to read notes from files: %w", err)
	}

	for _, note := range notes {
		existing, err := s.ReadNoteFromDB(note.ID)
		if err != nil {
			// Note doesn't exist in DB, import
			if err := s.WriteNoteToDB(note); err != nil {
				report.Errors = append(report.Errors, fmt.Sprintf("import %s: %v", note.ID, err))
				continue
			}
			report.Imported++
			continue
		}

		// Compare timestamps
		if note.UpdatedAt.After(existing.UpdatedAt) {
			if err := s.WriteNoteToDB(note); err != nil {
				report.Errors = append(report.Errors, fmt.Sprintf("update %s: %v", note.ID, err))
				continue
			}
			report.Updated++
		} else {
			report.Skipped++
		}
	}

	return report, nil
}

// syncBidirectional performs bidirectional sync with timestamp priority
func (s *Syncer) syncBidirectional(report *SyncReport) (*SyncReport, error) {
	// Get all notes from both sources
	dbNotes, err := s.ReadAllNotesFromDB()
	if err != nil {
		return nil, fmt.Errorf("failed to read notes from DB: %w", err)
	}

	fileIDs, err := s.fileHandler.ListNoteFiles()
	if err != nil {
		return nil, fmt.Errorf("failed to list note files: %w", err)
	}

	// Build map of DB notes
	dbMap := make(map[string]*Note)
	for _, note := range dbNotes {
		if note.DeletedAt == nil {
			dbMap[note.ID] = note
		}
	}

	// Build set of file IDs
	fileSet := make(map[string]bool)
	for _, id := range fileIDs {
		fileSet[id] = true
	}

	// Process all notes
	for id, dbNote := range dbMap {
		fileNote, err := s.fileHandler.ReadNote(id)
		if err != nil {
			// Only in DB, export to file
			if err := s.fileHandler.WriteNote(dbNote); err != nil {
				report.Errors = append(report.Errors, fmt.Sprintf("export %s: %v", id, err))
				continue
			}
			report.Exported++
			continue
		}

		// Both exist, compare timestamps
		if fileNote.UpdatedAt.After(dbNote.UpdatedAt) {
			// File is newer, update DB
			if err := s.WriteNoteToDB(fileNote); err != nil {
				report.Errors = append(report.Errors, fmt.Sprintf("update DB %s: %v", id, err))
				continue
			}
			report.Updated++
		} else if dbNote.UpdatedAt.After(fileNote.UpdatedAt) {
			// DB is newer, update file
			if err := s.fileHandler.WriteNote(dbNote); err != nil {
				report.Errors = append(report.Errors, fmt.Sprintf("update file %s: %v", id, err))
				continue
			}
			report.Updated++
		} else {
			report.Skipped++
		}
		delete(fileSet, id)
	}

	// Process notes only in files
	for id := range fileSet {
		fileNote, err := s.fileHandler.ReadNote(id)
		if err != nil {
			report.Errors = append(report.Errors, fmt.Sprintf("read file %s: %v", id, err))
			continue
		}

		// Only in file, import to DB
		if err := s.WriteNoteToDB(fileNote); err != nil {
			report.Errors = append(report.Errors, fmt.Sprintf("import %s: %v", id, err))
			continue
		}
		report.Imported++
	}

	return report, nil
}

// ReadAllNotesFromDB reads all non-deleted notes from the database
func (s *Syncer) ReadAllNotesFromDB() ([]*Note, error) {
	db, err := sql.Open("sqlite", s.dbPath)
	if err != nil {
		return nil, err
	}
	defer db.Close()

	rows, err := db.Query(`
		SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at
		FROM notes
		ORDER BY updated_at DESC
	`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var notes []*Note
	for rows.Next() {
		note := &Note{}
		var createdAt, updatedAt string
		var deletedAt sql.NullString

		err := rows.Scan(
			&note.ID,
			&note.Title,
			&note.Content,
			&note.Tags,
			&createdAt,
			&updatedAt,
			&note.IsPinned,
			&deletedAt,
		)
		if err != nil {
			return nil, err
		}

		note.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
		note.UpdatedAt, _ = time.Parse(time.RFC3339, updatedAt)
		if deletedAt.Valid {
			t, _ := time.Parse(time.RFC3339, deletedAt.String)
			note.DeletedAt = &t
		}

		notes = append(notes, note)
	}

	return notes, nil
}

// ReadNoteFromDB reads a single note from the database
func (s *Syncer) ReadNoteFromDB(id string) (*Note, error) {
	db, err := sql.Open("sqlite", s.dbPath)
	if err != nil {
		return nil, err
	}
	defer db.Close()

	note := &Note{}
	var createdAt, updatedAt string
	var deletedAt sql.NullString

	err = db.QueryRow(`
		SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at
		FROM notes
		WHERE id = ? AND deleted_at IS NULL
	`, id).Scan(
		&note.ID,
		&note.Title,
		&note.Content,
		&note.Tags,
		&createdAt,
		&updatedAt,
		&note.IsPinned,
		&deletedAt,
	)
	if err != nil {
		return nil, err
	}

	note.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
	note.UpdatedAt, _ = time.Parse(time.RFC3339, updatedAt)
	return note, nil
}

// WriteNoteToDB writes a note to the database (insert or update)
func (s *Syncer) WriteNoteToDB(note *Note) error {
	db, err := sql.Open("sqlite", s.dbPath)
	if err != nil {
		return err
	}
	defer db.Close()

	// Check if note exists
	var exists bool
	err = db.QueryRow("SELECT 1 FROM notes WHERE id = ?", note.ID).Scan(&exists)
	if err != nil && err != sql.ErrNoRows {
		return err
	}

	if exists {
		// Update
		_, err = db.Exec(`
			UPDATE notes
			SET title = ?, content = ?, tags = ?, updated_at = ?, is_pinned = ?
			WHERE id = ?
		`,
			note.Title,
			note.Content,
			note.Tags,
			note.UpdatedAt.Format(time.RFC3339),
			note.IsPinned,
			note.ID,
		)
		return err
	}

	// Insert
	_, err = db.Exec(`
		INSERT INTO notes (id, title, content, tags, created_at, updated_at, is_pinned)
		VALUES (?, ?, ?, ?, ?, ?, ?)
	`,
		note.ID,
		note.Title,
		note.Content,
		note.Tags,
		note.CreatedAt.Format(time.RFC3339),
		note.UpdatedAt.Format(time.RFC3339),
		note.IsPinned,
	)
	return err
}

// DBExists checks if the database file exists
func (s *Syncer) DBExists() bool {
	_, err := os.Stat(s.dbPath)
	return err == nil
}

// SoftDeleteNote soft deletes a note by setting deleted_at timestamp
func (s *Syncer) SoftDeleteNote(noteID string) error {
	db, err := sql.Open("sqlite", s.dbPath)
	if err != nil {
		return err
	}
	defer db.Close()

	now := time.Now().Format(time.RFC3339)
	_, err = db.Exec(
		"UPDATE notes SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
		now, noteID,
	)
	return err
}

// RestoreNote restores a soft-deleted note
func (s *Syncer) RestoreNote(noteID string) error {
	db, err := sql.Open("sqlite", s.dbPath)
	if err != nil {
		return err
	}
	defer db.Close()

	_, err = db.Exec(
		"UPDATE notes SET deleted_at = NULL WHERE id = ?",
		noteID,
	)
	return err
}
