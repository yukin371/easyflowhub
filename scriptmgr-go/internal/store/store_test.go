package store

import (
	"fmt"
	"os"
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
