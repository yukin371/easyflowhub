package hostconfig

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	defaultClaudeConfigName = "settings.json"
	defaultCodexConfigName  = "config.toml"
	defaultCodexProvider    = "relay"
)

type ClaudeStatus struct {
	Path      string `json:"path"`
	Exists    bool   `json:"exists"`
	BaseURL   string `json:"base_url,omitempty"`
	Model     string `json:"model,omitempty"`
	Managed   bool   `json:"managed"`
	Backup    string `json:"backup,omitempty"`
	Changed   bool   `json:"changed"`
	AuthToken bool   `json:"auth_token_present"`
}

type CodexStatus struct {
	Path         string `json:"path"`
	Exists       bool   `json:"exists"`
	ActiveModel  string `json:"active_model,omitempty"`
	Provider     string `json:"provider,omitempty"`
	BaseURL      string `json:"base_url,omitempty"`
	Managed      bool   `json:"managed"`
	Backup       string `json:"backup,omitempty"`
	Changed      bool   `json:"changed"`
	WireAPI      string `json:"wire_api,omitempty"`
	RequiresAuth bool   `json:"requires_openai_auth"`
}

type HostStatus struct {
	Claude ClaudeStatus `json:"claude"`
	Codex  CodexStatus  `json:"codex"`
}

type FilePreview struct {
	Path    string      `json:"path"`
	Exists  bool        `json:"exists"`
	Changed bool        `json:"changed"`
	Diff    []DiffEntry `json:"diff,omitempty"`
	Before  string      `json:"before,omitempty"`
	After   string      `json:"after,omitempty"`
}

type DiffEntry struct {
	Field  string `json:"field"`
	Before string `json:"before,omitempty"`
	After  string `json:"after,omitempty"`
}

type SwitchPreview struct {
	TargetHost string       `json:"target_host"`
	Claude     *FilePreview `json:"claude,omitempty"`
	Codex      *FilePreview `json:"codex,omitempty"`
}

type SwitchOptions struct {
	HomeDir       string
	TargetHost    string
	ClaudeBaseURL string
	CodexBaseURL  string
	CodexProvider string
}

type claudeSettings struct {
	Env                     map[string]string `json:"env,omitempty"`
	Model                   string            `json:"model,omitempty"`
	SkipDangerousModePrompt bool              `json:"skipDangerousModePermissionPrompt,omitempty"`
	EnabledPlugins          map[string]bool   `json:"enabledPlugins,omitempty"`
	Permissions             map[string]any    `json:"permissions,omitempty"`
}

func DefaultClaudePath(homeDir string) string {
	return filepath.Join(homeDir, ".claude", defaultClaudeConfigName)
}

func DefaultCodexPath(homeDir string) string {
	return filepath.Join(homeDir, ".codex", defaultCodexConfigName)
}

func Detect(homeDir string) (HostStatus, error) {
	claude, err := DetectClaude(DefaultClaudePath(homeDir))
	if err != nil {
		return HostStatus{}, err
	}
	codex, err := DetectCodex(DefaultCodexPath(homeDir))
	if err != nil {
		return HostStatus{}, err
	}
	return HostStatus{
		Claude: claude,
		Codex:  codex,
	}, nil
}

func Switch(opts SwitchOptions) (HostStatus, error) {
	target, err := normalizeTarget(opts)
	if err != nil {
		return HostStatus{}, err
	}

	result, err := Detect(opts.HomeDir)
	if err != nil {
		return HostStatus{}, err
	}

	if target == "all" || target == "claude" {
		claudeStatus, err := SwitchClaude(DefaultClaudePath(opts.HomeDir), opts.ClaudeBaseURL)
		if err != nil {
			return HostStatus{}, err
		}
		result.Claude = claudeStatus
	}

	if target == "all" || target == "codex" {
		codexStatus, err := SwitchCodex(DefaultCodexPath(opts.HomeDir), opts.CodexProvider, opts.CodexBaseURL)
		if err != nil {
			return HostStatus{}, err
		}
		result.Codex = codexStatus
	}

	return result, nil
}

