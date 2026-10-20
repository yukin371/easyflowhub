import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TrashPanel } from './TrashPanel';
import {
  emptyTrash,
  listTrash,
  permanentDeleteNote,
  restoreNote,
  restoreNotesBatch,
} from '../../lib/tauri/notes';
import type { Note } from '../../types/note';
import { scriptTrashApi } from '../../lib/api/scriptmgr';
import { emptyInboxTrash, listInboxTrash, permanentlyDeleteInboxItem, restoreInboxItemFromTrash } from '../../lib/tauri/inbox';
import type { InboxItem } from '../../types/inbox';

vi.mock('../../lib/tauri/notes', () => ({
  listTrash: vi.fn(),
  restoreNote: vi.fn(),
  restoreNotesBatch: vi.fn(),
  permanentDeleteNote: vi.fn(),
  emptyTrash: vi.fn(),
}));

vi.mock('../../lib/api/scriptmgr', () => ({
  scriptTrashApi: {
    list: vi.fn(),
    restore: vi.fn(),
    purge: vi.fn(),
    empty: vi.fn(),
  },
}));

vi.mock('../../lib/tauri/inbox', () => ({
  emptyInboxTrash: vi.fn(),
  listInboxTrash: vi.fn(),
  permanentlyDeleteInboxItem: vi.fn(),
  restoreInboxItemFromTrash: vi.fn(),
}));

const listScriptTrashMock = vi.mocked(scriptTrashApi.list);
const restoreScriptMock = vi.mocked(scriptTrashApi.restore);
const purgeScriptMock = vi.mocked(scriptTrashApi.purge);
const listTrashMock = vi.mocked(listTrash);
const restoreNoteMock = vi.mocked(restoreNote);
const restoreNotesBatchMock = vi.mocked(restoreNotesBatch);
const permanentDeleteNoteMock = vi.mocked(permanentDeleteNote);
const emptyTrashMock = vi.mocked(emptyTrash);
const listInboxTrashMock = vi.mocked(listInboxTrash);
const restoreInboxItemMock = vi.mocked(restoreInboxItemFromTrash);
const permanentlyDeleteInboxItemMock = vi.mocked(permanentlyDeleteInboxItem);
const emptyInboxTrashMock = vi.mocked(emptyInboxTrash);

function createFixtureNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'note-1',
    title: 'Deleted note',
    content: 'trashed content',
    tags: '',
    created_at: '2026-04-24T00:00:00.000Z',
    updated_at: '2026-04-24T00:00:00.000Z',
    is_pinned: false,
    deleted_at: '2026-04-24T00:00:00.000Z',
    ...overrides,
  };
}

function createFixtureInboxItem(overrides: Partial<InboxItem> = {}): InboxItem {
  return {
    id: 'inbox-1',
    kind: 'image',
    title: 'Screenshot',
    content: '',
    source: 'manager',
    context: 'clipboard',
    capturedAt: '2026-10-19T00:00:00.000Z',
    archivedAt: null,
    deletedAt: '2026-10-20T00:00:00.000Z',
    convertedNoteId: null,
    attachments: [{
      id: 'attachment-1',
      itemId: 'inbox-1',
      kind: 'image',
      name: 'screen.png',
      mimeType: 'image/png',
      filename: 'stored.png',
      sizeBytes: 2048,
      createdAt: '2026-10-19T00:00:00.000Z',
    }],
    ...overrides,
  };
}

