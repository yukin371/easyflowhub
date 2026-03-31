package cli

import (
	"testing"

	"scriptmgr/internal/api"
)

func TestRunUnknownCommand(t *testing.T) {
	mockAPI := &api.API{}

	// Test: unknown command should return error
	if err := Run([]string{"unknown-command"}, mockAPI); err == nil {
		t.Error("Expected error for unknown command")
	}
}

func TestRunMissingCommand(t *testing.T) {
	mockAPI := &api.API{}

	// Test: missing command should return error
	if err := Run([]string{}, mockAPI); err == nil {
		t.Error("Expected error for missing command")
	}
}
