package executor

import (
	"errors"
	"os"
	"strings"
	"testing"
	"time"

	"scriptmgr/internal/config"
	"scriptmgr/internal/model"
	"scriptmgr/internal/store"
)

func TestListSessionsDoesNotReapFreshPIDZeroStartup(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)
	now := time.Now().UTC().Format(time.RFC3339)
	if err := st.AppendSession(model.SessionRecord{
		SessionID: "sess-starting", ScriptID: "s1", Status: "running",
		PID: 0, StartedAt: now, LastChecked: now,
	}); err != nil {
		t.Fatalf("append: %v", err)
	}

	sessions, err := m.ListSessions()
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(sessions) != 1 || sessions[0].Status != "running" {
		t.Fatalf("fresh PID=0 startup should remain running, got %+v", sessions)
	}
}

func TestListSessionsReapsStalePIDZeroStartup(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)
	old := time.Now().Add(-sessionStartupGrace - time.Second).UTC().Format(time.RFC3339)
	if err := st.AppendSession(model.SessionRecord{
		SessionID: "sess-stale", ScriptID: "s1", Status: "running",
		PID: 0, StartedAt: old, LastChecked: old,
	}); err != nil {
		t.Fatalf("append: %v", err)
	}

	sessions, err := m.ListSessions()
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(sessions) != 1 || sessions[0].Status != "failed" {
		t.Fatalf("stale PID=0 startup should fail, got %+v", sessions)
	}
}

func TestFailStartCompensatesPersistedSession(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)
	session := model.SessionRecord{
		SessionID: "sess-fail", ScriptID: "s1", Status: "running",
		StartedAt: time.Now().UTC().Format(time.RFC3339),
	}
	if err := st.AppendSession(session); err != nil {
		t.Fatalf("append: %v", err)
	}
	if err := st.CreateTask(session.SessionID, session.ScriptID, "{}"); err != nil {
		t.Fatalf("create task: %v", err)
	}

	cause := errors.New("process setup failed")
	if err := m.failStart(session, cause); !errors.Is(err, cause) {
		t.Fatalf("failStart should preserve cause, got %v", err)
	}
	updated, err := st.GetSession(session.SessionID)
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	if updated.Status != "failed" || updated.ExitCode == nil || *updated.ExitCode != 1 {
		t.Fatalf("session was not compensated: %+v", updated)
	}
	task, err := st.GetTask(session.SessionID)
	if err != nil {
		t.Fatalf("get task: %v", err)
	}
	if task.Status != "failed" {
		t.Fatalf("task was not compensated: %+v", task)
	}
	if task.Error != cause.Error() {
		t.Fatalf("task should retain startup cause, got %q", task.Error)
	}
}

// F01: a "running" record whose worker died without writing a terminal state
// must be reconciled to failed on read, persisted per-record, and mirrored
// into tasks.db so every read interface agrees on the same terminal state.
func TestListSessionsReconcilesDeadRunningSession(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)

	now := "2026-10-19T00:00:00Z"
	// PID -1 never exists, on any platform.
	dead := model.SessionRecord{
		SessionID: "sess-dead", ScriptID: "s1", Status: "running",
		PID: -1, OutputPath: "", StartedAt: now, LastChecked: now,
	}
	done := model.SessionRecord{
		SessionID: "sess-done", ScriptID: "s2", Status: "success",
		OutputPath: "", StartedAt: now, LastChecked: now,
	}
	for _, session := range []model.SessionRecord{dead, done} {
		if err := st.AppendSession(session); err != nil {
			t.Fatalf("append %s: %v", session.SessionID, err)
		}
		if err := st.CreateTask(session.SessionID, session.ScriptID, "{}"); err != nil {
			t.Fatalf("create task %s: %v", session.SessionID, err)
		}
	}

	sessions, err := m.ListSessions()
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(sessions) != 2 {
		t.Fatalf("expected both sessions to survive reconciliation, got %d", len(sessions))
	}

	byID := map[string]model.SessionRecord{}
	for _, session := range sessions {
		byID[session.SessionID] = session
	}
	if got := byID["sess-dead"].Status; got != "failed" {
		t.Fatalf("dead running session should be failed, got %q", got)
	}
	if got := byID["sess-done"].Status; got != "success" {
		t.Fatalf("finished session must keep its state, got %q", got)
	}

	// The transition must be durable: a second read sees the same state.
	reloaded, err := st.LoadSessions()
	if err != nil {
		t.Fatalf("reload: %v", err)
	}
	for _, session := range reloaded {
		if session.SessionID == "sess-dead" && session.Status != "failed" {
			t.Fatalf("transition was not persisted, got %q", session.Status)
		}
	}

	// tasks.db must agree with sessions.json on the terminal state.
	task, err := st.GetTask("sess-dead")
	if err != nil {
		t.Fatalf("get task: %v", err)
	}
	if task.Status != "failed" {
		t.Fatalf("tasks.db should agree sessions.json, got %q", task.Status)
	}
}