func PreviewSwitch(opts SwitchOptions) (SwitchPreview, error) {
	target, err := normalizeTarget(opts)
	if err != nil {
		return SwitchPreview{}, err
	}

	result := SwitchPreview{TargetHost: target}

	if target == "all" || target == "claude" {
		preview, err := PreviewClaudeSwitch(DefaultClaudePath(opts.HomeDir), opts.ClaudeBaseURL)
		if err != nil {
			return SwitchPreview{}, err
		}
		result.Claude = &preview
	}

	if target == "all" || target == "codex" {
		preview, err := PreviewCodexSwitch(DefaultCodexPath(opts.HomeDir), opts.CodexProvider, opts.CodexBaseURL)
		if err != nil {
			return SwitchPreview{}, err
		}
		result.Codex = &preview
	}

	return result, nil
}

func DetectClaude(path string) (ClaudeStatus, error) {
	status := ClaudeStatus{Path: path}
	content, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return status, nil
		}
		return ClaudeStatus{}, err
	}
	status.Exists = true

	var settings claudeSettings
	if err := json.Unmarshal(content, &settings); err != nil {
		return ClaudeStatus{}, fmt.Errorf("parse claude settings: %w", err)
	}

	status.Model = settings.Model
	if settings.Env != nil {
		status.BaseURL = strings.TrimSpace(settings.Env["ANTHROPIC_BASE_URL"])
		status.AuthToken = strings.TrimSpace(settings.Env["ANTHROPIC_AUTH_TOKEN"]) != ""
	}
	status.Managed = status.BaseURL != ""
	return status, nil
}

func SwitchClaude(path, baseURL string) (ClaudeStatus, error) {
	content, encoded, changed, err := buildClaudeContent(path, baseURL)
	if err != nil {
		return ClaudeStatus{}, err
	}

	backupPath := ""
	if len(content) > 0 {
		backupPath, err = backupAndWrite(path, content, encoded)
		if err != nil {
			return ClaudeStatus{}, err
		}
	} else {
		if err := ensureParentDir(path); err != nil {
			return ClaudeStatus{}, err
		}
		if err := atomicWrite(path, encoded); err != nil {
			return ClaudeStatus{}, err
		}
	}

	status, err := DetectClaude(path)
	if err != nil {
		return ClaudeStatus{}, err
	}
	status.Backup = backupPath
	status.Changed = changed
	return status, nil
}

func PreviewClaudeSwitch(path, baseURL string) (FilePreview, error) {
	content, encoded, changed, err := buildClaudeContent(path, baseURL)
	if err != nil {
		return FilePreview{}, err
	}
	beforeStatus, _ := DetectClaude(path)
	afterStatus, err := detectClaudeFromContent(path, encoded)
	if err != nil {
		return FilePreview{}, err
	}
	return FilePreview{
		Path:    path,
		Exists:  len(content) > 0,
		Changed: changed,
		Diff:    diffClaude(beforeStatus, afterStatus),
		Before:  string(content),
		After:   string(encoded),
	}, nil
}

func buildClaudeContent(path, baseURL string) ([]byte, []byte, bool, error) {
	settings := claudeSettings{}
	content, err := os.ReadFile(path)
	if err != nil && !os.IsNotExist(err) {
		return nil, nil, false, err
	}
	if len(content) > 0 {
		if err := json.Unmarshal(content, &settings); err != nil {
			return nil, nil, false, fmt.Errorf("parse claude settings: %w", err)
		}
	}
	if settings.Env == nil {
		settings.Env = make(map[string]string)
	}

	nextBaseURL := strings.TrimSpace(baseURL)
	prevBaseURL := strings.TrimSpace(settings.Env["ANTHROPIC_BASE_URL"])
	settings.Env["ANTHROPIC_BASE_URL"] = nextBaseURL

	encoded, err := marshalIndentedJSON(settings)
	if err != nil {
		return nil, nil, false, err
	}

	return content, encoded, prevBaseURL != nextBaseURL || len(content) == 0, nil
}

