package notes

import (
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
)

// Watcher monitors the notes directory for changes and auto-syncs
type Watcher struct {
	watcher    *fsnotify.Watcher
	api        *API
	debounce   map[string]*time.Timer
	debounceMu sync.Mutex
	stop       chan struct{}
	running    bool
	runningMu  sync.Mutex
}

// NewWatcher creates a new file watcher
func NewWatcher(api *API) (*Watcher, error) {
	fswatcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	return &Watcher{
		watcher:  fswatcher,
		api:      api,
		debounce: make(map[string]*time.Timer),
		stop:     make(chan struct{}),
	}, nil
}

// Start begins watching the notes directory
func (w *Watcher) Start() error {
	w.runningMu.Lock()
	defer w.runningMu.Unlock()

	if w.running {
		return nil
	}

	repoPath := w.api.GetRepoPath()

	// Ensure repo exists
	if err := w.api.EnsureRepo(); err != nil {
		return err
	}

	// Add watch
	if err := w.watcher.Add(repoPath); err != nil {
		return err
	}

	w.running = true
	go w.run()

	log.Printf("[NotesWatcher] Started watching: %s", repoPath)
	return nil
}

// Stop stops the watcher
func (w *Watcher) Stop() {
	w.runningMu.Lock()
	defer w.runningMu.Unlock()

	if !w.running {
		return
	}

	close(w.stop)
	w.watcher.Close()
	w.running = false

	// Clear pending timers
	w.debounceMu.Lock()
	for _, timer := range w.debounce {
		timer.Stop()
	}
	w.debounce = make(map[string]*time.Timer)
	w.debounceMu.Unlock()

	log.Println("[NotesWatcher] Stopped")
}

// run handles file system events
func (w *Watcher) run() {
	for {
		select {
		case <-w.stop:
			return
		case event, ok := <-w.watcher.Events:
			if !ok {
				return
			}
			w.handleEvent(event)
		case err, ok := <-w.watcher.Errors:
			if !ok {
				return
			}
			log.Printf("[NotesWatcher] Error: %v", err)
		}
	}
}

// handleEvent processes a file system event with debouncing
func (w *Watcher) handleEvent(event fsnotify.Event) {
	// Only process .md files
	if !strings.HasSuffix(event.Name, ".md") {
		return
	}

	// Get relative path for logging
	filename := filepath.Base(event.Name)

	// Debounce: wait 300ms before processing
	w.debounceMu.Lock()
	defer w.debounceMu.Unlock()

	// Cancel existing timer for this file
	if timer, exists := w.debounce[event.Name]; exists {
		timer.Stop()
	}

	// Create new timer
	w.debounce[event.Name] = time.AfterFunc(300*time.Millisecond, func() {
		w.processEvent(event, filename)
	})
}

// processEvent handles the actual sync after debounce
func (w *Watcher) processEvent(event fsnotify.Event, filename string) {
	noteID := strings.TrimSuffix(filename, ".md")

	switch {
	case event.Op&fsnotify.Create == fsnotify.Create:
		log.Printf("[NotesWatcher] File created: %s", filename)
		w.syncFileToDB(noteID)

	case event.Op&fsnotify.Write == fsnotify.Write:
		log.Printf("[NotesWatcher] File modified: %s", filename)
		w.syncFileToDB(noteID)

	case event.Op&fsnotify.Remove == fsnotify.Remove:
		log.Printf("[NotesWatcher] File deleted: %s", filename)
		w.deleteFromDB(noteID)

	case event.Op&fsnotify.Rename == fsnotify.Rename:
		// On Windows, rename is often followed by remove
		// Check if file still exists
		if _, err := os.Stat(event.Name); os.IsNotExist(err) {
			log.Printf("[NotesWatcher] File renamed/deleted: %s", filename)
			w.deleteFromDB(noteID)
		}
	}
}

// syncFileToDB syncs a single file to the database
func (w *Watcher) syncFileToDB(noteID string) {
	note, err := w.api.fileHandler.ReadNote(noteID)
	if err != nil {
		log.Printf("[NotesWatcher] Error reading file %s: %v", noteID, err)
		return
	}

	if err := w.api.syncer.WriteNoteToDB(note); err != nil {
		log.Printf("[NotesWatcher] Error syncing to DB %s: %v", noteID, err)
	}
}

// deleteFromDB soft deletes a note from the database
func (w *Watcher) deleteFromDB(noteID string) {
	if err := w.api.syncer.SoftDeleteNote(noteID); err != nil {
		log.Printf("[NotesWatcher] Error deleting from DB %s: %v", noteID, err)
	}
}
