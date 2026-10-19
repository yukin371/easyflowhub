package cli

import (
	"errors"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"

	"scriptmgr/internal/api"
	httpsrv "scriptmgr/internal/http"
	"scriptmgr/internal/localapi"
	"scriptmgr/internal/relay"
	"scriptmgr/internal/runtime"
)

// runServe starts the HTTP API server
func runServe(args []string, svc *api.API, stateDir string) error {
	addr, showHelp, err := parseServeAddress(args)
	if err != nil {
		return err
	}
	if showHelp {
		printServeUsage()
		return nil
	}

	// Create runtime manager for PID/Port/Lock file management
	rt := runtime.NewRuntimeManager(stateDir)

	// 1. Acquire startup lock (prevents race conditions during startup)
	if err := rt.AcquireLock(); err != nil {
		if errors.Is(err, runtime.ErrLockHeld) {
			return fmt.Errorf("another instance is starting, please wait")
		}
		return fmt.Errorf("failed to acquire startup lock: %w", err)
	}

	// 2. Check for existing instance
	if pid, running, _ := rt.CheckExistingPID(); running {
		rt.ReleaseLock()
		return fmt.Errorf("server already running (PID: %d)", pid)
	}

	// 3. Cleanup stale files from zombie process
	rt.CleanupStaleFiles()

	relayService, err := relay.NewService(stateDir)
	if err != nil {
		rt.ReleaseLock()
		return fmt.Errorf("failed to initialize relay service: %w", err)
	}

	srv := httpsrv.NewServerWithRelay(svc, relayService)

	slog.Info("HTTP API server starting", "addr", addr)
	fmt.Fprintln(os.Stderr, "Endpoints:")
	fmt.Fprintln(os.Stderr, "  GET  /api/scripts          - List scripts")
	fmt.Fprintln(os.Stderr, "  GET  /api/scripts/{id}     - Describe script")
	fmt.Fprintln(os.Stderr, "  POST /api/run              - Run script")
	fmt.Fprintln(os.Stderr, "  GET  /api/tasks            - List tasks")
	fmt.Fprintln(os.Stderr, "  GET  /api/tasks/{id}       - Get task")
	fmt.Fprintln(os.Stderr, "  GET  /api/tasks/{id}/log   - Read task log")
	fmt.Fprintln(os.Stderr, "  POST /api/cancel/{id}      - Cancel task/session")
	fmt.Fprintln(os.Stderr)
	fmt.Fprintln(os.Stderr, "Press Ctrl+C to stop")

	// Check if port is available
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		var opErr *net.OpError
		if errors.As(err, &opErr) {
			if opErr.Op == "listen" {
				return fmt.Errorf("port %s is already in use by another application. Use --port to specify a different port", addr)
			}
			return fmt.Errorf("failed to bind port %s: %w", addr, err)
		}
		return fmt.Errorf("failed to bind port %s: %w", addr, err)
	}
	ln.Close()

	// Extract port from addr and write PID/Port files
	port := extractPort(addr)
	if err := rt.WritePID(os.Getpid()); err != nil {
		rt.ReleaseLock()
		return fmt.Errorf("failed to write PID file: %w", err)
	}
	if err := rt.WritePort(port); err != nil {
		rt.ReleaseLock()
		return fmt.Errorf("failed to write port file: %w", err)
	}

	// 4. Release startup lock - startup is complete
	rt.ReleaseLock()

	// Handle graceful shutdown
	stop := make(chan os.Signal, 1)
	signal.Notify(stop, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-stop
		fmt.Fprintln(os.Stderr, "\nShutting down HTTP server...")
		rt.Cleanup()
		os.Exit(0)
	}()

	if err := http.ListenAndServe(addr, srv); err != nil {
		return fmt.Errorf("HTTP server error: %w", err)
	}

	return nil
}

func printServeUsage() {
	fmt.Println("Usage: scriptmgr serve [options] [addr]")
	fmt.Println()
	fmt.Println("Options:")
	fmt.Println("  --port, -p <port>  Port to listen on at 127.0.0.1 (default: 8765)")
	fmt.Println("  --help, -h           Show this help message")
	fmt.Println()
	fmt.Println("Examples:")
	fmt.Println("  scriptmgr serve                    # Start on 127.0.0.1:8765")
	fmt.Println("  scriptmgr serve --port 9000        # Start on 127.0.0.1:9000")
	fmt.Println("  scriptmgr serve 127.0.0.1:8765     # Bind to a specific IP")
	fmt.Println("  scriptmgr serve [::1]:8765         # Bind to IPv6 loopback")
}

func parseServeAddress(args []string) (string, bool, error) {
	addr := localapi.LoopbackAddress("8765")
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--port", "-p":
			if i+1 >= len(args) {
				return "", false, errors.New("missing value for --port")
			}
			i++
			port := strings.TrimPrefix(args[i], ":")
			if !validPort(port) {
				return "", false, fmt.Errorf("invalid server port: %s", args[i])
			}
			addr = localapi.LoopbackAddress(port)
		case "--help", "-h":
			return "", true, nil
		default:
			arg := args[i]
			if strings.HasPrefix(arg, ":") {
				port := strings.TrimPrefix(arg, ":")
				if !validPort(port) {
					return "", false, fmt.Errorf("invalid server address: %s", arg)
				}
				addr = net.JoinHostPort("", port)
				continue
			}
			if _, _, err := net.SplitHostPort(arg); err == nil {
				addr = arg
				continue
			}
			if !validPort(arg) {
				return "", false, fmt.Errorf("unknown option: %s", arg)
			}
			addr = localapi.LoopbackAddress(arg)
		}
	}
	if !localapi.IsLoopbackAddress(addr) {
		return "", false, fmt.Errorf("scriptmgr serve only supports loopback addresses; got %s", addr)
	}
	return addr, false, nil
}

func validPort(value string) bool {
	port, err := strconv.Atoi(value)
	return err == nil && port >= 1 && port <= 65535
}

// extractPort parses the port number from an address string
// Handles formats: ":8765", "127.0.0.1:8765", "8765"
func extractPort(addr string) int {
	_, portStr, err := net.SplitHostPort(addr)
	if err != nil {
		// Try parsing as just port number
		if p, e := strconv.Atoi(addr); e == nil {
			return p
		}
		return 8765 // default
	}
	p, _ := strconv.Atoi(portStr)
	return p
}