func DetectCodex(path string) (CodexStatus, error) {
	status := CodexStatus{Path: path}
	content, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return status, nil
		}
		return CodexStatus{}, err
	}
	status.Exists = true

	status.ActiveModel = parseRootString(content, "model")
	status.Provider = parseRootString(content, "model_provider")
	if status.Provider == "" {
		status.Provider = defaultCodexProvider
	}
	status.BaseURL = parseProviderString(content, status.Provider, "base_url")
	status.WireAPI = parseProviderString(content, status.Provider, "wire_api")
	status.RequiresAuth = parseProviderBool(content, status.Provider, "requires_openai_auth")
	status.Managed = status.BaseURL != ""
	return status, nil
}

func SwitchCodex(path, providerName, baseURL string) (CodexStatus, error) {
	content, next, changed, err := buildCodexContent(path, providerName, baseURL)
	if err != nil {
		return CodexStatus{}, err
	}

	backupPath := ""
	if len(content) > 0 {
		backupPath, err = backupAndWrite(path, content, next)
		if err != nil {
			return CodexStatus{}, err
		}
	} else {
		if err := ensureParentDir(path); err != nil {
			return CodexStatus{}, err
		}
		if err := atomicWrite(path, next); err != nil {
			return CodexStatus{}, err
		}
	}
	status, err := DetectCodex(path)
	if err != nil {
		return CodexStatus{}, err
	}
	status.Backup = backupPath
	status.Changed = changed
	return status, nil
}

func PreviewCodexSwitch(path, providerName, baseURL string) (FilePreview, error) {
	content, next, changed, err := buildCodexContent(path, providerName, baseURL)
	if err != nil {
		return FilePreview{}, err
	}
	beforeStatus, _ := DetectCodex(path)
	afterStatus, err := detectCodexFromContent(path, next)
	if err != nil {
		return FilePreview{}, err
	}
	return FilePreview{
		Path:    path,
		Exists:  len(content) > 0,
		Changed: changed,
		Diff:    diffCodex(beforeStatus, afterStatus),
		Before:  string(content),
		After:   string(next),
	}, nil
}

func buildCodexContent(path, providerName, baseURL string) ([]byte, []byte, bool, error) {
	if strings.TrimSpace(providerName) == "" {
		providerName = defaultCodexProvider
	}
	content, err := os.ReadFile(path)
	if err != nil && !os.IsNotExist(err) {
		return nil, nil, false, err
	}

	var next []byte
	if len(content) == 0 {
		next = []byte(strings.Join([]string{
			fmt.Sprintf("model_provider = %q", providerName),
			"",
			fmt.Sprintf("[model_providers.%s]", providerName),
			fmt.Sprintf("name = %q", providerName),
			`wire_api = "responses"`,
			"requires_openai_auth = true",
			fmt.Sprintf("base_url = %q", strings.TrimSpace(baseURL)),
			"",
		}, "\n"))
	} else {
		updated := string(content)
		updated = setRootValue(updated, "model_provider", fmt.Sprintf("%q", providerName))
		updated = upsertProviderSection(updated, providerName, map[string]string{
			"name":                 fmt.Sprintf("%q", providerName),
			"wire_api":             `"responses"`,
			"requires_openai_auth": "true",
			"base_url":             fmt.Sprintf("%q", strings.TrimSpace(baseURL)),
		})
		next = []byte(normalizeTrailingNewline(updated))
	}

	return content, next, !bytes.Equal(content, next), nil
}

func marshalIndentedJSON(value any) ([]byte, error) {
	encoded, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return nil, err
	}
	encoded = append(encoded, '\n')
	return encoded, nil
}

func backupAndWrite(path string, original, next []byte) (string, error) {
	if bytes.Equal(original, next) {
		return "", nil
	}
	if err := ensureParentDir(path); err != nil {
		return "", err
	}
	backupPath := fmt.Sprintf("%s.bak-%s", path, time.Now().Format("20060102-150405"))
	if err := os.WriteFile(backupPath, original, 0644); err != nil {
		return "", err
	}
	if err := atomicWrite(path, next); err != nil {
		return "", err
	}
	return backupPath, nil
}

