package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/signal"
	"sort"
	"strings"
	"syscall"

	"scriptmgr/internal/api"
	"scriptmgr/internal/mcp"
	"scriptmgr/internal/mcp/transport"
	"scriptmgr/internal/mcpcli"
	"scriptmgr/internal/notes"
)

// runMCPCommand handles the mcp command with multiple subcommands
func runMCPCommand(args []string, svc *api.API) error {
	// No args - list available servers
	if len(args) == 0 {
		return listMCPServers()
	}

	// First arg is server name
	serverName := args[0]

	// Load config to check if server exists
	config, err := mcpcli.LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	serverConfig, exists := config.Servers[serverName]
	if !exists {
		// Check if it's a reserved command
		if serverName == "serve" || serverName == "server" {
			// Start scriptmgr's own MCP server
			return runMCPServer(args[1:], svc)
		}
		return fmt.Errorf("unknown MCP server: %s", serverName)
	}

	// Only server name - list tools for that server
	if len(args) == 1 {
		return listMCPTools(serverName, serverConfig)
	}

	// Server and tool name - call the tool
	toolName := args[1]
	toolArgs := parseMCPArgs(args[2:])
	return callMCPTool(serverName, serverConfig, toolName, toolArgs)
}

// runMCPServer starts scriptmgr's own MCP server
func runMCPServer(args []string, svc *api.API) error {
	return runMCP(args, svc)
}

// listMCPServers lists all configured MCP servers
func listMCPServers() error {
	config, err := mcpcli.LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	if len(config.Servers) == 0 {
		fmt.Println("No MCP servers configured.")
		fmt.Println()
		fmt.Println("To add a server:")
		fmt.Println("  scriptmgr mcp-add <name> <command>")
		fmt.Println()
		fmt.Println("To import from Claude config:")
		fmt.Println("  scriptmgr mcp-import-claude")
		return nil
	}

	// Sort server names
	names := make([]string, 0, len(config.Servers))
	for name := range config.Servers {
		names = append(names, name)
	}
	sort.Strings(names)

	fmt.Printf("Configured MCP servers (%d):\n", len(names))
	for _, name := range names {
		cfg := config.Servers[name]
		fmt.Printf("  %-15s %s %s\n", name, cfg.Command, strings.Join(cfg.Args, " "))
	}
	fmt.Println()
	fmt.Println("Usage:")
	fmt.Println("  scriptmgr mcp <server>              - List tools")
	fmt.Println("  scriptmgr mcp <server> <tool> [args] - Call tool")

	return nil
}

// listMCPTools lists all tools available from an MCP server
func listMCPTools(serverName string, config mcpcli.ServerConfig) error {
	client, err := mcpcli.NewClient(config)
	if err != nil {
		return fmt.Errorf("failed to connect to %s: %w", serverName, err)
	}
	defer client.Close()

	tools, err := client.ListTools()
	if err != nil {
		return fmt.Errorf("failed to list tools: %w", err)
	}

	if len(tools) == 0 {
		fmt.Printf("No tools available from %s\n", serverName)
		return nil
	}

	fmt.Printf("Tools from %s (%d):\n", serverName, len(tools))
	for _, tool := range tools {
		fmt.Printf("  %-30s %s\n", tool.Name, truncate(tool.Description, 50))
	}

	return nil
}

// callMCPTool calls a tool on an MCP server
func callMCPTool(serverName string, config mcpcli.ServerConfig, toolName string, args map[string]any) error {
	client, err := mcpcli.NewClient(config)
	if err != nil {
		return fmt.Errorf("failed to connect to %s: %w", serverName, err)
	}
	defer client.Close()

	result, err := client.CallTool(toolName, args)
	if err != nil {
		return fmt.Errorf("tool call failed: %w", err)
	}

	// Output result as JSON
	output, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to serialize result: %w", err)
	}

	fmt.Println(string(output))
	return nil
}

// parseMCPArgs parses command line arguments into a map
// Supports: key=value, key=123, key=true, key=false, key='json'
func parseMCPArgs(args []string) map[string]any {
	result := make(map[string]any)

	for _, arg := range args {
		// Check for key=value format
		if idx := strings.Index(arg, "="); idx > 0 {
			key := arg[:idx]
			value := arg[idx+1:]

			// Try to parse value
			result[key] = parseValue(value)
		}
	}

	return result
}

