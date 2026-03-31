package transport

import (
	"bytes"
	"encoding/json"
	"testing"
)

func TestTransport_ReadMessage(t *testing.T) {
	tests := []struct {
		name      string
		input     string
		wantError bool
	}{
		{
			name:      "basic",
			input:     `{"jsonrpc": "2.0", "method": "test", "id": 1}`,
			wantError: false,
		},
		{
			name:      "with_params",
			input:     `{"jsonrpc": "2.0", "method": "test", "id": 2, "params": {"key": "value"}}`,
			wantError: false,
		},
		{
			name:      "invalid_json",
			input:     `invalid json`,
			wantError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := bytes.NewReader([]byte(tt.input + "\n"))
			w := &bytes.Buffer{}
			transport := NewTransportWithIO(r, w)

			msg, err := transport.ReadMessage()
			if tt.wantError {
				if err == nil {
					t.Errorf("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			// Verify the message is valid JSON
			var parsed map[string]any
			if err := json.Unmarshal(msg, &parsed); err != nil {
				t.Errorf("failed to parse message: %v", err)
			}
		})
	}
}

func TestTransport_WriteMessage(t *testing.T) {
	tests := []struct {
		name      string
		message   string
		wantError bool
	}{
		{
			name:      "basic",
			message:   `{"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}`,
			wantError: false,
		},
		{
			name:      "with_error",
			message:   `{"jsonrpc": "2.0", "id": 2, "error": {"code": -32601, "message": "method not found"}}`,
			wantError: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := bytes.NewReader([]byte{})
			w := &bytes.Buffer{}
			transport := NewTransportWithIO(r, w)

			err := transport.WriteMessage([]byte(tt.message))
			if tt.wantError {
				if err == nil {
					t.Errorf("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			// Verify output ends with newline
			output := w.String()
			if output[len(output)-1] != '\n' {
				t.Errorf("expected output to end with newline, got: %q", output)
			}
		})
	}
}

func TestTransport_WriteResponse(t *testing.T) {
	tests := []struct {
		name      string
		response  any
		wantError bool
	}{
		{
			name: "basic",
			response: map[string]any{
				"jsonrpc": "2.0",
				"id":      1,
				"result":  map[string]any{"ok": true},
			},
			wantError: false,
		},
		{
			name: "with_error",
			response: map[string]any{
				"jsonrpc": "2.0",
				"id":      2,
				"error": map[string]any{
					"code":    -32601,
					"message": "method not found",
				},
			},
			wantError: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := bytes.NewReader([]byte{})
			w := &bytes.Buffer{}
			transport := NewTransportWithIO(r, w)

			err := transport.WriteResponse(tt.response)
			if tt.wantError {
				if err == nil {
					t.Errorf("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			// Verify output is valid JSON
			output := w.String()
			var parsed map[string]any
			if err := json.Unmarshal([]byte(output), &parsed); err != nil {
				t.Errorf("failed to parse output: %v", err)
			}
		})
	}
}