describe('TrashPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listTrashMock.mockResolvedValue([
      createFixtureNote({ id: 'note-1', title: 'Deleted note 1' }),
      createFixtureNote({ id: 'note-2', title: 'Deleted note 2' }),
    ]);
    restoreNoteMock.mockResolvedValue('note-1' as never);
    restoreNotesBatchMock.mockResolvedValue('note-1,note-2' as never);
    permanentDeleteNoteMock.mockResolvedValue('note-1' as never);
    emptyTrashMock.mockResolvedValue(2 as never);
    listScriptTrashMock.mockResolvedValue([]);
    listInboxTrashMock.mockResolvedValue([]);
    restoreInboxItemMock.mockResolvedValue('inbox-1' as never);
    permanentlyDeleteInboxItemMock.mockResolvedValue('inbox-1' as never);
    emptyInboxTrashMock.mockResolvedValue(0);
  });

  it('恢复单条笔记后将其从回收站列表移除', async () => {
    render(<TrashPanel />);

    await screen.findByText('Deleted note 1');
    const restoreButtons = screen.getAllByRole('button', { name: '恢复' });
    fireEvent.click(restoreButtons[0]);

    await waitFor(() => {
      expect(restoreNoteMock).toHaveBeenCalledWith('note-1');
    });
    await waitFor(() => {
      expect(screen.queryByText('Deleted note 1')).not.toBeInTheDocument();
    });
  });

  it('恢复多选笔记时调用批量恢复接口', async () => {
    render(<TrashPanel />);

    await screen.findByText('Deleted note 1');
    const checkboxes = screen.getAllByRole('checkbox');
    fireEvent.click(checkboxes[0]);
    fireEvent.click(checkboxes[1]);

    fireEvent.click(await screen.findByRole('button', { name: '恢复选中 (2)' }));

    await waitFor(() => {
      expect(restoreNotesBatchMock).toHaveBeenCalledWith(['note-1', 'note-2']);
    });
    await waitFor(() => {
      expect(screen.queryByText('Deleted note 1')).not.toBeInTheDocument();
      expect(screen.queryByText('Deleted note 2')).not.toBeInTheDocument();
    });
  });

  it('清空回收站后显示空态', async () => {
    render(<TrashPanel />);

    fireEvent.click(await screen.findByRole('button', { name: '清空回收站' }));

    await waitFor(() => {
      expect(emptyTrashMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.getAllByText('回收站为空').length).toBeGreaterThan(0);
    });
  });

  it('脚本分区展示已删除脚本并可恢复到原路径', async () => {
    listScriptTrashMock.mockResolvedValue([
      {
        id: 'trash-1',
        name: 'Demo Script',
        original_path: 'E:/scripts/demo.ps1',
        script_type: 'powershell',
        deleted_at: '2026-10-19T00:00:00Z',
        size_bytes: 2048,
      },
    ]);
    restoreScriptMock.mockResolvedValue({
      id: 'trash-1',
      name: 'Demo Script',
      original_path: 'E:/scripts/demo.ps1',
      script_type: 'powershell',
      deleted_at: '2026-10-19T00:00:00Z',
      size_bytes: 2048,
    });

    render(<TrashPanel />);
    fireEvent.click(await screen.findByRole('tab', { name: /脚本/ }));

    expect(await screen.findByText('Demo Script')).toBeInTheDocument();
    expect(screen.getByText('E:/scripts/demo.ps1')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '恢复' }));

    await waitFor(() => expect(restoreScriptMock).toHaveBeenCalledWith('trash-1'));
    await waitFor(() => expect(screen.queryByText('Demo Script')).not.toBeInTheDocument());
  });

  it('彻底删除脚本前需要确认', async () => {
    listScriptTrashMock.mockResolvedValue([
      {
        id: 'trash-2',
        name: 'Broken Script',
        original_path: 'E:/scripts/broken.py',
        script_type: 'python',
        deleted_at: '2026-10-19T00:00:00Z',
        size_bytes: 100,
      },
    ]);
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);

    render(<TrashPanel />);
    fireEvent.click(await screen.findByRole('tab', { name: /脚本/ }));
    expect(await screen.findByText('Broken Script')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '永久删除' }));

    await waitFor(() => expect(purgeScriptMock).toHaveBeenCalledWith('trash-2'));
    expect(confirmSpy).toHaveBeenCalledTimes(1);
    confirmSpy.mockRestore();
  });

  it('收集箱分区展示图片并可恢复到收集箱', async () => {
    listInboxTrashMock.mockResolvedValue([createFixtureInboxItem()]);
    render(<TrashPanel />);
    fireEvent.click(await screen.findByRole('tab', { name: /收集箱/ }));

    expect(await screen.findByText('Screenshot')).toBeInTheDocument();
    expect(screen.getByText('附件：screen.png')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '恢复' }));

    await waitFor(() => expect(restoreInboxItemMock).toHaveBeenCalledWith('inbox-1'));
    await waitFor(() => expect(screen.queryByText('Screenshot')).not.toBeInTheDocument());
  });

  it('永久清理收集箱项目需要确认', async () => {
    listInboxTrashMock.mockResolvedValue([createFixtureInboxItem({ kind: 'audio', title: 'Meeting audio' })]);
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<TrashPanel />);
    fireEvent.click(await screen.findByRole('tab', { name: /收集箱/ }));
    expect(await screen.findByText('Meeting audio')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '永久删除' }));

    await waitFor(() => expect(permanentlyDeleteInboxItemMock).toHaveBeenCalledWith('inbox-1'));
    expect(confirmSpy).toHaveBeenCalledTimes(1);
    confirmSpy.mockRestore();
  });

  it('清空收集箱回收站前确认并刷新为空态', async () => {
    listInboxTrashMock.mockResolvedValue([createFixtureInboxItem()]);
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<TrashPanel />);
    fireEvent.click(await screen.findByRole('tab', { name: /收集箱/ }));
    await screen.findByText('Screenshot');

    fireEvent.click(screen.getByRole('button', { name: '清空收集箱回收站' }));

    await waitFor(() => expect(emptyInboxTrashMock).toHaveBeenCalledTimes(1));
    expect(confirmSpy).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getAllByText('没有已删除的收集项')).toHaveLength(2));
    confirmSpy.mockRestore();
  });
});
