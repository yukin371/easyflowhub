package executor

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"scriptmgr/internal/discovery"
	"scriptmgr/internal/model"
	"scriptmgr/internal/store"
)

type AsyncManager struct {
	store       *store.Store
	discovery   *discovery.Service
	broadcaster StatusBroadcaster
}

// StatusBroadcaster defines the interface for broadcasting task status changes
type StatusBroadcaster interface {
	BroadcastTaskStatus(taskID, scriptID, status string, exitCode *int, durationMs int64)
}

// SetBroadcaster sets the broadcaster for task status notifications
func (m *AsyncManager) SetBroadcaster(b StatusBroadcaster) {
	m.broadcaster = b
}

// broadcastStatus sends a status update to the broadcaster if one is set
func (m *AsyncManager) broadcastStatus(sessionID, scriptID, status string, exitCode *int, durationMs int64) {
	if m.broadcaster != nil {
		m.broadcaster.BroadcastTaskStatus(sessionID, scriptID, status, exitCode, durationMs)
	}
}

func NewAsyncManager(s *store.Store, d *discovery.Service) *AsyncManager {
	return &AsyncManager{
		store:     s,
		discovery: d,
	}
}

func (m *AsyncManager) Start(script model.ScriptRecord, scriptArgs []string, command []string) (model.SessionRecord, error) {
	sessionID := newID("sess")
	outputPath := filepath.Join(m.store.StateDir(), "sessions", sessionID+".log")
	if err := os.MkdirAll(filepath.Dir(outputPath), 0o755); err != nil {
		return model.SessionRecord{}, err
	}

	startedAt := time.Now()
	session := model.SessionRecord{
		SessionID:   sessionID,
		ScriptID:    script.ID,
		ScriptName:  script.Name,
		Command:     command,
		Status:      "running",
		WorkingDir:  filepath.Dir(script.Path),
		OutputPath:  outputPath,
		StartedAt:   startedAt.Format(time.RFC3339),
		LastChecked: startedAt.Format(time.RFC3339),
	}

	if err := m.store.AppendSession(session); err != nil {
		return model.SessionRecord{}, err
	}
	if err := m.store.CreateTask(sessionID, script.ID, "{}"); err != nil {
		return model.SessionRecord{}, err
	}
	if err := m.store.UpdateTaskStatus(sessionID, "running", nil, 0); err != nil {
		return model.SessionRecord{}, err
	}
	if err := m.store.SetTaskOutput(sessionID, "", outputPath); err != nil {
		return model.SessionRecord{}, err
	}

	// Broadcast running status
	m.broadcastStatus(sessionID, script.ID, "running", nil, 0)

	exePath, err := os.Executable()
	if err != nil {
		return model.SessionRecord{}, err
	}

	workerArgs := append([]string{"__worker", sessionID, script.ID}, scriptArgs...)
	cmd := exec.Command(exePath, workerArgs...)
	cmd.Dir = filepath.Dir(script.Path)
	devNull, err := os.OpenFile(os.DevNull, os.O_WRONLY, 0)
	if err != nil {
		return model.SessionRecord{}, err
	}
	defer devNull.Close()
	cmd.Stdout = devNull
	cmd.Stderr = devNull
	cmd.Stdin = nil
	applySysProcAttr(cmd)

	if err := cmd.Start(); err != nil {
		return model.SessionRecord{}, err
	}
	_ = cmd.Process.Release()

	session.PID = cmd.Process.Pid
	updated, err := m.store.UpdateSession(sessionID, func(record *model.SessionRecord) error {
		record.PID = session.PID
		return nil
	})
	if err != nil {
		return model.SessionRecord{}, err
	}
	return updated, nil
}