// Cancelling an already-finished session is a read, not a kill: it must not
// shell out to taskkill and must report the stored terminal state.
func TestCancelFinishedSessionDoesNotKill(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)

	now := "2026-10-19T00:00:00Z"
	exitCode := 0
	if err := st.AppendSession(model.SessionRecord{
		SessionID: "sess-done", ScriptID: "s1", Status: "success",
		// A PID that does not exist: if the code path wrongly reached
		// taskkill it would fail and surface as an error here.
		PID: -1, ExitCode: &exitCode, StartedAt: now, LastChecked: now,
	}); err != nil {
		t.Fatalf("append: %v", err)
	}

	session, err := m.Cancel("sess-done")
	if err != nil {
		t.Fatalf("cancel finished session should be a no-op read, got: %v", err)
	}
	if session.Status != "success" {
		t.Fatalf("terminal state must not be overwritten by cancel, got %q", session.Status)
	}
}

func TestCancelFreshStartupDoesNotTargetPIDZero(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)
	if err := st.AppendSession(model.SessionRecord{
		SessionID: "sess-starting", ScriptID: "s1", Status: "running", PID: 0,
		StartedAt: time.Now().UTC().Format(time.RFC3339),
	}); err != nil {
		t.Fatalf("append: %v", err)
	}
	if _, err := m.Cancel("sess-starting"); err == nil || !strings.Contains(err.Error(), "still starting") {
		t.Fatalf("expected explicit startup error, got %v", err)
	}
}

func TestCancelUnknownSessionReturnsError(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)
	if _, err := m.Cancel("missing"); err == nil {
		t.Fatal("expected error for unknown session")
	}
}

// The worker-side completion path updates exactly one record; a sibling
// session appended in the meantime must survive, mirroring the real
// two-worker interleaving from the F01 acceptance criteria.
func TestCompletionWriteDoesNotEraseSiblingSession(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)

	now := "2026-10-19T00:00:00Z"
	for _, id := range []string{"sess-a", "sess-b"} {
		if err := st.AppendSession(model.SessionRecord{
			SessionID: id, ScriptID: "s1", Status: "running",
			OutputPath: "", StartedAt: now, LastChecked: now,
		}); err != nil {
			t.Fatalf("append %s: %v", id, err)
		}
		if err := st.CreateTask(id, "s1", "{}"); err != nil {
			t.Fatalf("create task %s: %v", id, err)
		}
	}

	// Worker A finishes; B is still running and must be untouched.
	updated, err := st.UpdateSession("sess-a", func(record *model.SessionRecord) error {
		record.Status = "success"
		code := 0
		record.ExitCode = &code
		return nil
	})
	if err != nil {
		t.Fatalf("complete A: %v", err)
	}
	if err := m.syncTask(updated); err != nil {
		t.Fatalf("sync A: %v", err)
	}

	sessions, err := st.LoadSessions()
	if err != nil {
		t.Fatalf("load: %v", err)
	}
	if len(sessions) != 2 {
		t.Fatalf("B was erased by A's completion, got %d sessions", len(sessions))
	}
	for _, session := range sessions {
		if session.SessionID == "sess-b" && session.Status != "running" {
			t.Fatalf("B status changed to %q", session.Status)
		}
	}

	// Both read interfaces agree on A's terminal state.
	taskA, err := st.GetTask("sess-a")
	if err != nil {
		t.Fatalf("get task A: %v", err)
	}
	if taskA.Status != "success" {
		t.Fatalf("tasks.db A = %q, want success", taskA.Status)
	}
}

// syncTask must surface storage errors instead of swallowing them, so a
// diverging tasks.db row is visible rather than silent.
func TestSyncTaskPropagatesStorageErrors(t *testing.T) {
	st := store.New(config.Config{StateDir: t.TempDir()})
	m := NewAsyncManager(st, nil)

	// No CreateTask first: UpdateTaskStatus on a missing row is not an error
	// in SQLite, so drive the failure through an unreadable state dir instead.
	broken := store.New(config.Config{StateDir: filepathJoinImpossible(t)})
	bm := NewAsyncManager(broken, nil)
	session := model.SessionRecord{SessionID: "sess-x", ScriptID: "s1", Status: "success"}
	if err := bm.syncTask(session); err == nil {
		t.Fatal("expected syncTask to surface the storage error")
	}
	// And the healthy path stays quiet.
	if err := m.syncTask(model.SessionRecord{SessionID: "sess-x", ScriptID: "s1", Status: "success"}); err != nil {
		t.Fatalf("healthy syncTask should not error: %v", err)
	}
}

// filepathJoinImpossible returns a state dir path whose parent is a file, so
// opening the database inside it fails on every platform.
func filepathJoinImpossible(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	filePath := dir + string(os.PathSeparator) + "blocker"
	if err := os.WriteFile(filePath, []byte("x"), 0o644); err != nil {
		t.Fatalf("write blocker: %v", err)
	}
	return filePath + string(os.PathSeparator) + "state"
}
