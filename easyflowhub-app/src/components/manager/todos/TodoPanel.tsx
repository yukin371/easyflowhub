/**
 * TodoPanel - 全局待办事项聚合面板
 * 展示所有笔记中的 checkbox 待办，支持筛选、勾选和跳转
 */

import { useEffect, useState, useCallback } from 'react';
import { listNotes, saveNote, getNote, createTodoCardWindow } from '../../../lib/tauri/notes';
import { getSettings } from '../../../lib/tauri/settings';
import { extractAllTodos, getTodoStats, toggleTodoInContent, isWithinRetention } from '../../../lib/todoParser';
import type { TodoItem, TodoFilter } from '../../../types/todo';
import { MANAGER_ACTIVATED_EVENT } from '../ManagerPage';

interface TodoPanelProps {
  onNavigateToNote?: (noteId: string) => void;
}

export function TodoPanel({ onNavigateToNote }: TodoPanelProps) {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [filter, setFilter] = useState<TodoFilter>('pending');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const loadTodos = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
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
      setError(err instanceof Error ? err.message : '加载待办失败');
    } finally {
      setLoading(false);
    }
  }, []);

  // 加载 & 窗口激活时自动刷新
  useEffect(() => {
    loadTodos();
    const handleActivate = () => loadTodos();
    window.addEventListener(MANAGER_ACTIVATED_EVENT, handleActivate);
    return () => window.removeEventListener(MANAGER_ACTIVATED_EVENT, handleActivate);
  }, [loadTodos]);

  // 切换 todo 完成状态
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

  const filteredTodos = todos.filter((todo) => {
    if (filter === 'pending') return !todo.checked;
    if (filter === 'done') return todo.checked;
    return true;
  });

  const stats = getTodoStats(todos);

  if (loading && todos.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-[color:var(--manager-ink-muted)]">
        加载中...
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-[color:var(--manager-ink-muted)]">
        <p>{error}</p>
        <button
          onClick={loadTodos}
          className="rounded-lg border border-[color:var(--manager-border)] px-4 py-2 text-sm hover:bg-white/50"
        >
          重试
        </button>
      </div>
    );
  }

  return (
    <section className="flex h-full flex-col">
      {/* Header */}
      <header className="border-b border-[color:var(--manager-border)] px-7 py-5">
        <div className="flex items-center justify-between">
          <div>
            <p className="manager-kicker">Todos</p>
            <h2 className="mt-1 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-2xl text-[color:var(--manager-ink-strong)]">
              待办事项
            </h2>
            <p className="mt-1 text-sm text-[color:var(--manager-ink-soft)]">
              共 {stats.total} 项 · 已完成 {stats.done} · 待处理 {stats.pending}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => void createTodoCardWindow()}
              className="rounded-lg border border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] px-3 py-1.5 text-sm text-[color:var(--manager-ink-strong)] transition hover:opacity-80"
              title="在桌面悬浮显示待办卡片"
            >
              悬浮卡片
            </button>
            <button
              onClick={loadTodos}
              className="rounded-lg border border-[color:var(--manager-border)] px-3 py-1.5 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
            >
              刷新
            </button>
          </div>
        </div>

        {/* Filter tabs */}
        <div className="mt-4 flex gap-2">
          {([
            { key: 'pending' as const, label: '待处理' },
            { key: 'done' as const, label: '已完成' },
            { key: 'all' as const, label: '全部' },
          ]).map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setFilter(key)}
              className={`rounded-full border px-4 py-1.5 text-sm transition ${
                filter === key
                  ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'border-[color:var(--manager-border)] bg-white/70 text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)]'
              }`}
            >
              {label}
              {key === 'pending' && stats.pending > 0 && (
                <span className="ml-1.5 text-xs opacity-60">{stats.pending}</span>
              )}
            </button>
          ))}
        </div>
      </header>

      {/* Todo list */}
      <div className="flex-1 overflow-auto px-7 py-4">
        {filteredTodos.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-[color:var(--manager-ink-muted)]">
            <p className="text-lg">
              {filter === 'pending' ? '没有待处理的事项' : filter === 'done' ? '没有已完成的事项' : '笔记中没有待办事项'}
            </p>
            <p className="mt-2 text-sm">
              在笔记中使用 <code className="rounded bg-black/5 px-1.5 py-0.5 font-mono text-xs">- [ ]</code> 语法添加待办
            </p>
          </div>
        ) : (
          <ul className="space-y-2">
            {filteredTodos.map((todo) => (
              <li key={todo.id}>
                <div className="group flex w-full items-start gap-3 rounded-[14px] border border-transparent px-4 py-3 transition hover:border-[color:var(--manager-border)] hover:bg-white/55">
                  <button
                    onClick={() => void handleToggle(todo)}
                    className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded border text-xs transition ${
                      todo.checked
                        ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-white'
                        : 'border-[color:var(--manager-border)] bg-white/80 hover:border-[color:var(--manager-accent)]'
                    }`}
                  >
                    {todo.checked ? '✓' : ''}
                  </button>
                  <div className="min-w-0 flex-1">
                    <p className={`text-sm leading-relaxed ${todo.checked ? 'text-[color:var(--manager-ink-muted)] line-through' : 'text-[color:var(--manager-ink)]'}`}>
                      {todo.text}
                    </p>
                    <button
                      onClick={() => onNavigateToNote?.(todo.noteId)}
                      className="mt-1 text-[11px] uppercase tracking-[0.15em] text-[color:var(--manager-ink-subtle)] hover:text-[color:var(--manager-accent)]"
                    >
                      来自「{todo.noteTitle}」
                    </button>
                  </div>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
