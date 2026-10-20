import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useTodoEditSession } from './useTodoEditSession';
import type { TodoWriteResult } from './useTodos';
import type { TodoItem } from '../types/todo';

function createTodo(overrides: Partial<TodoItem> = {}): TodoItem {
  return {
    id: 'note-1:0',
    text: 'ship docs',
    checked: false,
    checkedAt: null,
    noteId: 'note-1',
    lineIndex: 0,
    noteTitle: 'Note one',
    ...overrides,
  };
}

const ok: TodoWriteResult = { ok: true };
const conflict: TodoWriteResult = { ok: false, failure: 'conflict', message: '同名待办不止一条' };

describe('useTodoEditSession', () => {
  it('surfaces write failure messages and returns the result from toggle', async () => {
    const toggleTodo = vi.fn(async () => conflict);
    const { result } = renderHook(() => useTodoEditSession({ toggleTodo, updateTodoText: vi.fn() }));

    let outcome: TodoWriteResult | undefined;
    await act(async () => {
      outcome = await result.current.toggle(createTodo());
    });

    expect(outcome).toEqual(conflict);
    expect(result.current.writeError).toBe('同名待办不止一条');

    act(() => result.current.clearWriteError());
    expect(result.current.writeError).toBeNull();
  });

  it('cancels the edit when the draft is empty or unchanged', async () => {
    const updateTodoText = vi.fn(async () => ok);
    const { result } = renderHook(() => useTodoEditSession({ toggleTodo: vi.fn(), updateTodoText }));
    const todo = createTodo();

    act(() => result.current.startEdit(todo));
    expect(result.current.editingTodoId).toBe(todo.id);

    act(() => result.current.setTodoDraft('   '));
    await act(async () => {
      await result.current.saveEdit(todo);
    });
    expect(updateTodoText).not.toHaveBeenCalled();
    expect(result.current.editingTodoId).toBeNull();

    act(() => result.current.startEdit(todo));
    act(() => result.current.setTodoDraft(todo.text));
    await act(async () => {
      await result.current.saveEdit(todo);
    });
    expect(updateTodoText).not.toHaveBeenCalled();
    expect(result.current.editingTodoId).toBeNull();
  });

  it('keeps the draft open on failure when keepEditOpenOnFailure is set', async () => {
    const updateTodoText = vi.fn(async () => conflict);
    const { result } = renderHook(() =>
      useTodoEditSession({ toggleTodo: vi.fn(), updateTodoText, keepEditOpenOnFailure: true })
    );
    const todo = createTodo();

    act(() => result.current.startEdit(todo));
    act(() => result.current.setTodoDraft('next text'));

    let outcome: TodoWriteResult | null | undefined;
    await act(async () => {
      outcome = await result.current.saveEdit(todo);
    });

    expect(outcome).toEqual(conflict);
    // 悬浮卡片：失败时保留编辑态与草稿，输入框解除禁用以便重试。
    expect(result.current.editingTodoId).toBe(todo.id);
    expect(result.current.todoDraft).toBe('next text');
    expect(result.current.savingTodoId).toBeNull();
    expect(result.current.writeError).toBe('同名待办不止一条');
  });

  it('exits the edit on failure by default (manager panel behavior)', async () => {
    const updateTodoText = vi.fn(async () => conflict);
    const { result } = renderHook(() => useTodoEditSession({ toggleTodo: vi.fn(), updateTodoText }));
    const todo = createTodo();

    act(() => result.current.startEdit(todo));
    act(() => result.current.setTodoDraft('next text'));
    await act(async () => {
      await result.current.saveEdit(todo);
    });

    expect(result.current.editingTodoId).toBeNull();
    expect(result.current.writeError).toBe('同名待办不止一条');
  });

  it('clears the edit after a successful save', async () => {
    const updateTodoText = vi.fn(async () => ok);
    const { result } = renderHook(() =>
      useTodoEditSession({ toggleTodo: vi.fn(), updateTodoText, keepEditOpenOnFailure: true })
    );
    const todo = createTodo();

    act(() => result.current.startEdit(todo));
    act(() => result.current.setTodoDraft('next text'));
    await act(async () => {
      await result.current.saveEdit(todo);
    });

    expect(updateTodoText).toHaveBeenCalledWith(todo, 'next text');
    expect(result.current.editingTodoId).toBeNull();
    expect(result.current.todoDraft).toBe('');
    expect(result.current.writeError).toBeNull();
  });
});
