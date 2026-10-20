package mcp

import (
	"context"
	"errors"
	"testing"
	"time"

	"scriptmgr/internal/model"
)

// seqGetter returns a task whose status advances through the given sequence,
// holding the final status once the sequence is exhausted.
func seqGetter(statuses ...string) func() (*model.Task, error) {
	i := 0
	return func() (*model.Task, error) {
		status := statuses[i]
		if i < len(statuses)-1 {
			i++
		}
		return &model.Task{TaskID: "t1", Status: status}, nil
	}
}

// countingSleeper records calls and optionally returns a fixed error.
type countingSleeper struct {
	calls int
	err   error
}

func (s *countingSleeper) sleep(ctx context.Context, d time.Duration) error {
	s.calls++
	return s.err
}

func TestWaitCompletionReturnsWhenTaskCompletes(t *testing.T) {
	sleeper := &countingSleeper{}
	w := &taskWaiter{interval: time.Millisecond, timeout: time.Minute, sleep: sleeper.sleep}
	task, timedOut, err := w.waitCompletion(context.Background(), seqGetter("running", "running", "completed"))
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if timedOut {
		t.Fatalf("expected completion, not timeout")
	}
	if task.Status != "completed" {
		t.Fatalf("expected completed task, got %q", task.Status)
	}
	if sleeper.calls != 2 {
		t.Fatalf("expected 2 sleeps between polls, got %d", sleeper.calls)
	}
}

func TestWaitCompletionTimesOutWhenTaskStaysRunning(t *testing.T) {
	sleeper := &countingSleeper{}
	w := &taskWaiter{interval: time.Millisecond, timeout: 3 * time.Millisecond, sleep: sleeper.sleep}
	task, timedOut, err := w.waitCompletion(context.Background(), seqGetter("running"))
	if err != nil {
		t.Fatalf("expected no error on timeout, got %v", err)
	}
	if !timedOut {
		t.Fatalf("expected timeout for a task that never finishes")
	}
	if task.Status != "running" {
		t.Fatalf("expected last observed status running, got %q", task.Status)
	}
}

func TestWaitCompletionHonorsCancellation(t *testing.T) {
	sleeper := &countingSleeper{err: context.Canceled}
	w := &taskWaiter{interval: time.Millisecond, timeout: time.Minute, sleep: sleeper.sleep}
	_, _, err := w.waitCompletion(context.Background(), seqGetter("running"))
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("expected context.Canceled, got %v", err)
	}
}

func TestWaitCompletionPropagatesGetterError(t *testing.T) {
	want := errors.New("boom")
	w := &taskWaiter{interval: time.Millisecond, timeout: time.Minute, sleep: (&countingSleeper{}).sleep}
	task, _, err := w.waitCompletion(context.Background(), func() (*model.Task, error) {
		return nil, want
	})
	if !errors.Is(err, want) {
		t.Fatalf("expected %v, got %v", want, err)
	}
	if task != nil {
		t.Fatalf("expected no task on getter error")
	}
}

func TestStaticSchemasAllHaveRegisteredHandlers(t *testing.T) {
	srv := New().WithAPI(&MockAPI{})
	registered := make(map[string]bool)
	for _, tool := range srv.router.GetDiscoveryTools() {
		registered[tool.Name] = true
	}
	for _, schema := range GetToolSchemas() {
		if !registered[schema.Name] {
			t.Errorf("schema %q is advertised but has no registered handler", schema.Name)
		}
	}
	for name := range registered {
		found := false
		for _, schema := range GetToolSchemas() {
			if schema.Name == name {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("registered tool %q is not present in the schema source", name)
		}
	}
}
