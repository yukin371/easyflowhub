import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TodoPanel } from './TodoPanel';
import { createTodoCardWindow, getNote, listNotes, saveNote } from '../../../lib/tauri/notes';
import { getSettings } from '../../../lib/tauri/settings';
import type { Note } from '../../../types/note';

vi.mock('../../../lib/tauri/notes', () => ({
  listNotes: vi.fn(),
  saveNote: vi.fn(),
  getNote: vi.fn(),
  createTodoCardWindow: vi.fn(),
}));

vi.mock('../../../lib/tauri/settings', () => ({
  getSettings: vi.fn(),
}));

const listNotesMock = vi.mocked(listNotes);
const saveNoteMock = vi.mocked(saveNote);
const getNoteMock = vi.mocked(getNote);
const createTodoCardWindowMock = vi.mocked(createTodoCardWindow);
const getSettingsMock = vi.mocked(getSettings);

function createFixtureNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'note-1',
    title: 'Source note',
    content: '- [ ] Ship panel test',
    tags: '',
    created_at: '2026-04-24T00:00:00.000Z',
    updated_at: '2026-04-24T00:00:00.000Z',
    is_pinned: false,
    deleted_at: null,
    ...overrides,
  };
}

describe('TodoPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listNotesMock.mockResolvedValue([createFixtureNote()]);
    getNoteMock.mockResolvedValue(createFixtureNote());
    saveNoteMock.mockResolvedValue(createFixtureNote());
    createTodoCardWindowMock.mockResolvedValue('todo-card-1');
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
  });

  it('勾选待办时保存完成状态并刷新列表', async () => {
    render(<TodoPanel />);

    const todoText = await screen.findByText('Ship panel test');
    const row = todoText.closest('li');
    const toggleButton = row?.querySelector('button');

    expect(toggleButton).toBeTruthy();
    fireEvent.click(toggleButton as HTMLButtonElement);

    await waitFor(() => {
      expect(saveNoteMock).toHaveBeenCalledWith({
        id: 'note-1',
        content: expect.stringMatching(/\[x\] Ship panel test @done:/),
      });
    });
  });

  it('点击来源标题时导航到原笔记', async () => {
    const onNavigateToNote = vi.fn();
    render(<TodoPanel onNavigateToNote={onNavigateToNote} />);

    const sourceButton = await screen.findByRole('button', { name: /Source note/i });
    fireEvent.click(sourceButton);

    expect(onNavigateToNote).toHaveBeenCalledWith('note-1');
  });

  it('点击悬浮卡片按钮时创建 todo card 窗口', async () => {
    render(<TodoPanel />);

    fireEvent.click(await screen.findByRole('button', { name: '悬浮卡片' }));

    await waitFor(() => {
      expect(createTodoCardWindowMock).toHaveBeenCalledTimes(1);
    });
  });
});
