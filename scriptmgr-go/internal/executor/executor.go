package executor

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"scriptmgr/internal/discovery"
	"scriptmgr/internal/model"
	"scriptmgr/internal/store"
	"scriptmgr/internal/validator"
)

type Service struct {
	store     *store.Store
	discovery *discovery.Service
	async     *AsyncManager
}

func New(s *store.Store, d *discovery.Service) *Service {
	return &Service{
		store:     s,
		discovery: d,
		async:     NewAsyncManager(s, d),
	}
}

func (s *Service) BuildCommand(script model.ScriptRecord, args []string) ([]string, error) {
	switch script.ScriptType {
	case "powershell":
		return append([]string{"powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script.Path}, args...), nil
	case "python":
		return append([]string{"python", script.Path}, args...), nil
	case "batch":
		return append([]string{"cmd.exe", "/c", script.Path}, args...), nil
	default:
		return nil, fmt.Errorf("unsupported script type: %s", script.ScriptType)
	}
}

func (s *Service) RunForeground(script model.ScriptRecord, command []string, captureOutput bool) (model.RunResult, error) {
	cmd := exec.Command(command[0], command[1:]...)
	workingDir := filepath.Dir(script.Path)
	cmd.Dir = workingDir
	cmd.Stdin = os.Stdin
	startedAt := time.Now()

	var buffer bytes.Buffer
	if captureOutput {
		cmd.Stdout = &buffer
		cmd.Stderr = &buffer
	} else {
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
	}

	err := cmd.Run()
	exitCode := exitCodeFromError(err)
	if err != nil && exitCode < 0 {
		return model.RunResult{}, err
	}

	finishedAt := time.Now()
	result := model.RunResult{
		ScriptID:   script.ID,
		ScriptName: script.Name,
		Command:    command,
		ExitCode:   exitCode,
		Status:     statusFromExitCode(exitCode),
		Succeeded:  exitCode == 0,
		WorkingDir: workingDir,
		StartedAt:  startedAt.Format(time.RFC3339),
		FinishedAt: finishedAt.Format(time.RFC3339),
		DurationMs: finishedAt.Sub(startedAt).Milliseconds(),
		Output:     buffer.String(),
		OutputMeta: ProcessOutput(buffer.String(), ""),
	}

	_ = s.store.AppendHistory(model.HistoryEntry{
		HistoryID:  newID("hist"),
		ScriptID:   result.ScriptID,
		ScriptName: result.ScriptName,
		Command:    result.Command,
		Status:     result.Status,
		ExitCode:   intPointer(result.ExitCode),
		WorkingDir: result.WorkingDir,
		StartedAt:  result.StartedAt,
		FinishedAt: result.FinishedAt,
		DurationMs: result.DurationMs,
		Output:     result.Output,
	})

	if exitCode != 0 {
		return result, fmt.Errorf("script exited with code %d", exitCode)
	}
	return result, nil
}

func (s *Service) StartDetachedSession(script model.ScriptRecord, scriptArgs []string, command []string) (model.SessionRecord, error) {
	return s.async.Start(script, scriptArgs, command)
}

func (s *Service) RunWorker(sessionID, scriptID string, scriptArgs []string) error {
	return s.async.RunWorker(s.BuildCommand, sessionID, scriptID, scriptArgs)
}

func (s *Service) ListSessions() ([]model.SessionRecord, error) {
	return s.async.ListSessions()
}

func (s *Service) CancelSession(sessionID string) (model.SessionRecord, error) {
	return s.async.Cancel(sessionID)
}

func (s *Service) CancelTask(taskID string) (model.SessionRecord, error) {
	return s.async.Cancel(taskID)
}

func (s *Service) ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error) {
	return s.async.ReadTaskLog(taskID, offset, limit, tail)
}

func historyFromSession(session model.SessionRecord) model.HistoryEntry {
	var output string
	if session.OutputPath != "" {
		if content, err := os.ReadFile(session.OutputPath); err == nil {
			output = string(content)
		}
	}
	finishedAt := session.FinishedAt
	if finishedAt == "" {
		finishedAt = nowRFC3339()
	}
	return model.HistoryEntry{
		HistoryID:  newID("hist"),
		ScriptID:   session.ScriptID,
		ScriptName: session.ScriptName,
		Command:    session.Command,
		Status:     session.Status,
		ExitCode:   session.ExitCode,
		WorkingDir: session.WorkingDir,
		StartedAt:  session.StartedAt,
		FinishedAt: finishedAt,
		DurationMs: session.DurationMs,
		OutputPath: session.OutputPath,
		Output:     output,
	}
}

func statusFromExitCode(exitCode int) string {
	if exitCode == 0 {
		return "succeeded"
	}
	return "failed"
}

func exitCodeFromError(err error) int {
	if err == nil {
		return 0
	}
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) {
		return exitErr.ExitCode()
	}
	return -1
}

func newID(prefix string) string {
	return fmt.Sprintf("%s-%d", prefix, time.Now().UnixNano())
}

func intPointer(v int) *int {
	return &v
}

func nowRFC3339() string {
	return time.Now().Format(time.RFC3339)
}

func (s *Service) ValidateScriptArgs(script model.ScriptRecord, scriptArgs []string) ([]string, error) {
	return validator.ValidateArgs(script.Parameters, scriptArgs)
}

// SetBroadcaster sets the broadcaster for async task status notifications
func (s *Service) SetBroadcaster(b StatusBroadcaster) {
	s.async.SetBroadcaster(b)
}
