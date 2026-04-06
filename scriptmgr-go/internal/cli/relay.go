package cli

import (
	"errors"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"syscall"

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
	fmt.Println("  serve [--port 8787]   Start OpenAI-compatible relay server")
	fmt.Println("  config [--json]       Print current relay config and provider health")
}
