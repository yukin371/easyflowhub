package notes

import "time"

// Note represents a note from DeskFlow database
type Note struct {
	ID        string     `json:"id"`
	Title     string     `json:"title"`
	Content   string     `json:"content"`
	Tags      string     `json:"tags"`
	CreatedAt time.Time  `json:"created_at"`
	UpdatedAt time.Time  `json:"updated_at"`
	IsPinned  bool       `json:"is_pinned"`
	DeletedAt *time.Time `json:"deleted_at,omitempty"`
}

// SyncReport contains the result of a sync operation
type SyncReport struct {
	Exported int      `json:"exported"` // DB → File
	Imported int      `json:"imported"` // File → DB
	Updated   int      `json:"updated"`  // Bidirectional updates
	Skipped   int      `json:"skipped"`  // No changes
	Errors    []string `json:"errors"`
}

// SyncDirection specifies the direction of sync
type SyncDirection string

const (
	SyncBidirectional SyncDirection = "bidirectional"
	SyncDBToFile      SyncDirection = "db_to_file"
	SyncFileToDB      SyncDirection = "file_to_db"
)
