package cli

import (
	"errors"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strconv"
	"syscall"

	"scriptmgr/internal/hostconfig"
	"scriptmgr/internal/relay"
)

func runRelayCommand(args []string, stateDir string) error {
	if len(args) == 0 {
		printRelayUsage()
		return nil
	}

	switch args[0] {
	case "serve":
		return runRelayServe(args[1:], stateDir)
	case "config":
		return runRelayConfig(args[1:], stateDir)
	case "hosts":
		return runRelayHosts(args[1:])
	case "--help", "-h", "help":
		printRelayUsage()
		return nil
	default:
		return fmt.Errorf("unknown relay subcommand: %s", args[0])
	}
}

func runRelayServe(args []string, stateDir string) error {
	port := relay.DefaultPort

	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--port", "-p":
			if i+1 >= len(args) {
				return errors.New("missing value for --port")
			}
			i++
			parsed, err := strconv.Atoi(args[i])
			if err != nil {
				return fmt.Errorf("invalid relay port: %s", args[i])
			}
			port = parsed
		case "--help", "-h":
			printRelayUsage()
			return nil
		default:
			return fmt.Errorf("unknown relay option: %s", args[i])
		}
	}

	server, err := relay.NewServer(stateDir)
	if err != nil {
		return err
	}

	addr := fmt.Sprintf(":%d", port)
	fmt.Fprintf(os.Stderr, "Relay server listening on %s\n", addr)
	fmt.Fprintln(os.Stderr, "Endpoints:")
	fmt.Fprintln(os.Stderr, "  GET  /health")
	fmt.Fprintln(os.Stderr, "  GET  /api/relay/config")
	fmt.Fprintln(os.Stderr, "  PUT  /api/relay/config")
	fmt.Fprintln(os.Stderr, "  GET  /api/extensions")
	fmt.Fprintln(os.Stderr, "  POST /v1/chat/completions")
	fmt.Fprintln(os.Stderr, "  POST /v1/responses")
	fmt.Fprintln(os.Stderr, "  GET  /v1/models")
	fmt.Fprintln(os.Stderr)
	fmt.Fprintln(os.Stderr, "Press Ctrl+C to stop")

	stop := make(chan os.Signal, 1)
	signal.Notify(stop, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-stop
		os.Exit(0)
	}()

	return http.ListenAndServe(addr, server)
}

func runRelayConfig(args []string, stateDir string) error {
	asJSON := contains(args, "--json")

	service, err := relay.NewService(stateDir)
	if err != nil {
		return err
	}

	snapshot, err := service.Snapshot()
	if err != nil {
		return err
	}

	if asJSON {
		return writeJSON(snapshot)
	}

	fmt.Printf("Relay config file: %s\n", service.LoadConfigPath())
	fmt.Printf("Providers: %d\n", len(snapshot.Config.Providers))
	fmt.Printf("Routes:    %d\n", len(snapshot.Config.Routes))
	fmt.Printf("Extensions: %d\n", len(snapshot.Extensions))
	for _, item := range snapshot.Providers {
		fmt.Printf("  %-18s healthy=%t enabled=%t base=%s\n",
			item.Provider.ID,
			item.Status.Healthy,
			item.Provider.Enabled,
			item.Provider.BaseURL,
		)
	}
	return nil
}

func printRelayUsage() {
	fmt.Println("Usage: scriptmgr relay <subcommand> [options]")
	fmt.Println()
	fmt.Println("Subcommands:")
	fmt.Println("  serve [--port 8787]                 Start OpenAI-compatible relay server")
	fmt.Println("  config [--json]                     Print current relay config and provider health")
	fmt.Println("  hosts status [--json]               Inspect Claude/Codex host relay settings")
	fmt.Println("  hosts switch [options]              Update Claude/Codex host configs to target relay upstreams")
}

func runRelayHosts(args []string) error {
	if len(args) == 0 {
		printRelayHostsUsage()
		return nil
	}

	homeDir, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	switch args[0] {
	case "status":
		asJSON := contains(args[1:], "--json")
		status, err := hostconfig.Detect(homeDir)
		if err != nil {
			return err
		}
		if asJSON {
			return writeJSON(status)
		}
		printRelayHostStatus(status)
		return nil
	case "switch":
		asJSON := false
		dryRun := false
		opts := hostconfig.SwitchOptions{
			HomeDir:       homeDir,
			TargetHost:    "all",
			CodexProvider: "relay",
		}
		for i := 1; i < len(args); i++ {
			switch args[i] {
			case "--json":
				asJSON = true
			case "--dry-run":
				dryRun = true
			case "--host":
				i++
				if i >= len(args) {
					return errors.New("missing value for --host")
				}
				opts.TargetHost = args[i]
			case "--claude-base-url":
				i++
				if i >= len(args) {
					return errors.New("missing value for --claude-base-url")
				}
				opts.ClaudeBaseURL = args[i]
			case "--codex-base-url":
				i++
				if i >= len(args) {
					return errors.New("missing value for --codex-base-url")
				}
				opts.CodexBaseURL = args[i]
			case "--codex-provider":
				i++
				if i >= len(args) {
					return errors.New("missing value for --codex-provider")
				}
				opts.CodexProvider = args[i]
			default:
				return fmt.Errorf("unknown relay hosts switch option: %s", args[i])
			}
		}

		if dryRun {
			preview, err := hostconfig.PreviewSwitch(opts)
			if err != nil {
				return err
			}
			if asJSON {
				return writeJSON(preview)
			}
			printRelayHostPreview(preview)
			return nil
		}

		status, err := hostconfig.Switch(opts)
		if err != nil {
			return err
		}
		if asJSON {
			return writeJSON(status)
		}
		printRelayHostStatus(status)
		return nil
	case "--help", "-h", "help":
		printRelayHostsUsage()
		return nil
	default:
		return fmt.Errorf("unknown relay hosts subcommand: %s", args[0])
	}
}

