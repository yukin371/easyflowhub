package http

import (
	"log"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

// TaskStatusEvent represents a task status change broadcast to WebSocket clients
type TaskStatusEvent struct {
	TaskID     string `json:"task_id"`
	ScriptID   string `json:"script_id"`
	Status     string `json:"status"`
	ExitCode   *int   `json:"exit_code,omitempty"`
	DurationMs int64  `json:"duration_ms,omitempty"`
	Timestamp  string `json:"timestamp"`
}

// Hub maintains the set of active WebSocket clients and broadcasts messages to them
type Hub struct {
	clients    map[*Client]bool
	broadcast  chan TaskStatusEvent
	register   chan *Client
	unregister chan *Client
	mu         sync.RWMutex
}

// Client represents a single WebSocket connection
type Client struct {
	hub  *Hub
	conn *websocket.Conn
	send chan TaskStatusEvent
}

// NewHub creates a new Hub instance
func NewHub() *Hub {
	return &Hub{
		clients:    make(map[*Client]bool),
		broadcast:  make(chan TaskStatusEvent, 256),
		register:   make(chan *Client),
		unregister: make(chan *Client),
	}
}

// Register adds a client to the hub
func (h *Hub) Register(client *Client) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.clients[client] = true
}

// Unregister removes a client from the hub and closes its send channel
func (h *Hub) Unregister(client *Client) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if _, ok := h.clients[client]; ok {
		delete(h.clients, client)
		close(client.send)
	}
}

// Broadcast sends an event to all connected clients
func (h *Hub) Broadcast(event TaskStatusEvent) {
	h.mu.RLock()
	defer h.mu.RUnlock()
	for client := range h.clients {
		select {
		case client.send <- event:
		default:
			// Client buffer full, skip (client will be cleaned up on disconnect)
		}
	}
}

// BroadcastTaskStatus implements StatusBroadcaster interface for executor integration
func (h *Hub) BroadcastTaskStatus(taskID, scriptID, status string, exitCode *int, durationMs int64) {
	event := TaskStatusEvent{
		TaskID:     taskID,
		ScriptID:   scriptID,
		Status:     status,
		ExitCode:   exitCode,
		DurationMs: durationMs,
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
	}
	h.Broadcast(event)
}

// Run starts the hub's main loop for handling client registration and broadcasting
func (h *Hub) Run() {
	for {
		select {
		case client := <-h.register:
			h.mu.Lock()
			h.clients[client] = true
			h.mu.Unlock()
		case client := <-h.unregister:
			h.mu.Lock()
			if _, ok := h.clients[client]; ok {
				delete(h.clients, client)
				close(client.send)
			}
			h.mu.Unlock()
		case event := <-h.broadcast:
			h.mu.RLock()
			for client := range h.clients {
				select {
				case client.send <- event:
				default:
					// Buffer full, skip
				}
			}
			h.mu.RUnlock()
		}
	}
}

// DefaultUpgrader is the default WebSocket upgrader
var DefaultUpgrader = websocket.Upgrader{
	ReadBufferSize:  1024,
	WriteBufferSize: 1024,
	CheckOrigin: func(r *http.Request) bool {
		return true // Allow all origins for Tauri integration
	},
}

// ServeWS handles WebSocket connection requests
func ServeWS(hub *Hub, upgrader websocket.Upgrader) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Printf("WebSocket upgrade error: %v", err)
			return
		}

		client := &Client{
			hub:  hub,
			conn: conn,
			send: make(chan TaskStatusEvent, 256),
		}

		hub.register <- client

		// Start write pump in goroutine
		go client.writePump()

		// Run read pump in current goroutine
		client.readPump()
	}
}

// writePump pumps messages from the hub to the WebSocket connection
func (c *Client) writePump() {
	ticker := time.NewTicker(54 * time.Second)
	defer func() {
		ticker.Stop()
		c.conn.Close()
	}()

	for {
		select {
		case event, ok := <-c.send:
			if !ok {
				// Hub closed the channel
				c.conn.WriteMessage(websocket.CloseMessage, []byte{})
				return
			}
			if err := c.conn.WriteJSON(event); err != nil {
				log.Printf("WebSocket write error: %v", err)
				return
			}
		case <-ticker.C:
			if err := c.conn.WriteMessage(websocket.PingMessage, nil); err != nil {
				return
			}
		}
	}
}

// readPump pumps messages from the WebSocket connection to the hub
func (c *Client) readPump() {
	defer func() {
		c.hub.unregister <- c
		c.conn.Close()
	}()

	c.conn.SetReadLimit(512)
	c.conn.SetReadDeadline(time.Now().Add(60 * time.Second))
	c.conn.SetPongHandler(func(string) error {
		c.conn.SetReadDeadline(time.Now().Add(60 * time.Second))
		return nil
	})

	for {
		_, _, err := c.conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				log.Printf("WebSocket read error: %v", err)
			}
			break
		}
	}
}
