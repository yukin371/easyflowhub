/**
 * TodoCardPage - 桌面悬浮待办卡片
 * 紧凑设计，可固定到桌面底层
 * 已完成项在保留期内显示删除线
 */

import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listNotes, saveNote, getNote } from '../lib/tauri/notes';
import { getSettings } from '../lib/tauri/settings';
import { extractAllTodos, toggleTodoInContent, isWithinRetention } from '../lib/todoParser';
import type { TodoItem } from '../types/todo';

export function TodoCardPage() {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [pinned, setPinned] = useState(false);
  const windowLabel = getCurrentWindow().label;

  const loadTodos = useCallback(async () => {
    try {
      const [notes, settings] = await Promise.all([
        listNotes(),
        getSettings(),
      ]);
      const hours = settings.todo?.done_retention_hours ?? 24;
      const allTodos = extractAllTodos(
        notes.map((n) => ({ id: n.id, title: n.title, content: n.content }))
      );
      // 只保留未完成 + 保留期内的已完成项
      setTodos(allTodos.filter((t) => !t.checked || isWithinRetention(t.checkedAt, hours)));
    } catch (err) {
      console.error('Failed to load todos:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadTodos();
    const interval = setInterval(loadTodos, 30000);
    return () => clearInterval(interval);
  }, [loadTodos]);

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

  const handleClose = useCallback(() => {
    getCurrentWindow().close();
  }, []);

  const handleTogglePin = useCallback(async () => {
    try {
      const newPinned = !pinned;
      await invoke('toggle_todo_card_pin', { label: windowLabel, pinned: newPinned });
      getCurrentWindow().setAlwaysOnTop(!newPinned);
      setPinned(newPinned);
    } catch (err) {
      console.error('Failed to toggle pin:', err);
    }
  }, [pinned, windowLabel]);

  const pending = todos.filter((t) => !t.checked);
  const recentlyDone = todos.filter((t) => t.checked);

  return (
    <div
      className="flex h-screen flex-col overflow-hidden rounded-2xl bg-white/92 shadow-lg backdrop-blur-md"
      style={{ border: '1px solid rgba(0,0,0,0.06)' }}
    >
      {/* Drag bar - always visible */}
      <div
        className="flex h-8 shrink-0 items-center justify-between px-2 select-none"
        data-tauri-drag-region
      >
        <span
          className="text-[10px] text-gray-400"
          data-tauri-drag-region
        >
          {loading ? '' : `${pending.length} 项`}
        </span>
        <div className="flex items-center gap-0.5">
          <button
            onClick={() => void loadTodos()}
            className="flex h-5 w-5 items-center justify-center rounded text-[10px] text-gray-400 transition hover:bg-gray-100 hover:text-gray-500"
            title="刷新"
          >
            ↻
          </button>
          <button
            onClick={() => void handleTogglePin()}
            className={`flex h-5 w-5 items-center justify-center rounded transition ${
              pinned
                ? 'text-amber-500 hover:text-amber-600'
                : 'text-gray-400 hover:bg-gray-100 hover:text-gray-500'
            }`}
            title={pinned ? '取消桌面固定' : '固定到桌面'}
          >
            <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <path d="M9.828 1.172a2.828 2.828 0 1 1 4 4L12 7l3 3-2 2-3-3-1.828 1.828a2.828 2.828 0 1 1-4-4L6 5 3 2l1-1 3 3 1.828-1.828z" />
              <path d="M5 11l-3 3" />
            </svg>
          </button>
          {!pinned && (
            <button
              onClick={handleClose}
              className="flex h-5 w-5 items-center justify-center rounded text-[10px] text-gray-400 transition hover:bg-red-50 hover:text-red-400"
              title="关闭"
            >
              ✕
            </button>
          )}
        </div>
      </div>

      {/* Todo list */}
      <div className="flex-1 overflow-auto px-2.5 pb-2">
        {loading ? (
          <div className="flex items-center justify-center py-4 text-xs text-gray-400">
            ...
          </div>
        ) : todos.length === 0 ? (
          <div className="flex items-center justify-center py-4 text-xs text-gray-400">
            全部完成
          </div>
        ) : (
          <ul className="space-y-0.5">
            {pending.map((todo) => (
              <li key={todo.id}>
                <button
                  onClick={() => void handleToggle(todo)}
                  className="flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left transition hover:bg-gray-50"
                >
                  <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded border border-gray-300 bg-white" />
                  <span className="min-w-0 flex-1 truncate text-[13px] leading-tight text-gray-700">
                    {todo.text}
                  </span>
                </button>
              </li>
            ))}
            {recentlyDone.length > 0 && (
              <>
                {pending.length > 0 && (
                  <li className="my-1 border-t border-gray-100" />
                )}
                {recentlyDone.map((todo) => (
                  <li key={todo.id}>
                    <button
                      onClick={() => void handleToggle(todo)}
                      className="flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left transition hover:bg-gray-50"
                    >
                      <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded border border-gray-200 bg-gray-100">
                        <svg width="8" height="8" viewBox="0 0 12 12" fill="none" stroke="#9ca3af" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M2 6l3 3 5-5" />
                        </svg>
                      </span>
                      <span className="min-w-0 flex-1 truncate text-[13px] leading-tight text-gray-400 line-through">
                        {todo.text}
                      </span>
                    </button>
                  </li>
                ))}
              </>
            )}
          </ul>
        )}
      </div>
    </div>
  );
}
