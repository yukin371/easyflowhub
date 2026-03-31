package store

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"time"

	_ "modernc.org/sqlite"
	"scriptmgr/internal/config"
	"scriptmgr/internal/model"
)

type Store struct {
	stateDir string
}

func New(cfg config.Config) *Store {
	return &Store{stateDir: cfg.StateDir}
}

func (s *Store) StateDir() string {
	return s.stateDir
}

func (s *Store) HistoryPath() string {
	return filepath.Join(s.stateDir, "history.json")
}

func (s *Store) FavoritesPath() string {
	return filepath.Join(s.stateDir, "favorites.json")
}

func (s *Store) SessionsPath() string {
	return filepath.Join(s.stateDir, "sessions.json")
}

func (s *Store) RootsPath() string {
	return filepath.Join(s.stateDir, "roots.json")
}

func (s *Store) TasksDBPath() string {
	return filepath.Join(s.stateDir, "tasks.db")
}

func (s *Store) LoadHistory() ([]model.HistoryEntry, error) {
	var entries []model.HistoryEntry
	if err := readJSONFile(s.HistoryPath(), &entries); err != nil {
		return nil, err
	}
	return entries, nil
}

func (s *Store) SaveHistory(entries []model.HistoryEntry) error {
	return writeJSONFile(s.HistoryPath(), entries)
}

func (s *Store) AppendHistory(entry model.HistoryEntry) error {
	entries, err := s.LoadHistory()
	if err != nil {
		return err
	}
	entries = append(entries, entry)
	if len(entries) > 200 {
		entries = entries[len(entries)-200:]
	}
	return s.SaveHistory(entries)
}

func (s *Store) LoadFavorites() (model.FavoritesStore, error) {
	var favorites model.FavoritesStore
	if err := readJSONFile(s.FavoritesPath(), &favorites); err != nil {
		return model.FavoritesStore{}, err
	}
	return favorites, nil
}

func (s *Store) SaveFavorites(favorites model.FavoritesStore) error {
	return writeJSONFile(s.FavoritesPath(), favorites)
}

func (s *Store) LoadSessions() ([]model.SessionRecord, error) {
	var sessions []model.SessionRecord
	if err := readJSONFile(s.SessionsPath(), &sessions); err != nil {
		return nil, err
	}
	return sessions, nil
}

func (s *Store) SaveSessions(sessions []model.SessionRecord) error {
	return writeJSONFile(s.SessionsPath(), sessions)
}

func (s *Store) LoadRoots() (model.ScriptRootsStore, error) {
	var roots model.ScriptRootsStore
	if err := readJSONFile(s.RootsPath(), &roots); err != nil {
		return model.ScriptRootsStore{}, err
	}
	return roots, nil
}

func (s *Store) SaveRoots(roots model.ScriptRootsStore) error {
	return writeJSONFile(s.RootsPath(), roots)
}

func (s *Store) CreateTask(taskID, scriptID, inputJSON string) error {
	db, err := s.openTaskDB()
	if err != nil {
		return err
	}
	defer db.Close()

	_, err = db.Exec(
		`INSERT INTO tasks (task_id, script_id, status, input_json) VALUES (?, ?, 'pending', ?)`,
		taskID, scriptID, inputJSON,
	)
	return err
}

func (s *Store) UpdateTaskStatus(taskID, status string, exitCode *int, durationMs int64) error {
	db, err := s.openTaskDB()
	if err != nil {
		return err
	}
	defer db.Close()

	now := time.Now()
	if status == "running" {
		_, err = db.Exec(
			`UPDATE tasks SET status = ?, started_at = ? WHERE task_id = ?`,
			status, now, taskID,
		)
		return err
	}

	_, err = db.Exec(
		`UPDATE tasks
		 SET status = ?, completed_at = ?, exit_code = ?, duration_ms = ?
		 WHERE task_id = ?`,
		status, now, exitCode, durationMs, taskID,
	)
	return err
}

func (s *Store) SetTaskOutput(taskID, summary, outputPath string) error {
	db, err := s.openTaskDB()
	if err != nil {
		return err
	}
	defer db.Close()

	_, err = db.Exec(
		`UPDATE tasks SET output_summary = ?, output_path = ? WHERE task_id = ?`,
		summary, outputPath, taskID,
	)
	return err
}

func (s *Store) SetTaskError(taskID, errorMessage string) error {
	db, err := s.openTaskDB()
	if err != nil {
		return err
	}
	defer db.Close()

	_, err = db.Exec(
		`UPDATE tasks SET error_message = ? WHERE task_id = ?`,
		errorMessage, taskID,
	)
	return err
}

