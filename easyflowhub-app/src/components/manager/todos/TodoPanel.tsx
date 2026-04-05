/**
 * TodoPanel - 全局待办事项聚合面板
 * 展示所有笔记中的 checkbox 待办，支持筛选、勾选和跳转
 */

import { useEffect, useState, useCallback, useMemo } from 'react';
import { listNotes, saveNote, getNote, createTodoCardWindow } from '../../../lib/tauri/notes';
import { getSettings } from '../../../lib/tauri/settings';
import {
  compareTodoGroupsForDisplay,
  extractAllTodos,
  getTodoStats,
  toggleTodoInContent,
  isWithinRetention,
  sortTodosForDisplay,
  TODO_INBOX_TITLE,
  updateTodoTextInContent,
} from '../../../lib/todoParser';
import type { TodoItem, TodoFilter } from '../../../types/todo';
import { MANAGER_ACTIVATED_EVENT } from '../ManagerPage';
import { ManagerPanelHeader } from '../shared/ManagerPanelHeader';

interface TodoPanelProps {
  onNavigateToNote?: (noteId: string) => void;
}

export function TodoPanel({ onNavigateToNote }: TodoPanelProps) {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [filter, setFilter] = useState<TodoFilter>('pending');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingTodoId, setEditingTodoId] = useState<string | null>(null);
  const [todoDraft, setTodoDraft] = useState('');
  const [savingTodoId, setSavingTodoId] = useState<string | null>(null);
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

  const handleStartEdit = useCallback((todo: TodoItem) => {
    setEditingTodoId(todo.id);
    setTodoDraft(todo.text);
  }, []);

  const handleCancelEdit = useCallback(() => {
    setEditingTodoId(null);
    setTodoDraft('');
    setSavingTodoId(null);
  }, []);

  const handleSaveEdit = useCallback(async (todo: TodoItem) => {
    const normalizedText = todoDraft.trim();
    if (!normalizedText) {
      handleCancelEdit();
      return;
    }

    if (normalizedText === todo.text) {
      handleCancelEdit();
      return;
    }

    try {
      setSavingTodoId(todo.id);
      const note = await getNote(todo.noteId);
      if (!note) {
        handleCancelEdit();
        return;
      }

      const newContent = updateTodoTextInContent(note.content, todo.lineIndex, normalizedText);
      if (newContent === null) {
        handleCancelEdit();
        return;
      }

      await saveNote({ id: todo.noteId, content: newContent });
      await loadTodos();
    } catch (err) {
      console.error('Failed to update todo:', err);
    } finally {
      handleCancelEdit();
    }
  }, [handleCancelEdit, loadTodos, todoDraft]);

  const filteredTodos = todos.filter((todo) => {
    if (filter === 'pending') return !todo.checked;
    if (filter === 'done') return todo.checked;
    return true;
  });
  const groupedTodos = useMemo(() => {
    const groups = new Map<string, { noteId: string; noteTitle: string; todos: TodoItem[] }>();
    for (const todo of filteredTodos) {
      const existing = groups.get(todo.noteId);
      if (existing) {
        existing.todos.push(todo);
        continue;
      }

      groups.set(todo.noteId, {
        noteId: todo.noteId,
        noteTitle: todo.noteTitle,
        todos: [todo],
      });
    }

    return Array.from(groups.values())
      .map((group) => ({
        ...group,
        todos: sortTodosForDisplay(group.todos),
      }))
      .sort(compareTodoGroupsForDisplay);
  }, [filteredTodos]);

  const pendingGroups = useMemo(
    () =>
      groupedTodos
        .map((group) => ({
          ...group,
          todos: group.todos.filter((todo) => !todo.checked),
        }))
        .filter((group) => group.todos.length > 0),
    [groupedTodos]
  );

  const completedGroups = useMemo(
    () =>
      groupedTodos
        .map((group) => ({
          ...group,
          todos: group.todos.filter((todo) => todo.checked),
        }))
        .filter((group) => group.todos.length > 0),
    [groupedTodos]
  );

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
        <ManagerPanelHeader
          kicker="Todos"
          title="待办事项"
          description={`共 ${stats.total} 项 · 已完成 ${stats.done} · 待处理 ${stats.pending}`}
          actions={
            <>
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
            </>
          }
        />

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
              在笔记中使用 <code className="rounded bg-black/5 px-1.5 py-0.5 font-mono text-xs">- [ ]</code> 或 <code className="rounded bg-black/5 px-1.5 py-0.5 font-mono text-xs">1. [ ]</code> 语法添加待办
            </p>
          </div>
        ) : (
          <div className="flex min-h-full flex-col">
            <div className="space-y-6">
              {(filter === 'done' ? completedGroups : pendingGroups).map((group, groupIndex) => (
                <section key={group.noteId}>
                  {groupIndex > 0 ? <hr className="mb-6 border-[color:var(--manager-border)]/60" /> : null}
                  <div className="mb-3 flex items-center justify-between gap-3 px-1">
                    {group.noteTitle.trim() !== TODO_INBOX_TITLE ? (
                      <button
                        onClick={() => onNavigateToNote?.(group.noteId)}
                        className="min-w-0 text-left"
                      >
                        <p className="truncate text-sm font-medium text-[color:var(--manager-ink-strong)]">
                          {group.noteTitle}
                        </p>
                        <p className="text-[11px] uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">
                          来源文件
                        </p>
                      </button>
                    ) : (
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium text-[color:var(--manager-ink-strong)]">
                          {group.noteTitle}
                        </p>
                        <p className="text-[11px] uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">
                          收件箱
                        </p>
                      </div>
                    )}
                    <span className="shrink-0 text-xs text-[color:var(--manager-ink-subtle)]">
                      {group.todos.length} 项
                    </span>
                  </div>

                  <ul className="space-y-2">
                    {group.todos.map((todo) => {
                      const isEditing = editingTodoId === todo.id;
                      const isSaving = savingTodoId === todo.id;

                      return (
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
                              {isEditing ? (
                                <input
                                  autoFocus
                                  value={todoDraft}
                                  disabled={isSaving}
                                  onChange={(event) => setTodoDraft(event.target.value)}
                                  onBlur={() => void handleSaveEdit(todo)}
                                  onKeyDown={(event) => {
                                    if (event.key === 'Enter') {
                                      event.preventDefault();
                                      void handleSaveEdit(todo);
                                    }

                                    if (event.key === 'Escape') {
                                      event.preventDefault();
                                      handleCancelEdit();
                                    }
                                  }}
                                  className="w-full rounded-lg border border-[color:var(--manager-accent)] bg-white px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none"
                                />
                              ) : (
                                <p className={`text-sm leading-relaxed ${todo.checked ? 'text-[color:var(--manager-ink-muted)] line-through' : 'text-[color:var(--manager-ink)]'}`}>
                                  {todo.text}
                                </p>
                              )}
                              {todo.noteTitle.trim() !== TODO_INBOX_TITLE ? (
                                <button
                                  onClick={() => onNavigateToNote?.(todo.noteId)}
                                  className="mt-1 text-[11px] uppercase tracking-[0.15em] text-[color:var(--manager-ink-subtle)] hover:text-[color:var(--manager-accent)]"
                                >
                                  打开原笔记
                                </button>
                              ) : null}
                            </div>
                            {!isEditing ? (
                              <button
                                onClick={() => handleStartEdit(todo)}
                                className="rounded-full border border-[color:var(--manager-border)] px-3 py-1 text-xs text-[color:var(--manager-ink-soft)] opacity-0 transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)] group-hover:opacity-100"
                              >
                                编辑
                              </button>
                            ) : (
                              <div className="flex items-center gap-2">
                                <button
                                  onMouseDown={(event) => event.preventDefault()}
                                  onClick={() => void handleSaveEdit(todo)}
                                  className="rounded-full bg-[color:var(--manager-accent)] px-3 py-1 text-xs text-white transition hover:opacity-90"
                                >
                                  保存
                                </button>
                                <button
                                  onMouseDown={(event) => event.preventDefault()}
                                  onClick={handleCancelEdit}
                                  className="rounded-full border border-[color:var(--manager-border)] px-3 py-1 text-xs text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
                                >
                                  取消
                                </button>
                              </div>
                            )}
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                </section>
              ))}
            </div>
            {filter === 'all' && completedGroups.length > 0 ? <div className="min-h-8 flex-1" /> : null}
            {filter === 'all' && completedGroups.length > 0 ? (
              <>
                <div className="flex items-center gap-4 px-1 pt-2">
                  <span className="manager-kicker">Recently Done</span>
                  <div className="h-px flex-1 bg-[color:var(--manager-border)]" />
                </div>
                {completedGroups.map((group) => (
                  <section key={`done-${group.noteId}`}>
                    <div className="mb-3 flex items-center justify-between gap-3 px-1">
                      {group.noteTitle.trim() !== TODO_INBOX_TITLE ? (
                        <button
                          onClick={() => onNavigateToNote?.(group.noteId)}
                          className="min-w-0 text-left"
                        >
                          <p className="truncate text-sm font-medium text-[color:var(--manager-ink-strong)]">
                            {group.noteTitle}
                          </p>
                          <p className="text-[11px] uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">
                            来源文件
                          </p>
                        </button>
                      ) : (
                        <div className="min-w-0">
                          <p className="truncate text-sm font-medium text-[color:var(--manager-ink-strong)]">
                            {group.noteTitle}
                          </p>
                          <p className="text-[11px] uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">
                            收件箱
                          </p>
                        </div>
                      )}
                      <span className="shrink-0 text-xs text-[color:var(--manager-ink-subtle)]">
                        {group.todos.length} 项
                      </span>
                    </div>

                    <ul className="space-y-2">
                      {group.todos.map((todo) => {
                        const isEditing = editingTodoId === todo.id;
                        const isSaving = savingTodoId === todo.id;

                        return (
                          <li key={todo.id}>
                            <div className="group flex w-full items-start gap-3 rounded-[14px] border border-transparent px-4 py-3 transition hover:border-[color:var(--manager-border)] hover:bg-white/55">
                              <button
                                onClick={() => void handleToggle(todo)}
                                className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded border border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-xs text-white transition"
                              >
                                ✓
                              </button>
                              <div className="min-w-0 flex-1">
                                {isEditing ? (
                                  <input
                                    autoFocus
                                    value={todoDraft}
                                    disabled={isSaving}
                                    onChange={(event) => setTodoDraft(event.target.value)}
                                    onBlur={() => void handleSaveEdit(todo)}
                                    onKeyDown={(event) => {
                                      if (event.key === 'Enter') {
                                        event.preventDefault();
                                        void handleSaveEdit(todo);
                                      }

                                      if (event.key === 'Escape') {
                                        event.preventDefault();
                                        handleCancelEdit();
                                      }
                                    }}
                                    className="w-full rounded-lg border border-[color:var(--manager-accent)] bg-white px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none"
                                  />
                                ) : (
                                  <p className="text-sm leading-relaxed text-[color:var(--manager-ink-muted)] line-through">
                                    {todo.text}
                                  </p>
                                )}
                                {todo.noteTitle.trim() !== TODO_INBOX_TITLE ? (
                                  <button
                                    onClick={() => onNavigateToNote?.(todo.noteId)}
                                    className="mt-1 text-[11px] uppercase tracking-[0.15em] text-[color:var(--manager-ink-subtle)] hover:text-[color:var(--manager-accent)]"
                                  >
                                    打开原笔记
                                  </button>
                                ) : null}
                              </div>
                              {!isEditing ? (
                                <button
                                  onClick={() => handleStartEdit(todo)}
                                  className="rounded-full border border-[color:var(--manager-border)] px-3 py-1 text-xs text-[color:var(--manager-ink-soft)] opacity-0 transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)] group-hover:opacity-100"
                                >
                                  编辑
                                </button>
                              ) : (
                                <div className="flex items-center gap-2">
                                  <button
                                    onMouseDown={(event) => event.preventDefault()}
                                    onClick={() => void handleSaveEdit(todo)}
                                    className="rounded-full bg-[color:var(--manager-accent)] px-3 py-1 text-xs text-white transition hover:opacity-90"
                                  >
                                    保存
                                  </button>
                                  <button
                                    onMouseDown={(event) => event.preventDefault()}
                                    onClick={handleCancelEdit}
                                    className="rounded-full border border-[color:var(--manager-border)] px-3 py-1 text-xs text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
                                  >
                                    取消
                                  </button>
                                </div>
                              )}
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  </section>
                ))}
              </>
            ) : null}
          </div>
        )}
      </div>
    </section>
  );
}
