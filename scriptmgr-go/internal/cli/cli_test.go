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

func TestParseServeAddressDefaultsToLoopback(t *testing.T) {
	addr, showHelp, err := parseServeAddress(nil)
	if err != nil || showHelp || addr != "127.0.0.1:8765" {
		t.Fatalf("expected default loopback address, got addr=%q help=%v err=%v", addr, showHelp, err)
	}
}

func TestParseServeAddressPortFlagUsesLoopback(t *testing.T) {
	addr, _, err := parseServeAddress([]string{"--port", "9000"})
	if err != nil || addr != "127.0.0.1:9000" {
		t.Fatalf("expected loopback port address, got %q: %v", addr, err)
	}
}

func TestParseServeAddressRejectsNonLoopbackAddresses(t *testing.T) {
	for _, input := range []string{":3000", "0.0.0.0:3000", "192.168.1.20:3000"} {
		if addr, _, err := parseServeAddress([]string{input}); err == nil {
			t.Errorf("expected non-loopback address %q to be rejected, got %q", input, addr)
		}
	}
}

func TestParseServeAddressAcceptsLoopbackAddresses(t *testing.T) {
	for _, input := range []string{"127.0.0.1:3000", "[::1]:3000"} {
		addr, _, err := parseServeAddress([]string{input})
		if err != nil || addr != input {
			t.Errorf("expected loopback address %q, got %q: %v", input, addr, err)
		}
	}
}

func TestParseRelayServeAddressDefaultsToLoopback(t *testing.T) {
	addr, showHelp, err := parseRelayServeAddress(nil)
	if err != nil || showHelp || addr != "127.0.0.1:8787" {
		t.Fatalf("expected default Relay loopback address, got addr=%q help=%v err=%v", addr, showHelp, err)
	}
}

func TestParseRelayServeAddressCustomPortUsesLoopback(t *testing.T) {
	addr, _, err := parseRelayServeAddress([]string{"--port", "9000"})
	if err != nil || addr != "127.0.0.1:9000" {
		t.Fatalf("expected Relay loopback port address, got %q: %v", addr, err)
	}
}
