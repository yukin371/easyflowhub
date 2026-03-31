package transport

import (
	"bufio"
	"encoding/json"
	"io"
	"os"
	"sync"
)

// Transport implements stdio transport for MCP
type Transport struct {
	decoder *json.Decoder
	encoder *json.Encoder
	writer  *bufio.Writer
	mu      sync.Mutex
}

// NewTransport creates a new stdio transport
func NewTransport() *Transport {
	decoder := json.NewDecoder(os.Stdin)
	writer := bufio.NewWriter(os.Stdout)
	encoder := json.NewEncoder(os.Stdout)
	return &Transport{decoder: decoder, writer: writer, encoder: encoder}
}

// NewTransportWithIO creates a transport with custom IO for testing
func NewTransportWithIO(r io.Reader, w io.Writer) *Transport {
	decoder := json.NewDecoder(r)
	writer := bufio.NewWriter(w)
	encoder := json.NewEncoder(w)
	return &Transport{decoder: decoder, writer: writer, encoder: encoder}
}

// ReadMessage reads a single JSON-RPC message from stdin
func (t *Transport) ReadMessage() ([]byte, error) {
	var msg json.RawMessage
	if err := t.decoder.Decode(&msg); err != nil {
		return nil, err
	}
	return []byte(msg), nil
}

// WriteMessage writes a JSON-RPC message to stdout
func (t *Transport) WriteMessage(msg []byte) error {
	t.mu.Lock()
	defer t.mu.Unlock()

	_, err := t.writer.Write(msg)
	if err != nil {
		return err
	}
	_, err = t.writer.WriteString("\n")
	if err != nil {
		return err
	}
	return t.writer.Flush()
}

// WriteResponse writes a response to stdout
func (t *Transport) WriteResponse(resp any) error {
	t.mu.Lock()
	defer t.mu.Unlock()
	return t.encoder.Encode(resp)
}

// Close closes the transport
func (t *Transport) Close() error {
	return nil
}
