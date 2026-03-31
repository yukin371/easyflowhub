package mcpcli

// ServerConfig represents configuration for a single MCP server
type ServerConfig struct {
	Command string            `json:"command"`
	Args    []string          `json:"args,omitempty"`
	Env     map[string]string `json:"env,omitempty"`
}

// Config represents the MCP CLI configuration file
type Config struct {
	Version string                 `json:"version"`
	Servers map[string]ServerConfig `json:"servers"`
}

// Tool represents an MCP tool definition
type Tool struct {
	Name        string         `json:"name"`
	Description string         `json:"description"`
	InputSchema map[string]any `json:"inputSchema,omitempty"`
}

// Request represents a JSON-RPC 2.0 request
type Request struct {
	JSONRPC string         `json:"jsonrpc"`
	ID      int            `json:"id"`
	Method  string         `json:"method"`
	Params  map[string]any `json:"params,omitempty"`
}

// Response represents a JSON-RPC 2.0 response
type Response struct {
	JSONRPC string         `json:"jsonrpc"`
	ID      int            `json:"id"`
	Result  any            `json:"result,omitempty"`
	Error   *ResponseError `json:"error,omitempty"`
}

// ResponseError represents a JSON-RPC error
type ResponseError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}