func atomicWrite(path string, content []byte) error {
	tempFile, err := os.CreateTemp(filepath.Dir(path), filepath.Base(path)+".tmp-*")
	if err != nil {
		return err
	}
	tempPath := tempFile.Name()
	defer os.Remove(tempPath)

	if _, err := tempFile.Write(content); err != nil {
		tempFile.Close()
		return err
	}
	if err := tempFile.Close(); err != nil {
		return err
	}
	return os.Rename(tempPath, path)
}

func ensureParentDir(path string) error {
	return os.MkdirAll(filepath.Dir(path), 0755)
}

func parseRootString(content []byte, key string) string {
	lines := strings.Split(string(content), "\n")
	currentSection := ""
	for _, raw := range lines {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			currentSection = strings.TrimSpace(line[1 : len(line)-1])
			continue
		}
		if currentSection != "" {
			continue
		}
		if strings.HasPrefix(line, key+" = ") {
			return trimTOMLValue(strings.TrimSpace(strings.TrimPrefix(line, key+" = ")))
		}
	}
	return ""
}

func parseProviderString(content []byte, providerName, key string) string {
	lines := strings.Split(string(content), "\n")
	currentSection := ""
	for _, raw := range lines {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			currentSection = strings.TrimSpace(line[1 : len(line)-1])
			continue
		}
		if currentSection != fmt.Sprintf("model_providers.%s", providerName) {
			continue
		}
		if strings.HasPrefix(line, key+" = ") {
			return trimTOMLValue(strings.TrimSpace(strings.TrimPrefix(line, key+" = ")))
		}
	}
	return ""
}

func parseProviderBool(content []byte, providerName, key string) bool {
	return strings.EqualFold(parseProviderString(content, providerName, key), "true")
}

func trimTOMLValue(value string) string {
	return strings.Trim(value, `"'`)
}

func setRootValue(content, key, value string) string {
	lines := strings.Split(content, "\n")
	currentSection := ""
	for i, raw := range lines {
		line := strings.TrimSpace(raw)
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			currentSection = strings.TrimSpace(line[1 : len(line)-1])
			continue
		}
		if currentSection != "" {
			continue
		}
		if strings.HasPrefix(line, key+" = ") {
			lines[i] = fmt.Sprintf("%s = %s", key, value)
			return strings.Join(lines, "\n")
		}
	}

	insertAt := 0
	for insertAt < len(lines) && strings.TrimSpace(lines[insertAt]) != "" {
		insertAt++
	}
	insert := fmt.Sprintf("%s = %s", key, value)
	if insertAt >= len(lines) {
		return normalizeTrailingNewline(strings.Join(append(lines, insert), "\n"))
	}
	updated := append([]string{}, lines[:insertAt]...)
	updated = append(updated, insert)
	updated = append(updated, lines[insertAt:]...)
	return strings.Join(updated, "\n")
}

func upsertProviderSection(content, providerName string, values map[string]string) string {
	lines := strings.Split(content, "\n")
	targetSection := fmt.Sprintf("model_providers.%s", providerName)
	sectionStart := -1
	sectionEnd := len(lines)

	for i, raw := range lines {
		line := strings.TrimSpace(raw)
		if !strings.HasPrefix(line, "[") || !strings.HasSuffix(line, "]") {
			continue
		}
		section := strings.TrimSpace(line[1 : len(line)-1])
		if section == targetSection {
			sectionStart = i
			continue
		}
		if sectionStart >= 0 {
			sectionEnd = i
			break
		}
	}

	if sectionStart == -1 {
		block := []string{
			"",
			fmt.Sprintf("[%s]", targetSection),
			fmt.Sprintf("name = %s", values["name"]),
			fmt.Sprintf("wire_api = %s", values["wire_api"]),
			fmt.Sprintf("requires_openai_auth = %s", values["requires_openai_auth"]),
			fmt.Sprintf("base_url = %s", values["base_url"]),
		}
		return normalizeTrailingNewline(strings.Join(append(lines, block...), "\n"))
	}

	section := append([]string{}, lines[sectionStart:sectionEnd]...)
	section = setOrAppendKey(section, "name", values["name"])
	section = setOrAppendKey(section, "wire_api", values["wire_api"])
	section = setOrAppendKey(section, "requires_openai_auth", values["requires_openai_auth"])
	section = setOrAppendKey(section, "base_url", values["base_url"])

	updated := append([]string{}, lines[:sectionStart]...)
	updated = append(updated, section...)
	updated = append(updated, lines[sectionEnd:]...)
	return strings.Join(updated, "\n")
}