func (m *AsyncManager) RunWorker(buildCommand func(model.ScriptRecord, []string) ([]string, error), sessionID, scriptID string, scriptArgs []string) error {
	script, err := m.discovery.FindScript(scriptID)
	if err != nil {
		return err
	}
	command, err := buildCommand(script, scriptArgs)
	if err != nil {
		return err
	}

	session, err := m.store.GetSession(sessionID)
	if err != nil {
		return err
	}

	outputFile, err := os.OpenFile(session.OutputPath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return err
	}
	defer outputFile.Close()

	cmd := exec.Command(command[0], command[1:]...)
	cmd.Dir = filepath.Dir(script.Path)
	cmd.Stdout = outputFile
	cmd.Stderr = outputFile
	cmd.Stdin = nil
	applySysProcAttr(cmd)

	startedAt, parseErr := time.Parse(time.RFC3339, session.StartedAt)
	if parseErr != nil || startedAt.IsZero() {
		startedAt = time.Now()
		session.StartedAt = startedAt.Format(time.RFC3339)
	}

	runErr := cmd.Run()
	exitCode := exitCodeFromError(runErr)
	finishedAt := time.Now()

	if runErr != nil && exitCode < 0 {
		_, _ = outputFile.WriteString(runErr.Error() + "\n")
		exitCode = 1
	}

	// Only this worker's own record is rewritten, and the store re-reads the
	// file under the cross-process lock, so sessions started by other
	// processes while this one ran are preserved (F01).
	finalStatus := statusFromExitCode(exitCode)
	updated, err := m.store.UpdateSession(sessionID, func(record *model.SessionRecord) error {
		record.Command = command
		record.WorkingDir = cmd.Dir
		record.FinishedAt = finishedAt.Format(time.RFC3339)
		record.LastChecked = record.FinishedAt
		record.DurationMs = finishedAt.Sub(startedAt).Milliseconds()
		record.ExitCode = intPointer(exitCode)
		record.Status = finalStatus
		return nil
	})
	if err != nil {
		return err
	}
	if err := m.syncTask(updated); err != nil {
		return err
	}
	return m.store.AppendHistory(historyFromSession(updated))
}

func (m *AsyncManager) ListSessions() ([]model.SessionRecord, error) {
	sessions, err := m.store.LoadSessions()
	if err != nil {
		return nil, err
	}
	sessions, err = m.refreshSessions(sessions)
	if err != nil {
		return nil, err
	}
	sort.Slice(sessions, func(i, j int) bool {
		return sessions[i].StartedAt > sessions[j].StartedAt
	})
	return sessions, nil
}

func (m *AsyncManager) Cancel(taskOrSessionID string) (model.SessionRecord, error) {
	session, err := m.store.GetSession(taskOrSessionID)
	if err != nil {
		return model.SessionRecord{}, err
	}
	if session.Status != "running" {
		return *session, nil
	}

	if err := exec.Command("taskkill", "/PID", strconv.Itoa(session.PID), "/T", "/F").Run(); err != nil {
		return model.SessionRecord{}, err
	}

	now := time.Now()
	exitCode := -1
	// Guard on "still running" inside the locked update: if the worker
	// finished and wrote its own terminal state between the check above and
	// this write, the cancel must not resurrect or overwrite it.
	updated, err := m.store.UpdateSession(taskOrSessionID, func(record *model.SessionRecord) error {
		if record.Status != "running" {
			return nil
		}
		record.Status = "cancelled"
		record.ExitCode = &exitCode
		record.FinishedAt = now.Format(time.RFC3339)
		record.LastChecked = record.FinishedAt
		startTime, _ := time.Parse(time.RFC3339, record.StartedAt)
		if !startTime.IsZero() {
			record.DurationMs = now.Sub(startTime).Milliseconds()
		}
		return nil
	})
	if err != nil {
		return model.SessionRecord{}, err
	}
	if updated.Status == "cancelled" {
		if err := m.syncTask(updated); err != nil {
			return updated, err
		}
		_ = m.store.AppendHistory(historyFromSession(updated))
	}
	return updated, nil
}

