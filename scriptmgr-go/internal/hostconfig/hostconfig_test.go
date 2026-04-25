package hostconfig

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSwitchClaudeUpdatesBaseURL(t *testing.T) {
	tempDir := t.TempDir()
	path := filepath.Join(tempDir, ".claude", "settings.json")
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}
	initial := `{
  "env": {
    "ANTHROPIC_BASE_URL": "https://old.example.com",
    "ANTHROPIC_AUTH_TOKEN": "secret"
  },
  "model": "claude-sonnet"
}
`
	if err := os.WriteFile(path, []byte(initial), 0644); err != nil {
		t.Fatalf("write initial settings: %v", err)
	}

	status, err := SwitchClaude(path, "https://new.example.com/anthropic")
	if err != nil {
		t.Fatalf("SwitchClaude failed: %v", err)
	}

	if status.BaseURL != "https://new.example.com/anthropic" {
		t.Fatalf("unexpected base url: %s", status.BaseURL)
	}
	if status.Backup == "" {
		t.Fatal("expected backup path to be recorded")
	}
	if _, err := os.Stat(status.Backup); err != nil {
		t.Fatalf("expected backup file: %v", err)
	}
}

func TestSwitchCodexCreatesProviderBlock(t *testing.T) {
	tempDir := t.TempDir()
	path := filepath.Join(tempDir, ".codex", "config.toml")
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatalf("mkdir: %v", err)
	}
	initial := `model_provider = "custom"
model = "gpt-5.5"

[model_providers.custom]
name = "custom"
wire_api = "responses"
requires_openai_auth = true
base_url = "http://127.0.0.1:8787/v1"
`
	if err := os.WriteFile(path, []byte(initial), 0644); err != nil {
		t.Fatalf("write initial config: %v", err)
	}

	status, err := SwitchCodex(path, "relay", "http://127.0.0.1:15721/v1")
	if err != nil {
		t.Fatalf("SwitchCodex failed: %v", err)
	}
	if status.Provider != "relay" {
		t.Fatalf("unexpected provider: %s", status.Provider)
	}
	if status.BaseURL != "http://127.0.0.1:15721/v1" {
		t.Fatalf("unexpected base url: %s", status.BaseURL)
	}

	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}
	text := string(content)
	if !strings.Contains(text, `model_provider = "relay"`) {
		t.Fatalf("expected root provider switch, got:\n%s", text)
	}
	if !strings.Contains(text, "[model_providers.relay]") {
		t.Fatalf("expected relay provider section, got:\n%s", text)
	}
}

func TestDetectReturnsCurrentHostState(t *testing.T) {
	tempDir := t.TempDir()

	claudePath := DefaultClaudePath(tempDir)
	if err := os.MkdirAll(filepath.Dir(claudePath), 0755); err != nil {
		t.Fatalf("mkdir claude: %v", err)
	}
	if err := os.WriteFile(claudePath, []byte(`{"env":{"ANTHROPIC_BASE_URL":"https://claude.example.com"},"model":"glm-5.0"}`), 0644); err != nil {
		t.Fatalf("write claude config: %v", err)
	}

	codexPath := DefaultCodexPath(tempDir)
	if err := os.MkdirAll(filepath.Dir(codexPath), 0755); err != nil {
		t.Fatalf("mkdir codex: %v", err)
	}
	if err := os.WriteFile(codexPath, []byte(`model_provider = "relay"

[model_providers.relay]
name = "relay"
wire_api = "responses"
requires_openai_auth = true
base_url = "http://127.0.0.1:8787/v1"
`), 0644); err != nil {
		t.Fatalf("write codex config: %v", err)
	}

	status, err := Detect(tempDir)
	if err != nil {
		t.Fatalf("Detect failed: %v", err)
	}

	if status.Claude.BaseURL != "https://claude.example.com" {
		t.Fatalf("unexpected claude url: %s", status.Claude.BaseURL)
	}
	if status.Codex.BaseURL != "http://127.0.0.1:8787/v1" {
		t.Fatalf("unexpected codex url: %s", status.Codex.BaseURL)
	}
}

func TestPreviewSwitchDoesNotWriteFiles(t *testing.T) {
	tempDir := t.TempDir()

	claudePath := DefaultClaudePath(tempDir)
	if err := os.MkdirAll(filepath.Dir(claudePath), 0755); err != nil {
		t.Fatalf("mkdir claude: %v", err)
	}
	initialClaude := `{"env":{"ANTHROPIC_BASE_URL":"https://old.example.com"},"model":"glm-5.0"}`
	if err := os.WriteFile(claudePath, []byte(initialClaude), 0644); err != nil {
		t.Fatalf("write claude config: %v", err)
	}

	preview, err := PreviewSwitch(SwitchOptions{
		HomeDir:       tempDir,
		TargetHost:    "claude",
		ClaudeBaseURL: "https://new.example.com/anthropic",
	})
	if err != nil {
		t.Fatalf("PreviewSwitch failed: %v", err)
	}

	if preview.Claude == nil || !preview.Claude.Changed {
		t.Fatalf("expected changed claude preview, got %+v", preview.Claude)
	}
	if len(preview.Claude.Diff) != 1 || preview.Claude.Diff[0].Field != "base_url" {
		t.Fatalf("expected base_url-only diff, got %+v", preview.Claude.Diff)
	}
	content, err := os.ReadFile(claudePath)
	if err != nil {
		t.Fatalf("read claude config: %v", err)
	}
	if string(content) != initialClaude {
		t.Fatalf("preview should not write file, got:\n%s", string(content))
	}
}

func TestPreviewCodexShowsUpdatedProviderBlock(t *testing.T) {
	tempDir := t.TempDir()
	path := filepath.Join(tempDir, ".codex", "config.toml")
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatalf("mkdir codex: %v", err)
	}
	initial := `model_provider = "custom"
model = "gpt-5.5"
`
	if err := os.WriteFile(path, []byte(initial), 0644); err != nil {
		t.Fatalf("write codex config: %v", err)
	}

	preview, err := PreviewCodexSwitch(path, "relay", "http://127.0.0.1:8787/v1")
	if err != nil {
		t.Fatalf("PreviewCodexSwitch failed: %v", err)
	}
	if !preview.Changed {
		t.Fatal("expected codex preview to report changes")
	}
	if !strings.Contains(preview.After, `model_provider = "relay"`) {
		t.Fatalf("expected updated provider in preview, got:\n%s", preview.After)
	}
	if !strings.Contains(preview.After, `[model_providers.relay]`) {
		t.Fatalf("expected relay provider block in preview, got:\n%s", preview.After)
	}
	if len(preview.Diff) < 2 {
		t.Fatalf("expected structured diff entries, got %+v", preview.Diff)
	}
}