// parseValue attempts to parse a string value into appropriate type
func parseValue(s string) any {
	// Check for boolean
	if s == "true" {
		return true
	}
	if s == "false" {
		return false
	}

	// Check for JSON object or array
	if (strings.HasPrefix(s, "{") && strings.HasSuffix(s, "}")) ||
		(strings.HasPrefix(s, "[") && strings.HasSuffix(s, "]")) {
		var v any
		if err := json.Unmarshal([]byte(s), &v); err == nil {
			return v
		}
	}

	// Check for number
	if strings.Contains(s, ".") {
		var f float64
		if _, err := fmt.Sscanf(s, "%f", &f); err == nil {
			return f
		}
	} else {
		var i int
		if _, err := fmt.Sscanf(s, "%d", &i); err == nil {
			return i
		}
	}

	// Default to string
	return s
}

// truncate truncates a string to maxLen
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}

// runMCPAdd adds a new MCP server configuration
func runMCPAdd(args []string) error {
	if len(args) < 2 {
		return fmt.Errorf("usage: scriptmgr mcp-add <name> <command> [args...]")
	}

	name := args[0]
	command := args[1]
	extraArgs := args[2:]

	config, err := mcpcli.LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	mcpcli.AddServer(config, name, command, extraArgs)

	if err := mcpcli.SaveConfig(config); err != nil {
		return fmt.Errorf("failed to save config: %w", err)
	}

	fmt.Printf("Added MCP server: %s\n", name)
	fmt.Printf("  Command: %s %s\n", command, strings.Join(extraArgs, " "))
	return nil
}

// runMCPRemove removes an MCP server configuration
func runMCPRemove(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("usage: scriptmgr mcp-remove <name>")
	}

	name := args[0]

	config, err := mcpcli.LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	if !mcpcli.RemoveServer(config, name) {
		return fmt.Errorf("server not found: %s", name)
	}

	if err := mcpcli.SaveConfig(config); err != nil {
		return fmt.Errorf("failed to save config: %w", err)
	}

	fmt.Printf("Removed MCP server: %s\n", name)
	return nil
}

// runMCPImportClaude imports MCP servers from Claude config files
func runMCPImportClaude() error {
	config, err := mcpcli.LoadConfig()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	imported, err := mcpcli.ImportFromClaude(config)
	if err != nil {
		return fmt.Errorf("failed to import: %w", err)
	}

	if imported == 0 {
		fmt.Println("No new servers to import from Claude config.")
		return nil
	}

	if err := mcpcli.SaveConfig(config); err != nil {
		return fmt.Errorf("failed to save config: %w", err)
	}

	fmt.Printf("Imported %d MCP server(s) from Claude config.\n", imported)
	return nil
}

// runMCP starts the MCP server using stdio transport
func runMCP(args []string, svc *api.API) error {
	slog.Info("starting MCP server")

	// Create stdio transport
	t := transport.NewTransport()

	// Create MCP server with API
	srv := mcp.New().WithAPI(svc)

	// Initialize Notes API
	appDataDir := notes.GetAppDataDir()
	notesAPI, err := notes.NewAPI(appDataDir)
	if err != nil {
		slog.Warn("failed to initialize notes API", "error", err)
	} else {
		srv = srv.WithNotesAPI(notesAPI)

		// Start file watcher for auto-sync
		watcher, err := notes.NewWatcher(notesAPI)
		if err != nil {
			slog.Warn("failed to create notes watcher", "error", err)
		} else {
			if err := watcher.Start(); err != nil {
				slog.Warn("failed to start notes watcher", "error", err)
			} else {
				slog.Info("notes file watcher started")
				defer watcher.Stop()
			}
		}
	}

	// Set transport on server for notifications
	srv.SetTransport(t)

	// Wire server as broadcaster for async task notifications
	svc.SetBroadcaster(srv)

	// Handle graceful shutdown
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-sigChan
		slog.Info("MCP server shutting down")
		os.Exit(0)
	}()

	// Run the server
	for {
		reqBytes, err := t.ReadMessage()
		if err != nil {
			if err == io.EOF {
				slog.Info("MCP server stopped", "reason", "EOF")
				return nil
			}
			return fmt.Errorf("failed to read message: %w", err)
		}

		// Parse request
		var req mcp.Request
		if err := json.Unmarshal(reqBytes, &req); err != nil {
			return fmt.Errorf("failed to parse request: %w", err)
		}

		// Handle request
		resp := srv.Handle(req)

		// Serialize response
		respBytes, err := json.Marshal(resp)
		if err != nil {
			return fmt.Errorf("failed to serialize response: %w", err)
		}

		// Write response
		if err := t.WriteMessage(respBytes); err != nil {
			return fmt.Errorf("failed to write message: %w", err)
		}
	}
}
