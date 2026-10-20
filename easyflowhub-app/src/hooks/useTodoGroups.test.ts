import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useTodoGroups } from './useTodoGroups';
import type { TodoItem } from '../types/todo';

function createTodo(overrides: Partial<TodoItem> = {}): TodoItem {
  return {
    id: 'note-1:0',
    text: 'task',
    checked: false,
    checkedAt: null,
    noteId: 'note-1',
    lineIndex: 0,
    noteTitle: 'Note one',
    ...overrides,
  };
}

describe('useTodoGroups', () => {
  it('groups todos by note and sorts groups with inbox first', () => {
    const todos = [
      createTodo({ id: 'a:0', noteId: 'a', noteTitle: 'Alpha', text: 'first', lineIndex: 0 }),
      createTodo({ id: 'b:0', noteId: 'b', noteTitle: '待办箱', text: 'inbox task', lineIndex: 0 }),
      createTodo({ id: 'a:1', noteId: 'a', noteTitle: 'Alpha', text: 'second', lineIndex: 1 }),
    ];

    const { result } = renderHook(() => useTodoGroups(todos));

    expect(result.current.groupedTodos.map((group) => group.noteId)).toEqual(['b', 'a']);
    expect(result.current.groupedTodos[1].todos.map((todo) => todo.text)).toEqual(['first', 'second']);
  });

  it('splits pending and completed groups without dropping empty groups from either side', () => {
    const todos = [
      createTodo({ id: 'a:0', noteId: 'a', noteTitle: 'Alpha', text: 'open', checked: false, lineIndex: 0 }),
      createTodo({
        id: 'a:1',
        noteId: 'a',
        noteTitle: 'Alpha',
        text: 'done',
        checked: true,
        checkedAt: '2026-10-20T00:00:00.000Z',
        lineIndex: 1,
      }),
      createTodo({ id: 'c:0', noteId: 'c', noteTitle: 'Only done note', text: 'done only', checked: true, checkedAt: '2026-10-20T00:00:00.000Z', lineIndex: 0 }),
    ];

    const { result } = renderHook(() => useTodoGroups(todos));

    // 只有含未完成项的笔记出现在 pendingGroups；纯已完成的笔记组只出现在 completedGroups。
    expect(result.current.pendingGroups.map((group) => group.noteId)).toEqual(['a']);
    expect(result.current.pendingGroups.flatMap((group) => group.todos).map((todo) => todo.text)).toEqual(['open']);
    expect(result.current.completedGroups.map((group) => group.noteId)).toEqual(['a', 'c']);
    expect(result.current.completedGroups.flatMap((group) => group.todos).map((todo) => todo.text)).toEqual(['done', 'done only']);
  });
});
