package http

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

func TestNewHub(t *testing.T) {
	hub := NewHub()
	if hub == nil {
		t.Fatal("NewHub() returned nil")
	}
	if hub.clients == nil {
		t.Error("Hub.clients should be initialized, got nil")
	}
	if hub.broadcast == nil {
		t.Error("Hub.broadcast channel should be initialized, got nil")
	}
	if hub.register == nil {
		t.Error("Hub.register channel should be initialized, got nil")
	}
	if hub.unregister == nil {
		t.Error("Hub.unregister channel should be initialized, got nil")
	}
}

func TestHubRegister(t *testing.T) {
	hub := NewHub()
	client := &Client{
		hub:  hub,
		send: make(chan TaskStatusEvent, 1),
	}

	hub.Register(client)

	if len(hub.clients) != 1 {
		t.Errorf("Expected 1 client, got %d", len(hub.clients))
	}
	if !hub.clients[client] {
		t.Error("Client should be registered in hub")
	}
}

func TestHubUnregister(t *testing.T) {
	hub := NewHub()
	client := &Client{
		hub:  hub,
		send: make(chan TaskStatusEvent, 1),
	}

	hub.Register(client)
	hub.Unregister(client)

	if len(hub.clients) != 0 {
		t.Errorf("Expected 0 clients after unregister, got %d", len(hub.clients))
	}

	// Verify channel is closed
	select {
	case _, ok := <-client.send:
		if ok {
			t.Error("Client send channel should be closed")
		}
	default:
		// Channel might not have been drained yet, check if closed
	}
}

func TestHubBroadcast(t *testing.T) {
	hub := NewHub()

	// Create test clients with buffered channels
	client1 := &Client{
		hub:  hub,
		send: make(chan TaskStatusEvent, 10),
	}
	client2 := &Client{
		hub:  hub,
		send: make(chan TaskStatusEvent, 10),
	}

	hub.Register(client1)
	hub.Register(client2)

	event := TaskStatusEvent{
		TaskID:    "task-123",
		ScriptID:  "script-456",
		Status:    "running",
		Timestamp: time.Now().UTC().Format(time.RFC3339),
	}

	hub.Broadcast(event)

	// Both clients should receive the event
	select {
	case received := <-client1.send:
		if received.TaskID != event.TaskID {
			t.Errorf("Client1: expected TaskID %s, got %s", event.TaskID, received.TaskID)
		}
	case <-time.After(time.Second):
		t.Error("Client1 did not receive broadcast")
	}

	select {
	case received := <-client2.send:
		if received.TaskID != event.TaskID {
			t.Errorf("Client2: expected TaskID %s, got %s", event.TaskID, received.TaskID)
		}
	case <-time.After(time.Second):
		t.Error("Client2 did not receive broadcast")
	}
}

func TestTaskStatusEventJSON(t *testing.T) {
	exitCode := 0
	event := TaskStatusEvent{
		TaskID:     "task-123",
		ScriptID:   "script-456",
		Status:     "success",
		ExitCode:   &exitCode,
		DurationMs: 1500,
		Timestamp:  "2026-03-16T00:00:00Z",
	}

	data, err := json.Marshal(event)
	if err != nil {
		t.Fatalf("Failed to marshal TaskStatusEvent: %v", err)
	}

	var parsed TaskStatusEvent
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("Failed to unmarshal TaskStatusEvent: %v", err)
	}

	if parsed.TaskID != event.TaskID {
		t.Errorf("TaskID: expected %s, got %s", event.TaskID, parsed.TaskID)
	}
	if parsed.ScriptID != event.ScriptID {
		t.Errorf("ScriptID: expected %s, got %s", event.ScriptID, parsed.ScriptID)
	}
	if parsed.Status != event.Status {
		t.Errorf("Status: expected %s, got %s", event.Status, parsed.Status)
	}
	if parsed.DurationMs != event.DurationMs {
		t.Errorf("DurationMs: expected %d, got %d", event.DurationMs, parsed.DurationMs)
	}
	if parsed.Timestamp != event.Timestamp {
		t.Errorf("Timestamp: expected %s, got %s", event.Timestamp, parsed.Timestamp)
	}
	if parsed.ExitCode == nil || *parsed.ExitCode != exitCode {
		t.Errorf("ExitCode: expected %d, got %v", exitCode, parsed.ExitCode)
	}
}

