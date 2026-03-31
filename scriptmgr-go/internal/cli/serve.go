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
	"syscall"

	"scriptmgr/internal/api"
	httpsrv "scriptmgr/internal/http"
	"scriptmgr/internal/runtime"
)

// runServe starts the HTTP API server
func runServe(args []string, svc *api.API, stateDir string) error {
	addr := ":8765" // Default port (less common than 8080)

	// Parse arguments
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--port", "-p":
			if i+1 >= len(args) {
				return errors.New("missing value for --port")
			}
			i++
			port := args[i]
			// Validate port format
			if port[0] != ':' {
				port = ":" + port
			}
			addr = port
		case "--help", "-h":
			printServeUsage()
			return nil
		default:
			// Assume it's an address without flag
			if args[i][0] == ':' || (len(args[i]) > 1 && args[i][0] >= '0' && args[i][0] <= '9') {
				addr = args[i]
				if addr[0] != ':' {
					addr = ":" + addr
				}
			} else {
				return fmt.Errorf("unknown option: %s", args[i])
			}
		}
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

	srv := httpsrv.NewServer(svc)

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
	fmt.Println("  --port, -p <port>  Port to listen on (default: 8765)")
	fmt.Println("  --help, -h           Show this help message")
	fmt.Println()
	fmt.Println("Examples:")
	fmt.Println("  scriptmgr serve                    # Start on default port 8765")
	fmt.Println("  scriptmgr serve :3000              # Start on port 3000")
	fmt.Println("  scriptmgr serve --port 9000       # Start on port 9000")
	fmt.Println("  scriptmgr serve 127.0.0.1:8765  # Bind to specific IP")
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
