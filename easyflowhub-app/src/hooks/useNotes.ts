import { useState, useCallback, useEffect, useRef } from 'react';
import {
  listNotes,
  createNote,
  saveNote,
  deleteNote as deleteNoteApi,
  togglePinNote,
} from '../lib/tauri/notes';
import type { Note, Tab } from '../types/note';

const MIGRATION_KEY = 'easyflowhub_notes_migrated_v1';
const OLD_STORAGE_KEY = 'easyflowhub_notes';

// Old Note format from localStorage (number id)
interface LegacyNote {
  id: number;
  title: string;
  content: string;
  created_at: string;
  updated_at: string;
  is_pinned: boolean;
}

export function useNotes() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [migrationStatus, setMigrationStatus] = useState<'pending' | 'migrating' | 'done'>('pending');
  const saveTimeoutRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  // Create a new tab (defined early for use in useEffect)
  const handleNewTab = useCallback(async () => {
    try {
      const newNote = await createNote();

      setNotes((prev) => [...prev, newNote]);

      const newTab: Tab = {
        id: `tab-${Date.now()}`,
        noteId: newNote.id,
        title: newNote.title || '新笔记',
        isDirty: false,
      };

      setTabs((prev) => [...prev, newTab]);
      setActiveTabId(newTab.id);
    } catch (err) {
      console.error('Failed to create note:', err);
    }
  }, []);

  // Migrate old localStorage notes to SQLite
  const migrateFromLocalStorage = useCallback(async () => {
    // Check if already migrated
    const migrated = localStorage.getItem(MIGRATION_KEY);
    if (migrated) {
      setMigrationStatus('done');
      return;
    }

    // Check if SQLite already has notes
    const existingNotes = await listNotes();
    if (existingNotes.length > 0) {
      localStorage.setItem(MIGRATION_KEY, 'true');
      setMigrationStatus('done');
      return;
    }

    // Try to load old notes from localStorage
    try {
      const stored = localStorage.getItem(OLD_STORAGE_KEY);
      if (!stored) {
        localStorage.setItem(MIGRATION_KEY, 'true');
        setMigrationStatus('done');
        return;
      }

      const oldNotes: LegacyNote[] = JSON.parse(stored);
      if (oldNotes.length === 0) {
        localStorage.setItem(MIGRATION_KEY, 'true');
        setMigrationStatus('done');
        return;
      }

      console.log(`Migrating ${oldNotes.length} notes from localStorage...`);

      // Migrate each note
      for (const oldNote of oldNotes) {
        await saveNote({
          id: `migrated-${oldNote.id}`,
          title: oldNote.title,
          content: oldNote.content,
          created_at: oldNote.created_at,
          updated_at: oldNote.updated_at,
          is_pinned: oldNote.is_pinned,
        });
      }

      // Mark as migrated
      localStorage.setItem(MIGRATION_KEY, 'true');
      console.log('Migration completed successfully');
    } catch (err) {
      console.error('Migration failed:', err);
    }

    setMigrationStatus('done');
  }, []);

  // Load notes on mount
  useEffect(() => {
    const initialize = async () => {
      setIsLoading(true);
      try {
        // Run migration first
        if (migrationStatus === 'pending') {
          await migrateFromLocalStorage();
        }

        // Load notes from SQLite
        const loadedNotes = await listNotes();
        setNotes(loadedNotes);

        // Create a default tab if no tabs exist
        if (loadedNotes.length > 0) {
          const defaultNote = loadedNotes[0];
          const defaultTab: Tab = {
            id: `tab-${Date.now()}`,
            noteId: defaultNote.id,
            title: defaultNote.title || '新笔记',
            isDirty: false,
          };
          setTabs([defaultTab]);
          setActiveTabId(defaultTab.id);
        } else {
          // Create a new empty tab
          await handleNewTab();
        }
      } catch (err) {
        console.error('Failed to initialize notes:', err);
      } finally {
        setIsLoading(false);
      }
    };

    initialize();
  }, [migrationStatus, migrateFromLocalStorage, handleNewTab]);

  // Get the active tab
  const activeTab = tabs.find((t) => t.id === activeTabId) || null;

  // Get the active note
  const activeNote = activeTab
    ? notes.find((n) => n.id === activeTab.noteId) || null
    : null;

  // Select a tab
  const handleTabSelect = useCallback((tabId: string) => {
    setActiveTabId(tabId);
  }, []);

  // Close a tab
  const handleTabClose = useCallback(
    async (tabId: string) => {
      const tabIndex = tabs.findIndex((t) => t.id === tabId);
      const tab = tabs.find((t) => t.id === tabId);

      if (tabs.length === 1) {
        // Don't close the last tab, just clear it
        if (tab?.noteId) {
          try {
            const updatedNote = await saveNote({
              id: tab.noteId,
              content: '',
              title: '',
            });
            setNotes((prev) =>
              prev.map((n) => (n.id === tab.noteId ? updatedNote : n))
            );
            setTabs((prev) =>
              prev.map((t) =>
                t.id === tabId ? { ...t, title: '新笔记', isDirty: false } : t
              )
            );
          } catch (err) {
            console.error('Failed to clear note:', err);
          }
        }
        return;
      }

      // Remove the tab
      setTabs((prev) => prev.filter((t) => t.id !== tabId));

      // Select another tab if the active one was closed
      if (activeTabId === tabId) {
        const newTabIndex = Math.min(tabIndex, tabs.length - 2);
        setActiveTabId(tabs[newTabIndex].id);
      }
    },
    [tabs, activeTabId]
  );

  // Update note content with debounced save
  const updateNoteContent = useCallback(
    (content: string) => {
      if (!activeTab?.noteId) return;

      // Extract title from first line
      const lines = content.split('\n');
      const title = lines[0].replace(/^#+\s*/, '').slice(0, 50) || '新笔记';

      // Update local state immediately
      setNotes((prev) =>
        prev.map((n) =>
          n.id === activeTab.noteId
            ? { ...n, content, title, updated_at: new Date().toISOString() }
            : n
        )
      );

      setTabs((prev) =>
        prev.map((t) =>
          t.id === activeTabId ? { ...t, title, isDirty: true } : t
        )
      );

      // Debounce save to SQLite
      const noteId = activeTab.noteId;
      const existingTimeout = saveTimeoutRef.current.get(noteId);
      if (existingTimeout) {
        clearTimeout(existingTimeout);
      }

      const timeout = setTimeout(async () => {
        try {
          const updatedNote = await saveNote({
            id: noteId,
            content,
            title,
          });
          setNotes((prev) =>
            prev.map((n) => (n.id === noteId ? updatedNote : n))
          );

          // Clear dirty flag
          setTabs((prev) =>
            prev.map((t) => (t.id === activeTabId ? { ...t, isDirty: false } : t))
          );
        } catch (err) {
          console.error('Failed to save note:', err);
        }
        saveTimeoutRef.current.delete(noteId);
      }, 500);

      saveTimeoutRef.current.set(noteId, timeout);
    },
    [activeTab, activeTabId]
  );

  // Delete a note
  const deleteNote = useCallback(async (noteId: string) => {
    try {
      await deleteNoteApi(noteId);
      setNotes((prev) => prev.filter((n) => n.id !== noteId));
      setTabs((prev) => prev.filter((t) => t.noteId !== noteId));
    } catch (err) {
      console.error('Failed to delete note:', err);
    }
  }, []);

  // Toggle pin status
  const togglePin = useCallback(async (noteId: string) => {
    try {
      const updatedNote = await togglePinNote(noteId);
      setNotes((prev) =>
        prev.map((n) => (n.id === noteId ? updatedNote : n))
      );
    } catch (err) {
      console.error('Failed to toggle pin:', err);
    }
  }, []);

  // Cleanup timeouts on unmount
  useEffect(() => {
    return () => {
      saveTimeoutRef.current.forEach((timeout) => clearTimeout(timeout));
    };
  }, []);

  return {
    notes,
    tabs,
    activeTab,
    activeNote,
    activeTabId,
    isLoading,
    handleNewTab,
    handleTabSelect,
    handleTabClose,
    updateNoteContent,
    deleteNote,
    togglePin,
  };
}