func setOrAppendKey(lines []string, key, value string) []string {
	for i := 1; i < len(lines); i++ {
		line := strings.TrimSpace(lines[i])
		if strings.HasPrefix(line, key+" = ") {
			lines[i] = fmt.Sprintf("%s = %s", key, value)
			return lines
		}
	}
	return append(lines, fmt.Sprintf("%s = %s", key, value))
}

func normalizeTrailingNewline(content string) string {
	if strings.HasSuffix(content, "\n") {
		return content
	}
	return content + "\n"
}

func detectClaudeFromContent(path string, content []byte) (ClaudeStatus, error) {
	status := ClaudeStatus{Path: path, Exists: len(content) > 0}
	if len(content) == 0 {
		return status, nil
	}
	var settings claudeSettings
	if err := json.Unmarshal(content, &settings); err != nil {
		return ClaudeStatus{}, fmt.Errorf("parse claude settings: %w", err)
	}
	status.Model = settings.Model
	if settings.Env != nil {
		status.BaseURL = strings.TrimSpace(settings.Env["ANTHROPIC_BASE_URL"])
		status.AuthToken = strings.TrimSpace(settings.Env["ANTHROPIC_AUTH_TOKEN"]) != ""
	}
	status.Managed = status.BaseURL != ""
	return status, nil
}

func detectCodexFromContent(path string, content []byte) (CodexStatus, error) {
	status := CodexStatus{Path: path, Exists: len(content) > 0}
	if len(content) == 0 {
		return status, nil
	}
	status.ActiveModel = parseRootString(content, "model")
	status.Provider = parseRootString(content, "model_provider")
	if status.Provider == "" {
		status.Provider = defaultCodexProvider
	}
	status.BaseURL = parseProviderString(content, status.Provider, "base_url")
	status.WireAPI = parseProviderString(content, status.Provider, "wire_api")
	status.RequiresAuth = parseProviderBool(content, status.Provider, "requires_openai_auth")
	status.Managed = status.BaseURL != ""
	return status, nil
}

func diffClaude(before, after ClaudeStatus) []DiffEntry {
	var diff []DiffEntry
	diff = appendDiff(diff, "base_url", before.BaseURL, after.BaseURL)
	diff = appendDiff(diff, "model", before.Model, after.Model)
	return diff
}

func diffCodex(before, after CodexStatus) []DiffEntry {
	var diff []DiffEntry
	diff = appendDiff(diff, "provider", before.Provider, after.Provider)
	diff = appendDiff(diff, "base_url", before.BaseURL, after.BaseURL)
	diff = appendDiff(diff, "wire_api", before.WireAPI, after.WireAPI)
	return diff
}

func appendDiff(items []DiffEntry, field, before, after string) []DiffEntry {
	if before == after {
		return items
	}
	return append(items, DiffEntry{
		Field:  field,
		Before: before,
		After:  after,
	})
}

func normalizeTarget(opts SwitchOptions) (string, error) {
	target := strings.ToLower(strings.TrimSpace(opts.TargetHost))
	if target == "" {
		target = "all"
	}
	if opts.CodexProvider == "" {
		opts.CodexProvider = defaultCodexProvider
	}
	switch target {
	case "all", "claude", "codex":
	default:
		return "", fmt.Errorf("unsupported host target: %s", opts.TargetHost)
	}
	if target == "all" || target == "claude" {
		if strings.TrimSpace(opts.ClaudeBaseURL) == "" {
			return "", fmt.Errorf("missing --claude-base-url for host target %s", target)
		}
	}
	if target == "all" || target == "codex" {
		if strings.TrimSpace(opts.CodexBaseURL) == "" {
			return "", fmt.Errorf("missing --codex-base-url for host target %s", target)
		}
	}
	return target, nil
}
