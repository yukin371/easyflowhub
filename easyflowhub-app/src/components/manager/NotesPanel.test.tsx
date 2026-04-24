import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { NotesPanel } from './NotesPanel';
import { createNote, getNote, listNotes, saveNote, togglePinNote, trashNote, trashNotesBatch } from '../../lib/tauri/notes';
import { MANAGER_OPEN_NOTE_EVENT } from './ManagerPage';
import type { Note } from '../../types/note';
import { DEFAULT_SHORTCUT_CONFIG } from '../../types/shortcut';

const flushSaveMock = vi.hoisted(() => vi.fn());
const scheduleSaveMock = vi.hoisted(() => vi.fn());
const resetSaveFeedbackMock = vi.hoisted(() => vi.fn());
const clearBackupDraftMock = vi.hoisted(() => vi.fn());
const inspectBackupMock = vi.hoisted(() => vi.fn());

vi.mock('../../lib/tauri/notes', () => ({
  createNote: vi.fn(),
  trashNote: vi.fn(),
  trashNotesBatch: vi.fn(),
  listNotes: vi.fn(),
  saveNote: vi.fn(),
  togglePinNote: vi.fn(),
  getNote: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    onFocusChanged: vi.fn(async () => () => {}),
  })),
}));

vi.mock('../../hooks/useEditorPreferences', () => ({
  useEditorPreferences: () => ({
    shortcutConfig: DEFAULT_SHORTCUT_CONFIG,
    editorSettings: {
      undo_steps: 100,
      cursor_style: 'accent',
      cursor_color: '#4f5a43',
      cursor_trail: true,
    },
    refreshEditorPreferences: vi.fn(async () => undefined),
  }),
}));

vi.mock('../../hooks/useShortcutEngine', () => ({
  useShortcutEngine: vi.fn(),
}));

vi.mock('../../hooks/useAutoSave', () => ({
  useAutoSave: () => ({
    saveState: 'saved',
    saveError: null,
    scheduleSave: scheduleSaveMock,
    flushSave: flushSaveMock,
    resetSaveFeedback: resetSaveFeedbackMock,
  }),
}));

vi.mock('../../hooks/useBackupRestore', () => ({
  useBackupRestore: () => ({
    backupDraft: null,
    inspectBackup: inspectBackupMock,
    clearBackupDraft: clearBackupDraftMock,
    restoreBackupDraft: vi.fn(),
    mergeBackupDraft: vi.fn(),
  }),
}));

vi.mock('../../hooks/useEditorImageInsertion', () => ({
  useEditorImageInsertion: () => ({
    isDragActive: false,
    handlePaste: vi.fn(),
    handleDragEnter: vi.fn(),
    handleDragLeave: vi.fn(),
    handleDragOver: vi.fn(),
    handleDrop: vi.fn(),
  }),
}));

vi.mock('./notes/filters/TagFilterBar', () => ({
  TagFilterBar: () => <div>TagFilterBar</div>,
}));

vi.mock('./notes/views/ListView', () => ({
  ListView: () => <div>ListView</div>,
}));

vi.mock('./notes/views/GridView', () => ({
  GridView: ({ notes }: { notes: Note[] }) => (
    <div>
      {notes.map((note) => (
        <span key={note.id}>{note.title || note.id}</span>
      ))}
    </div>
  ),
}));

vi.mock('./notes/views/TimelineView', () => ({
  TimelineView: () => <div>TimelineView</div>,
}));

vi.mock('./notes/NoteEditor', () => ({
  NoteEditor: ({
    note,
    draftContent,
    onClose,
  }: {
    note: Note;
    draftContent: string;
    onClose: () => Promise<void>;
  }) => (
    <div>
      <div>Editing: {note.id}</div>
      <div data-testid="editor-content">{draftContent}</div>
      <button onClick={() => void onClose()}>Close editor</button>
    </div>
  ),
}));

const listNotesMock = vi.mocked(listNotes);
const getNoteMock = vi.mocked(getNote);
const createNoteMock = vi.mocked(createNote);
const saveNoteMock = vi.mocked(saveNote);
const togglePinNoteMock = vi.mocked(togglePinNote);
const trashNoteMock = vi.mocked(trashNote);
const trashNotesBatchMock = vi.mocked(trashNotesBatch);

function createFixtureNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'note-1',
    title: 'Existing note',
    content: 'hello',
    tags: 'alpha',
    created_at: '2026-04-24T00:00:00.000Z',
    updated_at: '2026-04-24T00:00:00.000Z',
    is_pinned: false,
    deleted_at: null,
    ...overrides,
  };
}

describe('NotesPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    listNotesMock.mockResolvedValue([createFixtureNote()]);
    getNoteMock.mockResolvedValue(null);
    createNoteMock.mockResolvedValue(createFixtureNote({ id: 'new-note' }));
    saveNoteMock.mockResolvedValue(createFixtureNote());
    togglePinNoteMock.mockResolvedValue(createFixtureNote({ is_pinned: true }));
    trashNoteMock.mockResolvedValue('note-1' as never);
    trashNotesBatchMock.mockResolvedValue('note-1' as never);
    flushSaveMock.mockResolvedValue(undefined);
    scheduleSaveMock.mockImplementation(() => undefined);
    resetSaveFeedbackMock.mockImplementation(() => undefined);
    clearBackupDraftMock.mockImplementation(() => undefined);
    inspectBackupMock.mockImplementation(() => undefined);
  });

  it('收到 open-note 事件时加载并打开指定笔记', async () => {
    getNoteMock.mockResolvedValueOnce(
      createFixtureNote({
        id: 'target-note',
        title: 'Target note',
        content: 'from event',
        tags: 'beta',
      })
    );

    render(<NotesPanel />);
    await screen.findByText('Existing note');

    window.dispatchEvent(
      new CustomEvent(MANAGER_OPEN_NOTE_EVENT, {
        detail: { noteId: 'target-note' },
      })
    );

    await waitFor(() => {
      expect(getNoteMock).toHaveBeenCalledWith('target-note');
    });
    await waitFor(() => {
      expect(screen.getByText('Editing: target-note')).toBeInTheDocument();
    });
  });

  it('关闭编辑器时先 flush 当前草稿再退出编辑态', async () => {
    getNoteMock.mockResolvedValueOnce(
      createFixtureNote({
        id: 'target-note',
        title: 'Target note',
        content: 'draft content',
        tags: 'beta',
      })
    );

    render(<NotesPanel />);
    await screen.findByText('Existing note');

    window.dispatchEvent(
      new CustomEvent(MANAGER_OPEN_NOTE_EVENT, {
        detail: { noteId: 'target-note' },
      })
    );

    await screen.findByText('Editing: target-note');
    fireEvent.click(screen.getByRole('button', { name: 'Close editor' }));

    await waitFor(() => {
      expect(flushSaveMock).toHaveBeenCalledWith({
        noteId: 'target-note',
        title: 'Target note',
        content: 'draft content',
        tags: 'beta',
      });
    });
    await waitFor(() => {
      expect(screen.queryByText('Editing: target-note')).not.toBeInTheDocument();
    });
  });

  it('打开包含图片的笔记时保留内联 markdown 内容用于原位编辑', async () => {
    getNoteMock.mockResolvedValueOnce(
      createFixtureNote({
        id: 'image-note',
        title: 'Image note',
        content: 'hello\n\n![tmp268](asset:65097bcc-1144-430c-871d-5f6b1443a149.png)',
        tags: 'beta',
      })
    );

    render(<NotesPanel />);
    await screen.findByText('Existing note');

    window.dispatchEvent(
      new CustomEvent(MANAGER_OPEN_NOTE_EVENT, {
        detail: { noteId: 'image-note' },
      })
    );

    await screen.findByText('Editing: image-note');
    expect(screen.getByTestId('editor-content').textContent).toContain(
      '![tmp268](asset:65097bcc-1144-430c-871d-5f6b1443a149.png)'
    );

    fireEvent.click(screen.getByRole('button', { name: 'Close editor' }));

    await waitFor(() => {
      expect(flushSaveMock).toHaveBeenCalledWith({
        noteId: 'image-note',
        title: 'Image note',
        content: 'hello\n\n![tmp268](asset:65097bcc-1144-430c-871d-5f6b1443a149.png)',
        tags: 'beta',
      });
    });
  });
});
