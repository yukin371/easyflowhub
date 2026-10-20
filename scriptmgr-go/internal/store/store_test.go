package store

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"scriptmgr/internal/config"
	"scriptmgr/internal/model"
)

func newTestStore(t *testing.T) *Store {
	t.Helper()
	return New(config.Config{StateDir: t.TempDir()})
}

func runningSession(id string) model.SessionRecord {
	now := time.Now().Format(time.RFC3339)
	return model.SessionRecord{
		SessionID:   id,
		ScriptID:    "script-" + id,
		Status:      "running",
		StartedAt:   now,
		LastChecked: now,
	}
}

// F01 acceptance: a session started by process B while process A is mid-run
// must survive A's completion write. The old implementation loaded the whole
// list before running and wrote it back afterwards, erasing B.
func TestUpdateSessionPreservesConcurrentlyAppendedSession(t *testing.T) {
	s := newTestStore(t)

	if err := s.AppendSession(runningSession("sess-a")); err != nil {
		t.Fatalf("append A: %v", err)
	}

	// B starts while A is still running.
	if err := s.AppendSession(runningSession("sess-b")); err != nil {
		t.Fatalf("append B: %v", err)
	}

	// A completes: only A's record is rewritten.
	updated, err := s.UpdateSession("sess-a", func(record *model.SessionRecord) error {
		record.Status = "success"
		exitCode := 0
		record.ExitCode = &exitCode
		return nil
	})
	if err != nil {
		t.Fatalf("update A: %v", err)
	}
	if updated.Status != "success" {
		t.Fatalf("expected A success, got %q", updated.Status)
	}

	sessions, err := s.LoadSessions()
	if err != nil {
		t.Fatalf("load: %v", err)
	}
	if len(sessions) != 2 {
		t.Fatalf("expected both sessions to survive, got %d: %+v", len(sessions), sessions)
	}
	var foundB bool
	for _, session := range sessions {
		if session.SessionID == "sess-b" {
			foundB = true
			if session.Status != "running" {
				t.Fatalf("B must keep its own status, got %q", session.Status)
			}
		}
	}
	if !foundB {
		t.Fatal("session B was erased by A's completion write")
	}
}

// Two workers interleaving start / complete / cancel style updates must never
// lose a record, regardless of scheduling.
func TestConcurrentSessionMutationsLoseNoRecords(t *testing.T) {
	s := newTestStore(t)

	const workers = 8
	const rounds = 5

	var wg sync.WaitGroup
	errCh := make(chan error, workers*(rounds+1))

	// Every "worker" appends its own session, then flips it to a terminal
	// state, while the others do the same concurrently.
	for w := 0; w < workers; w++ {
		wg.Add(1)
		go func(w int) {
			defer wg.Done()
			id := fmt.Sprintf("sess-%02d", w)
			if err := s.AppendSession(runningSession(id)); err != nil {
				errCh <- fmt.Errorf("worker %d append: %w", w, err)
				return
			}
			for r := 0; r < rounds; r++ {
				if _, err := s.UpdateSession(id, func(record *model.SessionRecord) error {
					record.LastChecked = time.Now().Format(time.RFC3339)
					return nil
				}); err != nil {
					errCh <- fmt.Errorf("worker %d touch: %w", w, err)
					return
				}
			}
			if _, err := s.UpdateSession(id, func(record *model.SessionRecord) error {
				record.Status = "success"
				exitCode := 0
				record.ExitCode = &exitCode
				record.FinishedAt = time.Now().Format(time.RFC3339)
				return nil
			}); err != nil {
				errCh <- fmt.Errorf("worker %d finish: %w", w, err)
			}
		}(w)
	}
	wg.Wait()
	close(errCh)
	for err := range errCh {
		t.Error(err)
	}

	sessions, err := s.LoadSessions()
	if err != nil {
		t.Fatalf("load: %v", err)
	}
	if len(sessions) != workers {
		t.Fatalf("expected %d sessions, got %d", workers, len(sessions))
	}
	for _, session := range sessions {
		if session.Status != "success" {
			t.Errorf("session %s ended as %q, want success", session.SessionID, session.Status)
		}
	}
}

func TestGetSessionReturnsSingleRecord(t *testing.T) {
	s := newTestStore(t)
	if err := s.AppendSession(runningSession("sess-a")); err != nil {
		t.Fatalf("append: %v", err)
	}
	if err := s.AppendSession(runningSession("sess-b")); err != nil {
		t.Fatalf("append: %v", err)
	}

	record, err := s.GetSession("sess-b")
	if err != nil {
		t.Fatalf("get: %v", err)
	}
	if record.SessionID != "sess-b" {
		t.Fatalf("expected sess-b, got %s", record.SessionID)
	}

	if _, err := s.GetSession("missing"); err == nil {
		t.Fatal("expected error for missing session")
	}
}

func TestUpdateSessionRejectsUnknownID(t *testing.T) {
	s := newTestStore(t)
	if _, err := s.UpdateSession("missing", func(*model.SessionRecord) error { return nil }); err == nil {
		t.Fatal("expected error for missing session")
	}
}

