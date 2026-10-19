import { describe, expect, it, vi } from 'vitest';
import {
  appendTodoLine,
  extractAllTodos,
  extractTodos,
  locateTodoLine,
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

  // F05: 多窗口插入/删除行后，快照里的 lineIndex 已过期；定位必须按文本
  // 身份重定位，同名待办多于一条时拒绝而不是猜。
  describe('locateTodoLine (F05)', () => {
    it('keeps the snapshot line when it still points at the same todo', () => {
      const content = '- [ ] alpha\n- [ ] beta';
      expect(locateTodoLine(content, { lineIndex: 1, text: 'beta' })).toEqual({
        kind: 'exact',
        lineIndex: 1,
      });
    });

    it('relocates by text when another window inserted lines above', () => {
      // 快照：A 在第 0 行。另一窗口在顶部插入 inserted 后，A 挪到第 1 行。
      const content = '- [ ] inserted\n- [ ] alpha\n- [ ] beta';
      expect(locateTodoLine(content, { lineIndex: 0, text: 'alpha' })).toEqual({
        kind: 'relocated',
        lineIndex: 1,
      });
    });

    it('relocates by text when lines above were deleted', () => {
      const content = '- [ ] alpha';
      expect(locateTodoLine(content, { lineIndex: 2, text: 'alpha' })).toEqual({
        kind: 'relocated',
        lineIndex: 0,
      });
    });

    it('ignores the @done suffix when matching text', () => {
      const content = '- [x] alpha @done:2026-04-05T12:00:00.000Z';
      expect(locateTodoLine(content, { lineIndex: 3, text: 'alpha' })).toEqual({
        kind: 'relocated',
        lineIndex: 0,
      });
    });

    it('reports conflict for duplicate todos instead of guessing', () => {
      const content = '- [ ] inserted\n- [ ] alpha\n- [ ] alpha';
      expect(locateTodoLine(content, { lineIndex: 0, text: 'alpha' })).toEqual({
        kind: 'conflict',
      });
    });

    it('still reports conflict when the stale line matches but duplicates exist', () => {
      // 快照行号仍指向一条同名待办，但无法证明它没被顶替成另一条同名项。
      const content = '- [ ] alpha\n- [ ] alpha';
      expect(locateTodoLine(content, { lineIndex: 0, text: 'alpha' })).toEqual({
        kind: 'conflict',
      });
    });

    it('reports missing when the todo was deleted elsewhere', () => {
      const content = '- [ ] beta';
      expect(locateTodoLine(content, { lineIndex: 0, text: 'alpha' })).toEqual({
        kind: 'missing',
      });
    });

    it('does not match non-checkbox lines with the same text', () => {
      const content = 'alpha\n- [ ] beta';
      expect(locateTodoLine(content, { lineIndex: 0, text: 'alpha' })).toEqual({
        kind: 'missing',
      });
    });
  });

  describe('appendTodoLine', () => {
    it('creates the first line from empty content', () => {
      expect(appendTodoLine('', ' first ')).toBe('- [ ] first');
    });

    it('appends after trimming trailing whitespace', () => {
      expect(appendTodoLine('- [ ] a\n\n', 'b')).toBe('- [ ] a\n- [ ] b');
    });

    it('leaves content untouched for blank text', () => {
      expect(appendTodoLine('- [ ] a', '   ')).toBe('- [ ] a');
    });
  });
});
