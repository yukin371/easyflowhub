import { describe, expect, it, vi } from 'vitest';
import {
  extractAllTodos,
  extractTodos,
  toggleTodoInContent,
  updateTodoTextInContent,
} from './todoParser';

describe('todoParser', () => {
  it('extracts ordered todos', () => {
    const todos = extractTodos('1. [ ] first\n2. [x] done @done:2026-04-05T10:00:00Z', 'note-1', 'Test');

    expect(todos).toEqual([
      expect.objectContaining({ text: 'first', checked: false, lineIndex: 0 }),
      expect.objectContaining({ text: 'done', checked: true, lineIndex: 1, checkedAt: '2026-04-05T10:00:00Z' }),
    ]);
  });

  it('uses derived note titles when explicit title is empty', () => {
    const todos = extractAllTodos([
      { id: 'note-1', title: '', content: '# Derived Title\n- [ ] task' },
    ]);

    expect(todos[0]?.noteTitle).toBe('Derived Title');
  });

  it('preserves ordered marker when toggling todo state', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-04-05T12:00:00Z'));

    const updated = toggleTodoInContent('1. [ ] first task', 0);
    expect(updated).toBe('1. [x] first task @done:2026-04-05T12:00:00.000Z');

    vi.useRealTimers();
  });

  it('updates todo text without losing @done metadata', () => {
    const updated = updateTodoTextInContent(
      '1. [x] first task @done:2026-04-05T12:00:00.000Z',
      0,
      'renamed task'
    );

    expect(updated).toBe('1. [x] renamed task @done:2026-04-05T12:00:00.000Z');
  });
});
