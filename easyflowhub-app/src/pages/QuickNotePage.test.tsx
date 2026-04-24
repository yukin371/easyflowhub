import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Note } from '../types/note';
import { QuickNotePage } from './QuickNotePage';
import { createNote, getNote, listNotes } from '../lib/tauri/notes';
import { invoke } from '@tauri-apps/api/core';
import { DEFAULT_SHORTCUT_CONFIG } from '../types/shortcut';

const destroyMock = vi.hoisted(() => vi.fn());
const flushSaveMock = vi.hoisted(() => vi.fn());
const scheduleSaveMock = vi.hoisted(() => vi.fn());
const resetSaveFeedbackMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    destroy: destroyMock,
  })),
}));

vi.mock('../lib/tauri/notes', () => ({
  closeAllNoteWindows: vi.fn(),
  createNote: vi.fn(),
  getNote: vi.fn(),
  listNotes: vi.fn(),
  toggleNoteWindowsVisibility: vi.fn(),
}));

vi.mock('../hooks/useAutoSave', () => ({
  useAutoSave: () => ({
    flushSave: flushSaveMock,
    scheduleSave: scheduleSaveMock,
    resetSaveFeedback: resetSaveFeedbackMock,
  }),
}));

vi.mock('../hooks/useEditorPreferences', () => ({
  useEditorPreferences: () => ({
    shortcutConfig: DEFAULT_SHORTCUT_CONFIG,
    editorSettings: {
      undo_steps: 100,
      cursor_style: 'accent',
      cursor_color: '#4f5a43',
      cursor_trail: true,
    },
  }),
}));

vi.mock('../hooks/useShortcutEngine', () => ({
  useShortcutEngine: vi.fn(),
}));

vi.mock('../hooks/useEditorImageInsertion', () => ({
  useEditorImageInsertion: vi.fn(() => ({
    isDragActive: false,
    insertResult: null,
    handlePaste: vi.fn(),
    handleDragOver: vi.fn(),
    handleDragEnter: vi.fn(),
    handleDragLeave: vi.fn(),
    handleDrop: vi.fn(),
  })),
}));

vi.mock('../hooks/useHistory', async () => {
  const React = await import('react');

  return {
    useHistory: (initialValue: string) => {
      const [value, setValue] = React.useState(initialValue);
      return {
        value,
        setValue,
        undo: vi.fn(),
        redo: vi.fn(),
        canUndo: false,
        canRedo: false,
        clear: vi.fn(),
        flush: vi.fn(),
      };
    },
  };
});

vi.mock('../lib/imageAssets', async () => {
  const actual = await vi.importActual<typeof import('../lib/imageAssets')>('../lib/imageAssets');
  return {
    ...actual,
    resolveAssetFilenameToUrl: vi.fn(async (filename: string) => `resolved://${filename}`),
  };
});

const createNoteMock = vi.mocked(createNote);
const getNoteMock = vi.mocked(getNote);
const listNotesMock = vi.mocked(listNotes);
const invokeMock = vi.mocked(invoke);

function createFixtureNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'note-1',
    title: 'Quick draft',
    content: '',
    tags: '',
    created_at: '2026-04-24T00:00:00.000Z',
    updated_at: '2026-04-24T00:00:00.000Z',
    is_pinned: false,
    deleted_at: null,
    ...overrides,
  };
}

describe('QuickNotePage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.history.pushState({}, '', '/?mode=quick');

    createNoteMock.mockResolvedValue(createFixtureNote());
    getNoteMock.mockResolvedValue(null);
    listNotesMock.mockResolvedValue([]);
    destroyMock.mockResolvedValue(undefined);
    flushSaveMock.mockResolvedValue(undefined);
    scheduleSaveMock.mockImplementation(() => undefined);
    resetSaveFeedbackMock.mockImplementation(() => undefined);
    invokeMock.mockResolvedValue(undefined as never);
  });

  it('按下 Escape 时关闭当前 quick note 窗口', async () => {
    render(<QuickNotePage />);

    await screen.findByLabelText('笔记内容');

    fireEvent.keyDown(window, { key: 'Escape' });

    await waitFor(() => {
      expect(destroyMock).toHaveBeenCalledTimes(1);
    });
    expect(invokeMock).not.toHaveBeenCalledWith('close_window');
  });

  it('工具面板关闭当前失败时回退到后端 close_window 命令', async () => {
    destroyMock.mockRejectedValueOnce(new Error('destroy failed'));

    render(<QuickNotePage />);

    await screen.findByLabelText('笔记内容');
    fireEvent.click(screen.getByTitle('工具与快捷键'));
    fireEvent.click(await screen.findByRole('button', { name: '关闭当前' }));

    await waitFor(() => {
      expect(destroyMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('close_window');
    });
  });

  it('加载 legacy 资源路径图片后可以从 quick note 中移除', async () => {
    const legacyNote = createFixtureNote({
      id: 'legacy-note',
      content: 'hello\n\n![legacy](http://asset.localhost/C%3A%5Cfoo%5Cassets%5Clegacy.png)',
      title: '',
    });
    window.history.pushState({}, '', '/?mode=quick&note_id=legacy-note');
    getNoteMock.mockResolvedValueOnce(legacyNote);

    render(<QuickNotePage />);

    await screen.findByAltText('legacy');
    fireEvent.click(screen.getByTitle('移除图片'));

    await waitFor(() => {
      expect(screen.queryByAltText('legacy')).not.toBeInTheDocument();
    });
  });

  it('双击图片时打开放大预览层', async () => {
    const imageNote = createFixtureNote({
      id: 'image-note',
      content: 'hello\n\n![legacy](http://asset.localhost/C%3A%5Cfoo%5Cassets%5Clegacy.png)',
      title: '',
    });
    window.history.pushState({}, '', '/?mode=quick&note_id=image-note');
    getNoteMock.mockResolvedValueOnce(imageNote);

    render(<QuickNotePage />);

    const image = await screen.findByAltText('legacy');
    fireEvent.doubleClick(image);

    expect(await screen.findByRole('dialog', { name: 'legacy' })).toBeInTheDocument();
  });
});
