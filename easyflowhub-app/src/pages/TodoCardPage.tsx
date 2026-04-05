/**
 * TodoCardPage - 桌面悬浮待办卡片
 * 向快速笔记靠拢的极简待办浮窗
 */

import { useEffect, useMemo, useRef, useState, useCallback, type CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { createNote, getNote, listNotes, saveNote } from '../lib/tauri/notes';
import { getSettings } from '../lib/tauri/settings';
import {
  extractAllTodos,
  isWithinRetention,
  toggleTodoInContent,
  updateTodoTextInContent,
} from '../lib/todoParser';
import type { Note } from '../types/note';
import type { TodoItem } from '../types/todo';

const BG_COLOR = '#FAF8F0';
const TODO_INBOX_TITLE = '待办箱';

function appendTodoLine(content: string, text: string) {
  const normalizedText = text.trim();
  if (!normalizedText) {
    return content;
  }

  if (!content.trim()) {
    return `- [ ] ${normalizedText}`;
  }

  return `${content.replace(/\s*$/, '')}\n- [ ] ${normalizedText}`;
}

export function TodoCardPage() {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [pinned, setPinned] = useState(false);
  const [editingTodoId, setEditingTodoId] = useState<string | null>(null);
  const [todoDraft, setTodoDraft] = useState('');
  const [isComposerOpen, setIsComposerOpen] = useState(false);
  const [newTodoText, setNewTodoText] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [isWindowFocused, setIsWindowFocused] = useState(true);

  const addInputRef = useRef<HTMLInputElement | null>(null);
  const editInputRef = useRef<HTMLInputElement | null>(null);
  const windowLabel = getCurrentWindow().label;

  const loadTodos = useCallback(async () => {
    try {
      setLoading(true);
      const [notes, settings] = await Promise.all([listNotes(), getSettings()]);
      const hours = settings.todo?.done_retention_hours ?? 24;
      const allTodos = extractAllTodos(
        notes.map((note) => ({ id: note.id, title: note.title, content: note.content }))
      );
      setTodos(allTodos.filter((todo) => !todo.checked || isWithinRetention(todo.checkedAt, hours)));
    } catch (err) {
      console.error('Failed to load todos:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadTodos();
    const interval = window.setInterval(() => {
      void loadTodos();
    }, 30000);
    return () => window.clearInterval(interval);
  }, [loadTodos]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupFocusState = async () => {
      try {
        const win = getCurrentWindow();
        setIsWindowFocused(await win.isFocused());
        unlisten = await win.onFocusChanged(({ payload }) => {
          setIsWindowFocused(payload);
        });
      } catch (error) {
        console.error('Failed to track todo card focus:', error);
      }
    };

    void setupFocusState();

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (isComposerOpen) {
      window.requestAnimationFrame(() => addInputRef.current?.focus());
    }
  }, [isComposerOpen]);

  useEffect(() => {
    if (editingTodoId) {
      window.requestAnimationFrame(() => editInputRef.current?.focus());
    }
  }, [editingTodoId]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!document.hasFocus() || !isWindowFocused) {
        return;
      }

      if (event.key !== 'Escape') {
        return;
      }

      if (editingTodoId) {
        event.preventDefault();
        setEditingTodoId(null);
        setTodoDraft('');
        return;
      }

      if (isComposerOpen) {
        event.preventDefault();
        setIsComposerOpen(false);
        setNewTodoText('');
        return;
      }

      event.preventDefault();
      void getCurrentWindow().close();
    };

    window.addEventListener('keydown', handleKeyDown, { capture: true });
    return () => window.removeEventListener('keydown', handleKeyDown, { capture: true });
  }, [editingTodoId, isComposerOpen, isWindowFocused]);

  const groupedTodos = useMemo(() => {
    const groups = new Map<string, TodoItem[]>();

    for (const todo of todos) {
      const list = groups.get(todo.noteId) ?? [];
      list.push(todo);
      groups.set(todo.noteId, list);
    }

    return Array.from(groups.values());
  }, [todos]);

  const pendingCount = todos.filter((todo) => !todo.checked).length;

  const handleToggle = useCallback(async (todo: TodoItem) => {
    try {
      const note = await getNote(todo.noteId);
      if (!note) return;
      const newContent = toggleTodoInContent(note.content, todo.lineIndex);
      if (newContent === null) return;
      await saveNote({ id: todo.noteId, content: newContent });
      await loadTodos();
    } catch (err) {
      console.error('Failed to toggle todo:', err);
    }
  }, [loadTodos]);

  const handleTogglePin = useCallback(async () => {
    try {
      const nextPinned = !pinned;
      await invoke('toggle_todo_card_pin', { label: windowLabel, pinned: nextPinned });
      await getCurrentWindow().setAlwaysOnTop(!nextPinned);
      setPinned(nextPinned);
    } catch (err) {
      console.error('Failed to toggle pin:', err);
    }
  }, [pinned, windowLabel]);

  const ensureTodoInboxNote = useCallback(async (): Promise<Note> => {
    const notes = await listNotes();
    const existing = notes.find((note) => note.title.trim() === TODO_INBOX_TITLE);
    if (existing) {
      return existing;
    }

    const created = await createNote();
    return await saveNote({
      id: created.id,
      title: TODO_INBOX_TITLE,
      content: '',
      tags: created.tags,
    });
  }, []);

  const handleCreateTodo = useCallback(async () => {
    const normalizedText = newTodoText.trim();
    if (!normalizedText) {
      setIsComposerOpen(false);
      setNewTodoText('');
      return;
    }

    try {
      setSubmitting(true);
      const inbox = await ensureTodoInboxNote();
      const updatedContent = appendTodoLine(inbox.content, normalizedText);
      await saveNote({ id: inbox.id, content: updatedContent, title: inbox.title, tags: inbox.tags });
      setNewTodoText('');
      setIsComposerOpen(false);
      await loadTodos();
    } catch (error) {
      console.error('Failed to create todo:', error);
    } finally {
      setSubmitting(false);
    }
  }, [ensureTodoInboxNote, loadTodos, newTodoText]);

  const handleStartEdit = useCallback((todo: TodoItem) => {
    setEditingTodoId(todo.id);
    setTodoDraft(todo.text);
    setIsComposerOpen(false);
  }, []);

  const handleSaveEdit = useCallback(async (todo: TodoItem) => {
    const normalizedText = todoDraft.trim();
    if (!normalizedText) {
      setEditingTodoId(null);
      setTodoDraft('');
      return;
    }

    if (normalizedText === todo.text) {
      setEditingTodoId(null);
      setTodoDraft('');
      return;
    }

    try {
      setSubmitting(true);
      const note = await getNote(todo.noteId);
      if (!note) {
        return;
      }

      const updatedContent = updateTodoTextInContent(note.content, todo.lineIndex, normalizedText);
      if (updatedContent === null) {
        return;
      }

      await saveNote({ id: note.id, title: note.title, content: updatedContent, tags: note.tags });
      setEditingTodoId(null);
      setTodoDraft('');
      await loadTodos();
    } catch (error) {
      console.error('Failed to update todo:', error);
    } finally {
      setSubmitting(false);
    }
  }, [loadTodos, todoDraft]);

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        display: 'flex',
        flexDirection: 'column',
        backgroundColor: BG_COLOR,
      }}
    >
      <div
        data-tauri-drag-region
        style={{
          position: 'relative',
          height: 32,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          paddingInline: 8,
          backgroundColor: 'rgba(250, 248, 240, 0.92)',
          borderBottom: '1px solid rgba(109, 90, 62, 0.08)',
          flexShrink: 0,
        }}
      >
        <div
          style={{
            paddingInline: 8,
            fontSize: 10,
            letterSpacing: '0.22em',
            color: 'rgba(139, 125, 105, 0.56)',
          }}
          data-tauri-drag-region
        >
          {loading ? '' : `${pendingCount}`}
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <button
            onClick={() => {
              setEditingTodoId(null);
              setTodoDraft('');
              setIsComposerOpen((prev) => !prev);
            }}
            title="新增待办"
            style={toolbarButtonStyle}
          >
            +
          </button>
          <button onClick={() => void loadTodos()} title="刷新" style={toolbarButtonStyle}>
            ↻
          </button>
          <button
            onClick={() => void handleTogglePin()}
            title={pinned ? '取消桌面固定' : '固定到桌面'}
            style={{
              ...toolbarButtonStyle,
              color: pinned ? '#3b82f6' : '#9ca3af',
            }}
          >
            <svg width="12" height="12" fill={pinned ? 'currentColor' : 'none'} stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
            </svg>
          </button>
          <button
            onClick={() => void getCurrentWindow().close()}
            title="关闭"
            style={toolbarButtonStyle}
          >
            ×
          </button>
        </div>
      </div>

      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', padding: 8, gap: 8, overflow: 'hidden' }}>
        {isComposerOpen ? (
          <div
            style={{
              borderRadius: 14,
              background: 'rgba(255,255,255,0.68)',
              border: '1px solid rgba(109, 90, 62, 0.1)',
              padding: 8,
            }}
          >
            <input
              ref={addInputRef}
              value={newTodoText}
              disabled={submitting}
              onChange={(event) => setNewTodoText(event.target.value)}
              onBlur={() => {
                void handleCreateTodo();
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  void handleCreateTodo();
                }

                if (event.key === 'Escape') {
                  event.preventDefault();
                  setIsComposerOpen(false);
                  setNewTodoText('');
                }
              }}
              placeholder="..."
              style={inputStyle}
            />
          </div>
        ) : null}

        <div style={{ flex: 1, overflowY: 'auto', paddingInline: 2 }}>
          {loading ? (
            <div style={emptyStateStyle}>...</div>
          ) : todos.length === 0 ? (
            <div style={emptyStateStyle}>○</div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {groupedTodos.map((group, groupIndex) => (
                <div key={`${group[0]?.noteId ?? groupIndex}`}>
                  {groupIndex > 0 ? (
                    <div
                      style={{
                        height: 1,
                        margin: '2px 6px 8px',
                        background: 'linear-gradient(90deg, transparent, rgba(109, 90, 62, 0.16), transparent)',
                      }}
                    />
                  ) : null}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {group.map((todo) => {
                      const isEditing = editingTodoId === todo.id;

                      return (
                        <div
                          key={todo.id}
                          style={{
                            display: 'flex',
                            alignItems: 'flex-start',
                            gap: 8,
                            borderRadius: 12,
                            padding: '4px 6px',
                            background: isEditing ? 'rgba(255,255,255,0.75)' : 'transparent',
                          }}
                        >
                          <button
                            onClick={() => void handleToggle(todo)}
                            style={{
                              ...checkboxStyle,
                              background: todo.checked ? 'rgba(109, 90, 62, 0.2)' : 'rgba(255,255,255,0.84)',
                              borderColor: todo.checked ? 'rgba(109, 90, 62, 0.22)' : 'rgba(109, 90, 62, 0.18)',
                            }}
                          >
                            {todo.checked ? (
                              <svg width="9" height="9" viewBox="0 0 12 12" fill="none" stroke="#6d5a3e" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <path d="M2 6l3 3 5-5" />
                              </svg>
                            ) : null}
                          </button>

                          {isEditing ? (
                            <input
                              ref={editInputRef}
                              value={todoDraft}
                              disabled={submitting}
                              onChange={(event) => setTodoDraft(event.target.value)}
                              onBlur={() => {
                                void handleSaveEdit(todo);
                              }}
                              onKeyDown={(event) => {
                                if (event.key === 'Enter') {
                                  event.preventDefault();
                                  void handleSaveEdit(todo);
                                }

                                if (event.key === 'Escape') {
                                  event.preventDefault();
                                  setEditingTodoId(null);
                                  setTodoDraft('');
                                }
                              }}
                              style={inputStyle}
                            />
                          ) : (
                            <button
                              onClick={() => handleStartEdit(todo)}
                              style={{
                                flex: 1,
                                border: 'none',
                                background: 'transparent',
                                padding: 0,
                                textAlign: 'left',
                                cursor: 'text',
                                color: todo.checked ? 'rgba(109, 90, 62, 0.38)' : '#4b3d2b',
                                fontSize: 13,
                                lineHeight: 1.45,
                                textDecoration: todo.checked ? 'line-through' : 'none',
                                wordBreak: 'break-word',
                              }}
                            >
                              {todo.text}
                            </button>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

const toolbarButtonStyle: CSSProperties = {
  width: 22,
  height: 22,
  borderRadius: 999,
  border: 'none',
  background: 'transparent',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  color: '#8b7d69',
  cursor: 'pointer',
  fontSize: 13,
  lineHeight: 1,
};

const checkboxStyle: CSSProperties = {
  width: 16,
  height: 16,
  marginTop: 1,
  flexShrink: 0,
  borderRadius: 999,
  border: '1px solid rgba(109, 90, 62, 0.18)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  cursor: 'pointer',
};

const inputStyle: CSSProperties = {
  width: '100%',
  border: 'none',
  outline: 'none',
  background: 'transparent',
  color: '#4b3d2b',
  fontSize: 13,
  lineHeight: 1.45,
  padding: '2px 0',
};

const emptyStateStyle: CSSProperties = {
  minHeight: '100%',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  color: 'rgba(139, 125, 105, 0.56)',
  fontSize: 14,
  letterSpacing: '0.2em',
};