func (m *AsyncManager) ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error) {
	task, err := m.store.GetTask(taskID)
	if err != nil {
		return "", nil, err
	}
	if task.OutputPath == "" {
		sessions, loadErr := m.store.LoadSessions()
		if loadErr == nil {
			for _, session := range sessions {
				if strings.EqualFold(session.SessionID, taskID) && session.OutputPath != "" {
					task.OutputPath = session.OutputPath
					break
				}
			}
		}
	}
	if task.OutputPath == "" {
		return "", task, fmt.Errorf("task has no output log: %s", taskID)
	}
	output, err := ReadLog(task.OutputPath, offset, limit, tail)
	if err != nil {
		return "", task, err
	}
	return output, task, nil
}

// refreshSessions reconciles "running" rows whose process died without a
// completion write. Only the transitioned records are persisted, each through
// its own locked update, so a refresh can never overwrite a session another
// process is concurrently starting or finishing. LastChecked is refreshed in
// memory for display only; it is not worth a cross-process write per poll.
func (m *AsyncManager) refreshSessions(sessions []model.SessionRecord) ([]model.SessionRecord, error) {
	now := time.Now()
	var firstErr error
	for i, session := range sessions {
		session.LastChecked = now.Format(time.RFC3339)
		sessions[i] = session
		if session.Status != "running" || processExists(session.PID) {
			continue
		}

		exitCode := 1
		finishedAt := now.Format(time.RFC3339)
		updated, err := m.store.UpdateSession(session.SessionID, func(record *model.SessionRecord) error {
			// Re-check under the lock: the worker may have just written its
			// own terminal state.
			if record.Status != "running" || processExists(record.PID) {
				return nil
			}
			record.Status = "failed"
			record.ExitCode = &exitCode
			record.FinishedAt = finishedAt
			record.LastChecked = finishedAt
			startTime, _ := time.Parse(time.RFC3339, record.StartedAt)
			if !startTime.IsZero() {
				record.DurationMs = now.Sub(startTime).Milliseconds()
			}
			return nil
		})
		if err != nil {
			if firstErr == nil {
				firstErr = err
			}
			continue
		}
		sessions[i] = updated
		if updated.Status == "failed" {
			if err := m.syncTask(updated); err != nil && firstErr == nil {
				firstErr = err
			}
			_ = m.store.AppendHistory(historyFromSession(updated))
		}
	}
	return sessions, firstErr
}

// syncTask mirrors a session's terminal state into tasks.db. Storage errors
// are returned instead of swallowed so a diverging task record is visible to
// the caller rather than silently drift.
func (m *AsyncManager) syncTask(session model.SessionRecord) error {
	if err := m.store.UpdateTaskStatus(session.SessionID, session.Status, session.ExitCode, session.DurationMs); err != nil {
		m.broadcastStatus(session.SessionID, session.ScriptID, session.Status, session.ExitCode, session.DurationMs)
		return fmt.Errorf("sync task status %s: %w", session.SessionID, err)
	}
	output := historyFromSession(session).Output
	summary := ProcessOutput(output, session.OutputPath)
	var errs []error
	if err := m.store.SetTaskOutput(session.SessionID, summary.Preview, session.OutputPath); err != nil {
		errs = append(errs, fmt.Errorf("sync task output %s: %w", session.SessionID, err))
	}
	if session.Status == "failed" && session.ExitCode != nil && *session.ExitCode < 0 {
		if err := m.store.SetTaskError(session.SessionID, "process terminated unexpectedly"); err != nil {
			errs = append(errs, fmt.Errorf("sync task error %s: %w", session.SessionID, err))
		}
	}

	// Broadcast status change for non-running states
	m.broadcastStatus(session.SessionID, session.ScriptID, session.Status, session.ExitCode, session.DurationMs)
	return errors.Join(errs...)
}

func processExists(pid int) bool {
	if pid <= 0 {
		return false
	}
	cmd := exec.Command("tasklist", "/FI", fmt.Sprintf("PID eq %d", pid), "/FO", "CSV", "/NH")
	var output bytes.Buffer
	cmd.Stdout = &output
	cmd.Stderr = &output
	if err := cmd.Run(); err != nil {
		return false
	}
	text := strings.TrimSpace(output.String())
	return text != "" && !strings.Contains(strings.ToLower(text), "no tasks are running")
}