// A lock left behind by a crashed process must not wedge every future run.
func TestSessionsLockStaleFileIsBroken(t *testing.T) {
	dir := t.TempDir()
	s := New(config.Config{StateDir: dir})

	lockPath := filepath.Join(dir, "sessions.lock")
	if err := os.WriteFile(lockPath, []byte("999999"), 0o644); err != nil {
		t.Fatalf("write stale lock: %v", err)
	}
	stale := time.Now().Add(-sessionsLockStale - time.Second)
	if err := os.Chtimes(lockPath, stale, stale); err != nil {
		t.Fatalf("age stale lock: %v", err)
	}

	if err := s.AppendSession(runningSession("sess-a")); err != nil {
		t.Fatalf("append with stale lock present: %v", err)
	}
	if _, err := os.Stat(lockPath); !os.IsNotExist(err) {
		t.Fatalf("lock file should be released after the operation, stat err: %v", err)
	}
}

// A genuinely held lock must serialise writers instead of corrupting state.
func TestSessionsLockHeldTimeoutsCleanly(t *testing.T) {
	dir := t.TempDir()
	s := New(config.Config{StateDir: dir})

	origWait := sessionsLockWait
	sessionsLockWait = 200 * time.Millisecond
	defer func() { sessionsLockWait = origWait }()

	lockPath := filepath.Join(dir, "sessions.lock")
	fresh, err := os.OpenFile(lockPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o644)
	if err != nil {
		t.Fatalf("create lock: %v", err)
	}
	defer fresh.Close()

	start := time.Now()
	_, err = s.UpdateSession("whatever", func(*model.SessionRecord) error { return nil })
	elapsed := time.Since(start)
	if err == nil {
		t.Fatal("expected a timeout error while the lock is held")
	}
	if !strings.Contains(err.Error(), "timed out") {
		t.Fatalf("expected timeout error, got: %v", err)
	}
	if elapsed < sessionsLockWait {
		t.Fatalf("gave up too early after %v, want ~%v", elapsed, sessionsLockWait)
	}
}

// This helper is launched as a separate OS process by the test below. A
// goroutine-only test cannot prove that the O_EXCL lock coordinates workers
// that have independent address spaces.
func TestStoreLockSubprocessHelper(t *testing.T) {
	if os.Getenv("GO_WANT_STORE_LOCK_HELPER") != "1" {
		return
	}
	stateDir := os.Getenv("STORE_LOCK_STATE_DIR")
	sessionID := os.Getenv("STORE_LOCK_SESSION_ID")
	s := New(config.Config{StateDir: stateDir})
	if err := s.AppendSession(runningSession(sessionID)); err != nil {
		t.Fatalf("append from subprocess: %v", err)
	}
}

func TestSessionsLockCoordinatesAcrossProcesses(t *testing.T) {
	stateDir := t.TempDir()
	const workers = 5

	commands := make([]*exec.Cmd, 0, workers)
	for i := 0; i < workers; i++ {
		cmd := exec.Command(os.Args[0], "-test.run=TestStoreLockSubprocessHelper", "-test.v")
		cmd.Env = append(os.Environ(),
			"GO_WANT_STORE_LOCK_HELPER=1",
			"STORE_LOCK_STATE_DIR="+stateDir,
			fmt.Sprintf("STORE_LOCK_SESSION_ID=process-%02d", i),
		)
		if err := cmd.Start(); err != nil {
			t.Fatalf("start subprocess %d: %v", i, err)
		}
		commands = append(commands, cmd)
	}

	for i, cmd := range commands {
		if err := cmd.Wait(); err != nil {
			t.Fatalf("subprocess %d failed: %v", i, err)
		}
	}

	s := New(config.Config{StateDir: stateDir})
	sessions, err := s.LoadSessions()
	if err != nil {
		t.Fatalf("load sessions after subprocesses: %v", err)
	}
	if len(sessions) != workers {
		t.Fatalf("expected %d sessions from independent processes, got %d: %+v", workers, len(sessions), sessions)
	}
}

func TestConcurrentHistoryAppendsPreserveAllEntries(t *testing.T) {
	s := newTestStore(t)
	const writers = 24

	var wg sync.WaitGroup
	errCh := make(chan error, writers)
	for i := 0; i < writers; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			if err := s.AppendHistory(model.HistoryEntry{
				HistoryID: fmt.Sprintf("history-%02d", i),
				ScriptID:  fmt.Sprintf("script-%02d", i),
				Status:    "success",
			}); err != nil {
				errCh <- err
			}
		}(i)
	}
	wg.Wait()
	close(errCh)
	for err := range errCh {
		t.Error(err)
	}

	entries, err := s.LoadHistory()
	if err != nil {
		t.Fatalf("load history: %v", err)
	}
	if len(entries) != writers {
		t.Fatalf("expected %d history entries, got %d", writers, len(entries))
	}
	seen := make(map[string]bool, len(entries))
	for _, entry := range entries {
		seen[entry.HistoryID] = true
	}
	for i := 0; i < writers; i++ {
		if !seen[fmt.Sprintf("history-%02d", i)] {
			t.Errorf("missing history-%02d", i)
		}
	}
}