func printRelayHostsUsage() {
	fmt.Println("Usage: scriptmgr relay hosts <status|switch> [options]")
	fmt.Println()
	fmt.Println("Examples:")
	fmt.Println("  scriptmgr relay hosts status --json")
	fmt.Println("  scriptmgr relay hosts switch --dry-run --json --host codex --codex-base-url http://127.0.0.1:8787/v1")
	fmt.Println("  scriptmgr relay hosts switch --host claude --claude-base-url https://gateway.example.com/anthropic")
	fmt.Println("  scriptmgr relay hosts switch --host codex --codex-base-url http://127.0.0.1:8787/v1 --codex-provider relay")
	fmt.Println("  scriptmgr relay hosts switch --claude-base-url https://gateway.example.com/anthropic --codex-base-url http://127.0.0.1:8787/v1")
}

func printRelayHostStatus(status hostconfig.HostStatus) {
	fmt.Printf("Claude config: %s\n", status.Claude.Path)
	if !status.Claude.Exists {
		fmt.Println("  status: missing")
	} else {
		fmt.Printf("  base_url: %s\n", valueOrDash(status.Claude.BaseURL))
		fmt.Printf("  model: %s\n", valueOrDash(status.Claude.Model))
		fmt.Printf("  auth_token: %t\n", status.Claude.AuthToken)
		if status.Claude.Backup != "" {
			fmt.Printf("  backup: %s\n", filepath.Base(status.Claude.Backup))
		}
	}

	fmt.Printf("Codex config:  %s\n", status.Codex.Path)
	if !status.Codex.Exists {
		fmt.Println("  status: missing")
		return
	}
	fmt.Printf("  provider: %s\n", valueOrDash(status.Codex.Provider))
	fmt.Printf("  base_url: %s\n", valueOrDash(status.Codex.BaseURL))
	fmt.Printf("  model: %s\n", valueOrDash(status.Codex.ActiveModel))
	fmt.Printf("  wire_api: %s\n", valueOrDash(status.Codex.WireAPI))
	fmt.Printf("  requires_openai_auth: %t\n", status.Codex.RequiresAuth)
	if status.Codex.Backup != "" {
		fmt.Printf("  backup: %s\n", filepath.Base(status.Codex.Backup))
	}
}

func printRelayHostPreview(preview hostconfig.SwitchPreview) {
	fmt.Printf("Target host: %s\n", preview.TargetHost)
	if preview.Claude != nil {
		printRelayFilePreview("Claude", *preview.Claude)
	}
	if preview.Codex != nil {
		printRelayFilePreview("Codex", *preview.Codex)
	}
}

func printRelayFilePreview(label string, preview hostconfig.FilePreview) {
	fmt.Printf("%s preview: %s\n", label, preview.Path)
	fmt.Printf("  exists: %t\n", preview.Exists)
	fmt.Printf("  changed: %t\n", preview.Changed)
	if !preview.Changed {
		fmt.Println("  diff: no changes")
		return
	}
	if len(preview.Diff) > 0 {
		fmt.Println("  diff:")
		for _, item := range preview.Diff {
			fmt.Printf("    %s: %s -> %s\n", item.Field, valueOrPlaceholder(item.Before), valueOrPlaceholder(item.After))
		}
		return
	}
	fmt.Println("  before:")
	printIndentedBlock(preview.Before)
	fmt.Println("  after:")
	printIndentedBlock(preview.After)
}

func printIndentedBlock(value string) {
	if value == "" {
		fmt.Println("    <empty>")
		return
	}
	lines := splitLinesPreserveEmpty(value)
	for _, line := range lines {
		fmt.Printf("    %s\n", line)
	}
}

func splitLinesPreserveEmpty(value string) []string {
	if value == "" {
		return nil
	}
	lines := []string{}
	current := ""
	for _, ch := range value {
		if ch == '\n' {
			lines = append(lines, current)
			current = ""
			continue
		}
		current += string(ch)
	}
	if current != "" {
		lines = append(lines, current)
	}
	return lines
}

func valueOrPlaceholder(value string) string {
	if value == "" {
		return "<empty>"
	}
	return value
}
