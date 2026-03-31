package executor

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"syscall"
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

	sessions, err := m.store.LoadSessions()
	if err != nil {
		return model.SessionRecord{}, err
	}
	sessions = append(sessions, session)
	if err := m.store.SaveSessions(sessions); err != nil {
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
	cmd.SysProcAttr = &syscall.SysProcAttr{
		HideWindow:    true,
		CreationFlags: 0x00000008 | 0x00000200,
	}

	if err := cmd.Start(); err != nil {
		return model.SessionRecord{}, err
	}
	_ = cmd.Process.Release()

	session.PID = cmd.Process.Pid
	for i := range sessions {
		if strings.EqualFold(sessions[i].SessionID, sessionID) {
			sessions[i] = session
			break
		}
	}
	if err := m.store.SaveSessions(sessions); err != nil {
		return model.SessionRecord{}, err
	}
	return session, nil
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

	sessions, err := m.store.LoadSessions()
	if err != nil {
		return err
	}
	index := -1
	for i, session := range sessions {
		if strings.EqualFold(session.SessionID, sessionID) {
			index = i
			break
		}
	}
	if index < 0 {
		return fmt.Errorf("session not found: %s", sessionID)
	}

	session := sessions[index]
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
	cmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true}

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

	session.Command = command
	session.WorkingDir = cmd.Dir
	session.FinishedAt = finishedAt.Format(time.RFC3339)
	session.LastChecked = session.FinishedAt
	session.DurationMs = finishedAt.Sub(startedAt).Milliseconds()
	session.ExitCode = intPointer(exitCode)
	session.Status = statusFromExitCode(exitCode)
	sessions[index] = session

	if err := m.store.SaveSessions(sessions); err != nil {
		return err
	}
	m.syncTask(session)
	return m.store.AppendHistory(historyFromSession(session))
}

func (m *AsyncManager) ListSessions() ([]model.SessionRecord, error) {
	sessions, err := m.store.LoadSessions()
	if err != nil {
		return nil, err
	}
	sessions = m.refreshSessions(sessions)
	if err := m.store.SaveSessions(sessions); err != nil {
		return nil, err
	}
	sort.Slice(sessions, func(i, j int) bool {
		return sessions[i].StartedAt > sessions[j].StartedAt
	})
	return sessions, nil
}

func (m *AsyncManager) Cancel(taskOrSessionID string) (model.SessionRecord, error) {
	sessions, err := m.store.LoadSessions()
	if err != nil {
		return model.SessionRecord{}, err
	}
	sessions = m.refreshSessions(sessions)

	index := -1
	for i, session := range sessions {
		if strings.EqualFold(session.SessionID, taskOrSessionID) {
			index = i
			break
		}
	}
	if index < 0 {
		return model.SessionRecord{}, fmt.Errorf("session not found: %s", taskOrSessionID)
	}

	session := sessions[index]
	if session.Status != "running" {
		return session, m.store.SaveSessions(sessions)
	}

	if err := exec.Command("taskkill", "/PID", strconv.Itoa(session.PID), "/T", "/F").Run(); err != nil {
		return model.SessionRecord{}, err
	}

	now := time.Now()
	exitCode := -1
	session.Status = "cancelled"
	session.ExitCode = &exitCode
	session.FinishedAt = now.Format(time.RFC3339)
	session.LastChecked = session.FinishedAt
	startTime, _ := time.Parse(time.RFC3339, session.StartedAt)
	if !startTime.IsZero() {
		session.DurationMs = now.Sub(startTime).Milliseconds()
	}
	sessions[index] = session
	if err := m.store.SaveSessions(sessions); err != nil {
		return model.SessionRecord{}, err
	}
	m.syncTask(session)
	_ = m.store.AppendHistory(historyFromSession(session))
	return session, nil
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

func (m *AsyncManager) refreshSessions(sessions []model.SessionRecord) []model.SessionRecord {
	now := time.Now()
	for i, session := range sessions {
		session.LastChecked = now.Format(time.RFC3339)
		if session.Status != "running" {
			sessions[i] = session
			continue
		}
		if processExists(session.PID) {
			sessions[i] = session
			continue
		}

		exitCode := 1
		session.Status = "failed"
		session.ExitCode = &exitCode
		session.FinishedAt = now.Format(time.RFC3339)
		startTime, _ := time.Parse(time.RFC3339, session.StartedAt)
		if !startTime.IsZero() {
			session.DurationMs = now.Sub(startTime).Milliseconds()
		}
		sessions[i] = session
		m.syncTask(session)
		_ = m.store.AppendHistory(historyFromSession(session))
	}
	return sessions
}

func (m *AsyncManager) syncTask(session model.SessionRecord) {
	_ = m.store.UpdateTaskStatus(session.SessionID, session.Status, session.ExitCode, session.DurationMs)
	output := historyFromSession(session).Output
	summary := ProcessOutput(output, session.OutputPath)
	_ = m.store.SetTaskOutput(session.SessionID, summary.Preview, session.OutputPath)
	if session.Status == "failed" && session.ExitCode != nil && *session.ExitCode < 0 {
		_ = m.store.SetTaskError(session.SessionID, "process terminated unexpectedly")
	}

	// Broadcast status change for non-running states
	m.broadcastStatus(session.SessionID, session.ScriptID, session.Status, session.ExitCode, session.DurationMs)
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