func TestTaskStatusEventJSONOmitEmpty(t *testing.T) {
	// Test without optional fields
	event := TaskStatusEvent{
		TaskID:    "task-789",
		ScriptID:  "script-000",
		Status:    "running",
		Timestamp: "2026-03-16T00:00:00Z",
	}

	data, err := json.Marshal(event)
	if err != nil {
		t.Fatalf("Failed to marshal TaskStatusEvent: %v", err)
	}

	// Verify exit_code and duration_ms are omitted
	var raw map[string]any
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatalf("Failed to unmarshal to map: %v", err)
	}

	if _, exists := raw["exit_code"]; exists {
		t.Error("exit_code should be omitted when nil")
	}
	if _, exists := raw["duration_ms"]; exists {
		t.Error("duration_ms should be omitted when zero")
	}
}

// WebSocket integration tests

var testUpgrader = websocket.Upgrader{
	ReadBufferSize:  1024,
	WriteBufferSize: 1024,
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

func TestWebSocketUpgrade(t *testing.T) {
	hub := NewHub()
	go hub.Run()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ServeWS(hub, testUpgrader)(w, r)
	}))
	defer server.Close()

	// Replace http:// with ws://
	wsURL := "ws" + strings.TrimPrefix(server.URL, "http")

	// Connect as WebSocket client
	ws, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("Failed to upgrade to WebSocket: %v", err)
	}
	defer ws.Close()

	// Verify connection is established
	if ws == nil {
		t.Error("WebSocket connection should be established")
	}
}

func TestWebSocketClientReceivesBroadcast(t *testing.T) {
	hub := NewHub()
	go hub.Run()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ServeWS(hub, testUpgrader)(w, r)
	}))
	defer server.Close()

	wsURL := "ws" + strings.TrimPrefix(server.URL, "http")

	ws, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("Failed to connect: %v", err)
	}
	defer ws.Close()

	// Allow time for registration
	time.Sleep(50 * time.Millisecond)

	// Broadcast an event
	event := TaskStatusEvent{
		TaskID:    "task-test-123",
		ScriptID:  "script-test",
		Status:    "running",
		Timestamp: time.Now().UTC().Format(time.RFC3339),
	}
	hub.Broadcast(event)

	// Read message from WebSocket
	var received TaskStatusEvent
	err = ws.ReadJSON(&received)
	if err != nil {
		t.Fatalf("Failed to read from WebSocket: %v", err)
	}

	if received.TaskID != event.TaskID {
		t.Errorf("Expected TaskID %s, got %s", event.TaskID, received.TaskID)
	}
	if received.Status != event.Status {
		t.Errorf("Expected Status %s, got %s", event.Status, received.Status)
	}
}

func TestWebSocketMultipleClients(t *testing.T) {
	hub := NewHub()
	go hub.Run()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ServeWS(hub, testUpgrader)(w, r)
	}))
	defer server.Close()

	wsURL := "ws" + strings.TrimPrefix(server.URL, "http")

	// Connect two clients
	ws1, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("Failed to connect client1: %v", err)
	}
	defer ws1.Close()

	ws2, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("Failed to connect client2: %v", err)
	}
	defer ws2.Close()

	// Allow time for registration
	time.Sleep(50 * time.Millisecond)

	// Broadcast an event
	event := TaskStatusEvent{
		TaskID:    "task-multi",
		ScriptID:  "script-multi",
		Status:    "success",
		Timestamp: time.Now().UTC().Format(time.RFC3339),
	}
	hub.Broadcast(event)

	// Both clients should receive the same message
	var received1, received2 TaskStatusEvent

	err = ws1.ReadJSON(&received1)
	if err != nil {
		t.Fatalf("Client1 failed to read: %v", err)
	}

	err = ws2.ReadJSON(&received2)
	if err != nil {
		t.Fatalf("Client2 failed to read: %v", err)
	}

	if received1.TaskID != event.TaskID {
		t.Errorf("Client1: expected TaskID %s, got %s", event.TaskID, received1.TaskID)
	}
	if received2.TaskID != event.TaskID {
		t.Errorf("Client2: expected TaskID %s, got %s", event.TaskID, received2.TaskID)
	}
}

func TestWebSocketClientDisconnect(t *testing.T) {
	hub := NewHub()
	go hub.Run()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		ServeWS(hub, testUpgrader)(w, r)
	}))
	defer server.Close()

	wsURL := "ws" + strings.TrimPrefix(server.URL, "http")

	// Connect and immediately close
	ws, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("Failed to connect: %v", err)
	}

	// Allow registration
	time.Sleep(50 * time.Millisecond)

	initialClients := len(hub.clients)

	// Close connection
	ws.Close()

	// Allow cleanup
	time.Sleep(100 * time.Millisecond)

	// Verify client was removed
	finalClients := len(hub.clients)
	if finalClients >= initialClients {
		t.Errorf("Expected fewer clients after disconnect, initial=%d, final=%d", initialClients, finalClients)
	}
}
