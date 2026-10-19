import '@testing-library/jest-dom/vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useTodos } from './useTodos';
import * as notesApi from '../lib/tauri/notes';
import * as settingsApi from '../lib/tauri/settings';
import type { Note } from '../types/note';

vi.mock('../lib/tauri/notes', () => ({
  listNotes: vi.fn(),
  getNote: vi.fn(),
  saveNote: vi.fn(),
  createNote: vi.fn(),
}));

vi.mock('../lib/tauri/settings', () => ({
  getSettings: vi.fn(),
}));

function createNoteFixture(overrides: Partial<Note> = {}): Note {
  return {
    id: 'note-1',
    title: 'Source note',
    content: '- [ ] alpha\n- [ ] beta',
    tags: '',
    created_at: '2026-10-19T00:00:00.000Z',
    updated_at: '2026-10-19T00:00:00.000Z',
    is_pinned: false,
    deleted_at: null,
    ...overrides,
  };
}

describe('useTodos (F05)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(settingsApi.getSettings).mockResolvedValue({
      quick_note: { width: 400, height: 300 },
      trash: { retention_days: 30 },
      editor: {
        undo_steps: 100,
        cursor_style: 'accent',
        cursor_color: '#4f5a43',
        cursor_trail: true,
      },
      todo: { done_retention_hours: 24 },
      translation: { target_language: 'zh-CN', model: '' },
      transcription: { model: '', language: '' },
      developer: { debug_mode: false },
      launcher_trigger: {
        enabled: true,
        trigger: 'keyboard_or_mouse_middle',
        keyboard_shortcut: 'Ctrl+Alt+Space',
        excluded_apps: [],
      },
    } as never);
  });

  it('relocates by text when another window inserted lines above', async () => {
    // 快照列表：alpha 在第 0 行。
    vi.mocked(notesApi.listNotes).mockResolvedValue([createNoteFixture()]);
    // 用户点击前，另一窗口在顶部插入了 inserted：alpha 现在在第 1 行。
    vi.mocked(notesApi.getNote).mockResolvedValue(
      createNoteFixture({ content: '- [ ] inserted\n- [ ] alpha\n- [ ] beta' }),
    );
    vi.mocked(notesApi.saveNote).mockResolvedValue(createNoteFixture());

    const { result } = renderHook(() => useTodos());
    await waitFor(() => expect(result.current.todos).toHaveLength(2));

    const staleAlpha = result.current.todos.find((todo) => todo.text === 'alpha');
    // 快照行号是 0，但最新正文里 alpha 在第 1 行。
    expect(staleAlpha?.lineIndex).toBe(0);

    await act(async () => {
      await result.current.toggleTodo(staleAlpha!);
    });

    // 必须勾选 alpha（第 1 行），而不是把 inserted（第 0 行）改错。
    expect(notesApi.saveNote).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'note-1',
        expected_updated_at: '2026-10-19T00:00:00.000Z',
      }),
    );
    const saved = vi.mocked(notesApi.saveNote).mock.calls[0][0];
    const savedContent = saved.content ?? '';
    const savedLines = savedContent.split('\n');
    // inserted 保持未勾选，alpha 被勾选并带 @done 时间戳。
    expect(savedLines[0]).toBe('- [ ] inserted');
    expect(savedLines[1]).toMatch(/^- \[x\] alpha @done:/);
    expect(savedContent).not.toContain('- [x] inserted');
  });

  it('refuses to write when duplicate todos make the target ambiguous', async () => {
    vi.mocked(notesApi.listNotes).mockResolvedValue([
      createNoteFixture({ content: '- [ ] alpha\n- [ ] alpha' }),
    ]);
    vi.mocked(notesApi.getNote).mockResolvedValue(
      createNoteFixture({ content: '- [ ] alpha\n- [ ] alpha' }),
    );

    const { result } = renderHook(() => useTodos());
    await waitFor(() => expect(result.current.todos).toHaveLength(2));

    let outcome: { ok: boolean; failure?: string } | undefined;
    await act(async () => {
      outcome = await result.current.toggleTodo(result.current.todos[0]);
    });

    expect(outcome?.ok).toBe(false);
    expect(outcome?.failure).toBe('conflict');
    // 冲突时绝不能写入。
    expect(notesApi.saveNote).not.toHaveBeenCalled();
  });

  it('surfaces a stale-write conflict when native CAS rejects the save', async () => {
    vi.mocked(notesApi.listNotes).mockResolvedValue([createNoteFixture()]);
    vi.mocked(notesApi.getNote).mockResolvedValue(createNoteFixture());
    vi.mocked(notesApi.saveNote).mockRejectedValue(
      new Error('note_conflict: note-1 was modified by another window since it was read'),
    );

    const { result } = renderHook(() => useTodos());
    await waitFor(() => expect(result.current.todos).toHaveLength(2));

    let outcome: { ok: boolean; failure?: string } | undefined;
    await act(async () => {
      outcome = await result.current.toggleTodo(result.current.todos[0]);
    });

    expect(outcome?.ok).toBe(false);
    expect(outcome?.failure).toBe('stale');
    // 冲突后必须重新拉取，让用户看到真实状态。
    expect(notesApi.listNotes).toHaveBeenCalledTimes(2);
  });

  it('recognizes a raw string CAS conflict from Tauri', async () => {
    vi.mocked(notesApi.listNotes).mockResolvedValue([createNoteFixture()]);
    vi.mocked(notesApi.getNote).mockResolvedValue(createNoteFixture());
    vi.mocked(notesApi.saveNote).mockRejectedValue(
      'note_conflict: note-1 was modified by another window since it was read',
    );

    const { result } = renderHook(() => useTodos());
    await waitFor(() => expect(result.current.todos).toHaveLength(2));

    let outcome: { ok: boolean; failure?: string } | undefined;
    await act(async () => {
      outcome = await result.current.toggleTodo(result.current.todos[0]);
    });

    expect(outcome?.ok).toBe(false);
    expect(outcome?.failure).toBe('stale');
    expect(notesApi.listNotes).toHaveBeenCalledTimes(2);
  });

  it('reports missing when the todo was deleted in another window', async () => {
    vi.mocked(notesApi.listNotes).mockResolvedValue([createNoteFixture()]);
    vi.mocked(notesApi.getNote).mockResolvedValue(
      createNoteFixture({ content: '- [ ] beta' }),
    );

    const { result } = renderHook(() => useTodos());
    await waitFor(() => expect(result.current.todos).toHaveLength(2));

    let outcome: { ok: boolean; failure?: string } | undefined;
    await act(async () => {
      outcome = await result.current.toggleTodo(result.current.todos[0]);
    });

    expect(outcome?.ok).toBe(false);
    expect(outcome?.failure).toBe('missing');
    expect(notesApi.saveNote).not.toHaveBeenCalled();
  });

  it('creates a todo through the inbox note with CAS', async () => {
    const inbox = createNoteFixture({ id: 'inbox-1', title: '待办箱', content: '' });
    vi.mocked(notesApi.listNotes).mockResolvedValue([inbox]);
    vi.mocked(notesApi.getNote).mockResolvedValue(inbox);
    vi.mocked(notesApi.saveNote).mockResolvedValue(inbox);

    const { result } = renderHook(() => useTodos());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.createTodo('write tests');
    });

    expect(notesApi.saveNote).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'inbox-1',
        content: '- [ ] write tests',
        expected_updated_at: '2026-10-19T00:00:00.000Z',
      }),
    );
  });

  it('serializes same-context inbox initialization', async () => {
    let inbox: Note | null = null;
    const created = createNoteFixture({ id: 'inbox-1', title: '', content: '' });
    vi.mocked(notesApi.listNotes).mockImplementation(async () => (inbox ? [inbox] : []));
    vi.mocked(notesApi.createNote).mockResolvedValue(created);
    vi.mocked(notesApi.getNote).mockImplementation(async () => inbox);
    vi.mocked(notesApi.saveNote).mockImplementation(async (input) => {
      inbox = {
        ...(inbox ?? created),
        title: input.title ?? inbox?.title ?? '',
        content: input.content ?? inbox?.content ?? '',
        tags: input.tags ?? inbox?.tags ?? '',
      };
      return inbox;
    });

    const { result } = renderHook(() => useTodos());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await Promise.all([
        result.current.createTodo('first'),
        result.current.createTodo('second'),
      ]);
    });

    expect(notesApi.createNote).toHaveBeenCalledTimes(1);
    const finalInbox = inbox as Note | null;
    expect(finalInbox?.content).toContain('first');
    expect(finalInbox?.content).toContain('second');
  });
});
