package model

import "time"

type ScriptRecord struct {
	ID          string            `json:"id"`
	Name        string            `json:"name"`
	Path        string            `json:"path"`
	ScriptType  string            `json:"script_type"`
	SourceRoot  string            `json:"source_root,omitempty"`
	Description string            `json:"description,omitempty"`
	Category    string            `json:"category,omitempty"`
	Tags        []string          `json:"tags,omitempty"`
	Author      string            `json:"author,omitempty"`
	Version     string            `json:"version,omitempty"`
	Icon        string            `json:"icon,omitempty"`
	Parameters  []ScriptParameter `json:"parameters,omitempty"`
}

type ScriptMetadata struct {
	ID          string            `json:"id"`
	Name        string            `json:"name"`
	Description string            `json:"description"`
	Category    string            `json:"category"`
	Tags        []string          `json:"tags"`
	Author      string            `json:"author"`
	Version     string            `json:"version"`
	Icon        string            `json:"icon"`
	Parameters  []ScriptParameter `json:"parameters"`
}

type ScriptParameter struct {
	Name        string `json:"name"`
	Type        string `json:"type"`
	Label       string `json:"label"`
	Default     string `json:"default"`
	Required    bool   `json:"required"`
	Description string `json:"description"`
}

type RunResult struct {
	ScriptID   string   `json:"script_id"`
	ScriptName string   `json:"script_name"`
	Command    []string `json:"command"`
	ExitCode   int      `json:"exit_code"`
	Status     string   `json:"status"`
	Succeeded  bool     `json:"succeeded"`
	Output     string   `json:"output,omitempty"`
	OutputMeta *OutputResult `json:"output_meta,omitempty"`
	WorkingDir string   `json:"working_dir"`
	StartedAt  string   `json:"started_at"`
	FinishedAt string   `json:"finished_at"`
	DurationMs int64    `json:"duration_ms"`
}

type SessionRecord struct {
	SessionID   string   `json:"session_id"`
	ScriptID    string   `json:"script_id"`
	ScriptName  string   `json:"script_name"`
	Command     []string `json:"command"`
	PID         int      `json:"pid"`
	Status      string   `json:"status"`
	WorkingDir  string   `json:"working_dir"`
	OutputPath  string   `json:"output_path"`
	StartedAt   string   `json:"started_at"`
	FinishedAt  string   `json:"finished_at,omitempty"`
	ExitCode    *int     `json:"exit_code,omitempty"`
	DurationMs  int64    `json:"duration_ms,omitempty"`
	LastChecked string   `json:"last_checked,omitempty"`
}

type HistoryEntry struct {
	HistoryID  string   `json:"history_id"`
	ScriptID   string   `json:"script_id"`
	ScriptName string   `json:"script_name"`
	Command    []string `json:"command"`
	Status     string   `json:"status"`
	ExitCode   *int     `json:"exit_code,omitempty"`
	WorkingDir string   `json:"working_dir"`
	StartedAt  string   `json:"started_at"`
	FinishedAt string   `json:"finished_at"`
	DurationMs int64    `json:"duration_ms"`
	OutputPath string   `json:"output_path,omitempty"`
	Output     string   `json:"output,omitempty"`
}

type FavoritesStore struct {
	IDs       []string `json:"ids"`
	UpdatedAt string   `json:"updated_at"`
}

type ScriptRootsStore struct {
	Roots     []string `json:"roots"`
	UpdatedAt string   `json:"updated_at"`
}

type ListResponse struct {
	OK          bool           `json:"ok"`
	Count       int            `json:"count"`
	Search      string         `json:"search,omitempty"`
	Roots       []string       `json:"roots"`
	Scripts     []ScriptRecord `json:"scripts"`
	GeneratedAt string         `json:"generated_at"`
}

type DescribeResponse struct {
	OK          bool         `json:"ok"`
	Script      ScriptRecord `json:"script"`
	GeneratedAt string       `json:"generated_at"`
}

type HistoryResponse struct {
	OK          bool           `json:"ok"`
	Count       int            `json:"count"`
	Entries     []HistoryEntry `json:"entries"`
	GeneratedAt string         `json:"generated_at"`
}

type FavoritesResponse struct {
	OK          bool           `json:"ok"`
	Count       int            `json:"count"`
	Favorites   []ScriptRecord `json:"favorites"`
	GeneratedAt string         `json:"generated_at"`
}

type SessionsResponse struct {
	OK          bool            `json:"ok"`
	Count       int             `json:"count"`
	Sessions    []SessionRecord `json:"sessions"`
	GeneratedAt string          `json:"generated_at"`
}

type CancelResponse struct {
	OK          bool          `json:"ok"`
	Session     SessionRecord `json:"session"`
	GeneratedAt string        `json:"generated_at"`
}

type RootsResponse struct {
	OK          bool     `json:"ok"`
	Count       int      `json:"count"`
	Roots       []string `json:"roots"`
	GeneratedAt string   `json:"generated_at"`
}

type RunOptions struct {
	DryRun        bool
	AsJSON        bool
	CaptureOutput bool
	Detach        bool
}

type Task struct {
	TaskID        string     `json:"task_id"`
	ScriptID      string     `json:"script_id"`
	Status        string     `json:"status"`
	CreatedAt     time.Time  `json:"created_at"`
	StartedAt     *time.Time `json:"started_at,omitempty"`
	CompletedAt   *time.Time `json:"completed_at,omitempty"`
	ExitCode      *int       `json:"exit_code,omitempty"`
	DurationMs    int64      `json:"duration_ms,omitempty"`
	InputJSON     string     `json:"input_json,omitempty"`
	OutputPath    string     `json:"output_path,omitempty"`
	OutputSummary string     `json:"output_summary,omitempty"`
	Error         string     `json:"error,omitempty"`
}

type OutputResult struct {
	Truncated   bool   `json:"truncated"`
	Preview     string `json:"preview"`
	TotalLength int    `json:"total_length"`
	LineCount   int    `json:"line_count"`
	LogPath     string `json:"log_path,omitempty"`
}
