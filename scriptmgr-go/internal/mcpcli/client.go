package mcpcli

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"time"
)

const (
	defaultTimeout = 30 * time.Second
)

// Client represents an MCP client that communicates with a server via stdio
type Client struct {
	cmd     *exec.Cmd
	stdin   io.Writer
	stdout  *bufio.Reader
	stderr  io.Reader
	nextID  int
	started bool
}

// NewClient creates and starts a new MCP client
func NewClient(server ServerConfig) (*Client, error) {
	// Build command
	cmd := exec.Command(server.Command, server.Args...)

	// Set environment
	if len(server.Env) > 0 {
		cmd.Env = os.Environ()
		for k, v := range server.Env {
			cmd.Env = append(cmd.Env, fmt.Sprintf("%s=%s", k, v))
		}
	}

	// Get pipes
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, fmt.Errorf("failed to create stdin pipe: %w", err)
	}

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("failed to create stdout pipe: %w", err)
	}

	stderr, err := cmd.StderrPipe()
	if err != nil {
		return nil, fmt.Errorf("failed to create stderr pipe: %w", err)
	}

	// Start process
	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("failed to start server: %w", err)
	}

	// Give the process a moment to initialize
	time.Sleep(500 * time.Millisecond)

	// Check if process is still running
	if cmd.ProcessState != nil && cmd.ProcessState.Exited() {
		return nil, fmt.Errorf("server exited immediately")
	}

	client := &Client{
		cmd:    cmd,
		stdin:  stdin,
		stdout: bufio.NewReader(stdout),
		stderr: stderr,
		nextID: 1,
	}

	// Initialize connection
	if err := client.initialize(); err != nil {
		client.Close()
		return nil, err
	}

	client.started = true
	return client, nil
}

// initialize sends the initialize request to the server
func (c *Client) initialize() error {
	params := map[string]any{
		"protocolVersion": "2024-11-05",
		"capabilities":    map[string]any{},
		"clientInfo": map[string]any{
			"name":    "scriptmgr",
			"version": "1.0.0",
		},
	}

	resp, err := c.Call("initialize", params)
	if err != nil {
		return fmt.Errorf("initialize failed: %w", err)
	}

	if resp.Error != nil {
		return fmt.Errorf("initialize error: %s", resp.Error.Message)
	}

	return nil
}

// Call sends a JSON-RPC request and waits for the response
func (c *Client) Call(method string, params map[string]any) (*Response, error) {
	req := Request{
		JSONRPC: "2.0",
		ID:      c.nextID,
		Method:  method,
		Params:  params,
	}
	c.nextID++

	// Send request
	reqBytes, err := json.Marshal(req)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal request: %w", err)
	}

	// Write as single line with newline
	if _, err := fmt.Fprintf(c.stdin, "%s\n", string(reqBytes)); err != nil {
		return nil, fmt.Errorf("failed to write request: %w", err)
	}

	// Read response, skipping non-JSON lines (like INFO logs)
	var respLine string
	for {
		line, err := c.stdout.ReadString('\n')
		if err != nil {
			return nil, fmt.Errorf("failed to read response: %w", err)
		}

		line = strings.TrimSpace(line)

		// Skip empty lines and log lines (INFO, DEBUG, WARNING, ERROR)
		if line == "" || isLogLine(line) {
			continue
		}

		// Check if line starts with { (JSON object)
		if strings.HasPrefix(line, "{") {
			respLine = line
			break
		}
	}

	// Parse response
	var resp Response
	if err := json.Unmarshal([]byte(respLine), &resp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w (line: %s)", err, respLine)
	}

	return &resp, nil
}

// isLogLine checks if a line is a log message that should be skipped
func isLogLine(line string) bool {
	// Skip common log prefixes
	prefixes := []string{"INFO", "DEBUG", "WARNING", "ERROR", "WARN", "TRACE"}
	for _, prefix := range prefixes {
		if strings.HasPrefix(line, prefix+" ") || strings.HasPrefix(line, prefix+"\t") {
			return true
		}
	}
	return false
}

// ListTools retrieves the list of available tools from the server
func (c *Client) ListTools() ([]Tool, error) {
	resp, err := c.Call("tools/list", nil)
	if err != nil {
		return nil, err
	}

	if resp.Error != nil {
		return nil, fmt.Errorf("tools/list error: %s", resp.Error.Message)
	}

	// Parse tools from result
	var tools struct {
		Tools []Tool `json:"tools"`
	}

	resultBytes, err := json.Marshal(resp.Result)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal result: %w", err)
	}

	if err := json.Unmarshal(resultBytes, &tools); err != nil {
		return nil, fmt.Errorf("failed to parse tools: %w", err)
	}

	return tools.Tools, nil
}

// CallTool invokes a tool on the server
func (c *Client) CallTool(name string, args map[string]any) (any, error) {
	params := map[string]any{
		"name":      name,
		"arguments": args,
	}

	resp, err := c.Call("tools/call", params)
	if err != nil {
		return nil, err
	}

	if resp.Error != nil {
		return nil, fmt.Errorf("tool error [%d]: %s", resp.Error.Code, resp.Error.Message)
	}

	return resp.Result, nil
}

// Close terminates the MCP server process
func (c *Client) Close() error {
	if c.cmd == nil || c.cmd.Process == nil {
		return nil
	}

	// Try graceful shutdown first
	if c.stdin != nil {
		// Send shutdown notification (optional)
		c.stdin.Write([]byte(`{"jsonrpc":"2.0","method":"shutdown"}` + "\n"))
	}

	// Give process time to exit
	done := make(chan error, 1)
	go func() {
		done <- c.cmd.Wait()
	}()

	select {
	case <-time.After(2 * time.Second):
		// Force kill if still running
		c.cmd.Process.Kill()
	case <-done:
		// Process exited cleanly
	}

	return nil
}

// ReadStderr reads any available stderr output
func (c *Client) ReadStderr() string {
	if c.stderr == nil {
		return ""
	}

	var buf strings.Builder
	bufBytes := make([]byte, 1024)
	for {
		n, err := c.stderr.Read(bufBytes)
		if n > 0 {
			buf.Write(bufBytes[:n])
		}
		if err != nil {
			break
		}
	}
	return buf.String()
}

// IsStarted returns whether the client has been started and initialized
func (c *Client) IsStarted() bool {
	return c.started
}