func (s *Store) GetTask(taskID string) (*model.Task, error) {
	db, err := s.openTaskDB()
	if err != nil {
		return nil, err
	}
	defer db.Close()

	row := db.QueryRow(
		`SELECT task_id, script_id, status, created_at, started_at, completed_at,
		        exit_code, duration_ms, input_json, output_path, output_summary, error_message
		   FROM tasks
		  WHERE task_id = ?`,
		taskID,
	)

	task, err := scanTask(row.Scan)
	if err != nil {
		return nil, err
	}
	return task, nil
}

func (s *Store) ListTasks(status string, limit int) ([]model.Task, error) {
	db, err := s.openTaskDB()
	if err != nil {
		return nil, err
	}
	defer db.Close()

	query := `SELECT task_id, script_id, status, created_at, started_at, completed_at,
	                 exit_code, duration_ms, input_json, output_path, output_summary, error_message
	            FROM tasks`
	args := []any{}
	if status != "" {
		statuses := splitStatuses(status)
		if len(statuses) == 1 {
			query += ` WHERE status = ?`
			args = append(args, statuses[0])
		} else if len(statuses) > 1 {
			placeholders := make([]string, 0, len(statuses))
			for _, item := range statuses {
				placeholders = append(placeholders, "?")
				args = append(args, item)
			}
			query += ` WHERE status IN (` + strings.Join(placeholders, ",") + `)`
		}
	}
	query += ` ORDER BY created_at DESC LIMIT ?`
	args = append(args, limit)

	rows, err := db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var tasks []model.Task
	for rows.Next() {
		task, err := scanTask(rows.Scan)
		if err != nil {
			return nil, err
		}
		tasks = append(tasks, *task)
	}
	return tasks, rows.Err()
}

func (s *Store) openTaskDB() (*sql.DB, error) {
	db, err := sql.Open("sqlite", s.TasksDBPath())
	if err != nil {
		return nil, err
	}
	if err := s.migrateTasks(db); err != nil {
		db.Close()
		return nil, err
	}
	return db, nil
}

func (s *Store) migrateTasks(db *sql.DB) error {
	_, err := db.Exec(`
		CREATE TABLE IF NOT EXISTS tasks (
			task_id TEXT PRIMARY KEY,
			script_id TEXT NOT NULL,
			status TEXT NOT NULL DEFAULT 'pending',
			created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
			started_at DATETIME,
			completed_at DATETIME,
			exit_code INTEGER,
			duration_ms INTEGER,
			input_json TEXT,
			output_path TEXT,
			output_summary TEXT,
			output_full TEXT,
			error_message TEXT
		);
		CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
		CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC);
	`)
	return err
}

func readJSONFile(path string, target any) error {
	content, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}
	if len(bytes.TrimSpace(content)) == 0 {
		return nil
	}
	return json.Unmarshal(content, target)
}

func writeJSONFile(path string, value any) error {
	content, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, content, 0o644)
}

func splitStatuses(raw string) []string {
	parts := strings.Split(raw, ",")
	var statuses []string
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part != "" {
			statuses = append(statuses, part)
		}
	}
	return statuses
}

func scanTask(scan func(dest ...any) error) (*model.Task, error) {
	var task model.Task
	var startedAt sql.NullTime
	var completedAt sql.NullTime
	var exitCode sql.NullInt64
	var durationMs sql.NullInt64
	var inputJSON sql.NullString
	var outputPath sql.NullString
	var outputSummary sql.NullString
	var errorMessage sql.NullString

	if err := scan(
		&task.TaskID,
		&task.ScriptID,
		&task.Status,
		&task.CreatedAt,
		&startedAt,
		&completedAt,
		&exitCode,
		&durationMs,
		&inputJSON,
		&outputPath,
		&outputSummary,
		&errorMessage,
	); err != nil {
		return nil, err
	}

	if startedAt.Valid {
		task.StartedAt = &startedAt.Time
	}
	if completedAt.Valid {
		task.CompletedAt = &completedAt.Time
	}
	if exitCode.Valid {
		value := int(exitCode.Int64)
		task.ExitCode = &value
	}
	if durationMs.Valid {
		task.DurationMs = durationMs.Int64
	}
	if inputJSON.Valid {
		task.InputJSON = inputJSON.String
	}
	if outputPath.Valid {
		task.OutputPath = outputPath.String
	}
	if outputSummary.Valid {
		task.OutputSummary = outputSummary.String
	}
	if errorMessage.Valid {
		task.Error = errorMessage.String
	}
	return &task, nil
}
