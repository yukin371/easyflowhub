import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TodoCardPage } from './TodoCardPage';
import { createNote, getNote, listNotes, saveNote } from '../lib/tauri/notes';
import { getSettings } from '../lib/tauri/settings';
import { invoke } from '@tauri-apps/api/core';
import { emitTo } from '@tauri-apps/api/event';
import { MANAGER_NAVIGATE_TO_NOTE_EVENT } from '../components/manager/ManagerPage';
import type { Note } from '../types/note';
import { TODO_INBOX_TITLE } from '../lib/todoParser';

const closeMock = vi.hoisted(() => vi.fn());

vi.mock('../lib/tauri/notes', () => ({
  createNote: vi.fn(),
  getNote: vi.fn(),
  listNotes: vi.fn(),
  saveNote: vi.fn(),
}));

vi.mock('../lib/tauri/settings', () => ({
  getSettings: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  emitTo: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    label: 'todo-card-1',
    close: closeMock,
    isFocused: vi.fn(async () => true),
    onFocusChanged: vi.fn(async () => () => {}),
    setAlwaysOnTop: vi.fn(async () => undefined),
  })),
}));

const listNotesMock = vi.mocked(listNotes);
const getNoteMock = vi.mocked(getNote);
const saveNoteMock = vi.mocked(saveNote);
const createNoteMock = vi.mocked(createNote);
const getSettingsMock = vi.mocked(getSettings);
const invokeMock = vi.mocked(invoke);
const emitToMock = vi.mocked(emitTo);

function createFixtureNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'note-1',
    title: 'Source note',
    content: '- [ ] Ship docs',
    tags: '',
    created_at: '2026-04-24T00:00:00.000Z',
    updated_at: '2026-04-24T00:00:00.000Z',
    is_pinned: false,
    deleted_at: null,
    ...overrides,
  };
}

describe('TodoCardPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    listNotesMock.mockResolvedValue([createFixtureNote()]);
    getNoteMock.mockResolvedValue(createFixtureNote());
    saveNoteMock.mockResolvedValue(createFixtureNote());
    createNoteMock.mockResolvedValue(
      createFixtureNote({ id: 'new-note', title: '', content: '' })
    );
    getSettingsMock.mockResolvedValue({
      quick_note: { width: 400, height: 300 },
      trash: { retention_days: 30 },
      editor: {
        undo_steps: 100,
        cursor_style: 'accent',
        cursor_color: '#4f5a43',
        cursor_trail: true,
      },
      todo: { done_retention_hours: 24 },
    });
    invokeMock.mockResolvedValue(undefined as never);
    emitToMock.mockResolvedValue(undefined as never);
    closeMock.mockResolvedValue(undefined);
  });

  it('勾选待办时保存带完成标记的内容', async () => {
    render(<TodoCardPage />);

    const todoText = await screen.findByText('Ship docs');
    const row = todoText.closest('div[style]')?.parentElement;
    const toggleButton = row?.querySelector('button');

    expect(toggleButton).toBeTruthy();

    fireEvent.click(toggleButton as HTMLButtonElement);

    await waitFor(() => {
      expect(saveNoteMock).toHaveBeenCalledWith({
        id: 'note-1',
        content: expect.stringMatching(/\[x\] Ship docs @done:/),
      });
    });
  });

  it('从待办卡片打开原笔记时通知 manager 导航', async () => {
    render(<TodoCardPage />);

    await screen.findByText('Ship docs');
    fireEvent.click(screen.getByTitle('打开原笔记'));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('show_manager_window');
    });
    await waitFor(() => {
      expect(emitToMock).toHaveBeenCalledWith('manager', MANAGER_NAVIGATE_TO_NOTE_EVENT, {
        noteId: 'note-1',
      });
    });
  });

  it('新增待办时会创建待办箱并保存新任务', async () => {
    listNotesMock.mockResolvedValue([]);
    saveNoteMock
      .mockResolvedValueOnce(
        createFixtureNote({ id: 'new-note', title: TODO_INBOX_TITLE, content: '' })
      )
      .mockResolvedValueOnce(
        createFixtureNote({
          id: 'new-note',
          title: TODO_INBOX_TITLE,
          content: '- [ ] Write release note',
        })
      );

    render(<TodoCardPage />);

    fireEvent.click(await screen.findByTitle('新增待办'));
    const input = await screen.findByPlaceholderText('...');
    fireEvent.change(input, { target: { value: 'Write release note' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => {
      expect(createNoteMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(saveNoteMock).toHaveBeenNthCalledWith(1, {
        id: 'new-note',
        title: TODO_INBOX_TITLE,
        content: '',
        tags: '',
      });
    });
    await waitFor(() => {
      expect(saveNoteMock).toHaveBeenNthCalledWith(2, {
        id: 'new-note',
        title: TODO_INBOX_TITLE,
        content: '- [ ] Write release note',
        tags: '',
      });
    });
  });
});
