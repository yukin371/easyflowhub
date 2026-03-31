package mcpcli

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

// ConfigPaths returns the paths to search for MCP configuration files
// in priority order
func ConfigPaths() []string {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return nil
	}

	var paths []string

	// 1. Environment variable override
	if envPath := os.Getenv("SCRIPTMGR_MCP_CONFIG"); envPath != "" {
		paths = append(paths, envPath)
	}

	// 2. scriptmgr专用配置
	paths = append(paths, filepath.Join(homeDir, ".scriptmgr", "mcp-config.json"))

	// 3. Claude Code MCP config (mcp_config.json)
	paths = append(paths, filepath.Join(homeDir, ".claude", "mcp_config.json"))

	// 4. Claude Desktop config (Windows)
	appData := os.Getenv("APPDATA")
	if appData != "" {
		paths = append(paths, filepath.Join(appData, "Claude", "claude_desktop_config.json"))
	}

	return paths
}

// LoadConfig loads MCP configuration from the first available config file
func LoadConfig() (*Config, error) {
	paths := ConfigPaths()
	for _, path := range paths {
		if _, err := os.Stat(path); err != nil {
			continue
		}

		config, err := loadConfigFile(path)
		if err != nil {
			return nil, fmt.Errorf("failed to load %s: %w", path, err)
		}
		return config, nil
	}

	// Return empty config if no file found
	return &Config{
		Version: "1.0",
		Servers: make(map[string]ServerConfig),
	}, nil
}

// loadConfigFile loads and parses a config file
func loadConfigFile(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	// Try scriptmgr format first (has "version" and "servers")
	var config Config
	if err := json.Unmarshal(data, &config); err == nil && config.Servers != nil {
		return &config, nil
	}

	// Try Claude mcp_config.json format (uses "servers" key)
	var mcpConfig struct {
		Servers map[string]ServerConfig `json:"servers"`
	}
	if err := json.Unmarshal(data, &mcpConfig); err == nil && mcpConfig.Servers != nil {
		return &Config{
			Version: "1.0",
			Servers: mcpConfig.Servers,
		}, nil
	}

	// Try Claude Desktop settings format (uses "mcpServers" key)
	var claudeConfig struct {
		McpServers map[string]ServerConfig `json:"mcpServers"`
	}
	if err := json.Unmarshal(data, &claudeConfig); err == nil && claudeConfig.McpServers != nil {
		return &Config{
			Version: "1.0",
			Servers: claudeConfig.McpServers,
		}, nil
	}

	return nil, fmt.Errorf("unrecognized config format")
}

// SaveConfig saves the configuration to the default path
func SaveConfig(config *Config) error {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	configDir := filepath.Join(homeDir, ".scriptmgr")
	if err := os.MkdirAll(configDir, 0755); err != nil {
		return err
	}

	configPath := filepath.Join(configDir, "mcp-config.json")
	data, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(configPath, data, 0644)
}

// AddServer adds a new server to the configuration
func AddServer(config *Config, name, command string, args []string) {
	// Parse command string
	parts := strings.Fields(command)
	if len(parts) > 0 {
		cmd := parts[0]
		cmdArgs := append(parts[1:], args...)

		// Resolve command path
		resolvedCmd := resolveCommand(cmd)

		config.Servers[name] = ServerConfig{
			Command: resolvedCmd,
			Args:    cmdArgs,
		}
	}
}

// RemoveServer removes a server from the configuration
func RemoveServer(config *Config, name string) bool {
	if _, exists := config.Servers[name]; exists {
		delete(config.Servers, name)
		return true
	}
	return false
}

// resolveCommand resolves a command to its full path if possible
func resolveCommand(cmd string) string {
	// If already an absolute path, return as is
	if filepath.IsAbs(cmd) {
		return cmd
	}

	// Try to find in PATH
	if path, err := exec.LookPath(cmd); err == nil {
		return path
	}

	// Return as is if not found
	return cmd
}

// GetConfigPath returns the path where config is/will be stored
func GetConfigPath() string {
	homeDir, _ := os.UserHomeDir()
	if homeDir == "" {
		return ""
	}
	return filepath.Join(homeDir, ".scriptmgr", "mcp-config.json")
}

// ImportFromClaude imports MCP servers from Claude config files
func ImportFromClaude(config *Config) (int, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return 0, err
	}

	claudePaths := []string{
		filepath.Join(homeDir, ".claude", "settings.json"),
		filepath.Join(homeDir, ".claude", "settings.local.json"),
	}

	imported := 0
	for _, path := range claudePaths {
		data, err := os.ReadFile(path)
		if err != nil {
			continue
		}

		var claudeConfig struct {
			McpServers map[string]ServerConfig `json:"mcpServers"`
		}
		if err := json.Unmarshal(data, &claudeConfig); err != nil {
			continue
		}

		for name, server := range claudeConfig.McpServers {
			if _, exists := config.Servers[name]; !exists {
				config.Servers[name] = server
				imported++
			}
		}
	}

	return imported, nil
}

// getDefaultShell returns the default shell for the current platform
func getDefaultShell() string {
	if runtime.GOOS == "windows" {
		return "cmd"
	}
	return "sh"
}
