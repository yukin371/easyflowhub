import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { open } from '@tauri-apps/plugin-dialog';
import { InboxPanel } from './InboxPanel';
import { archiveInboxItem, captureInboxItem, convertInboxItemToNote, deleteInboxItem, listInboxItems, openInboxAssetsDir, openInboxAttachment, openInboxLink, restoreInboxItem } from '../../../lib/tauri/inbox';
import { getTranscript, transcribeAudio } from '../../../lib/tauri/transcription';
import { getSettings } from '../../../lib/tauri/settings';
import { copyTextToClipboard } from '../../../lib/tauri/clipboard';
import type { InboxItem } from '../../../types/inbox';

vi.mock('../../../lib/tauri/inbox', () => ({
  archiveInboxItem: vi.fn(),
  captureInboxItem: vi.fn(),
  convertInboxItemToNote: vi.fn(),
  deleteInboxItem: vi.fn(),
  listInboxItems: vi.fn(),
  openInboxAssetsDir: vi.fn(),
  openInboxAttachment: vi.fn(),
  openInboxLink: vi.fn(),
  restoreInboxItem: vi.fn(),
}));

vi.mock('../../../lib/tauri/transcription', () => ({
  getTranscript: vi.fn(),
  transcribeAudio: vi.fn(),
}));

vi.mock('../../../lib/tauri/settings', () => ({
  getSettings: vi.fn(),
}));

vi.mock('../../../lib/tauri/clipboard', () => ({
  copyTextToClipboard: vi.fn(),
}));

vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent: vi.fn(async () => () => undefined) }),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const listInboxItemsMock = vi.mocked(listInboxItems);
const captureInboxItemMock = vi.mocked(captureInboxItem);
const archiveInboxItemMock = vi.mocked(archiveInboxItem);
const restoreInboxItemMock = vi.mocked(restoreInboxItem);
const convertInboxItemToNoteMock = vi.mocked(convertInboxItemToNote);
const deleteInboxItemMock = vi.mocked(deleteInboxItem);
const openInboxAssetsDirMock = vi.mocked(openInboxAssetsDir);
const openDialogMock = vi.mocked(open);
const transcribeAudioMock = vi.mocked(transcribeAudio);
const getTranscriptMock = vi.mocked(getTranscript);

function fixture(overrides: Partial<InboxItem> = {}): InboxItem {
  return {
    id: 'item-1',
    kind: 'text',
    title: 'Captured thought',
    content: 'Review this later',
    source: 'launcher',
    context: 'clipboard',
    capturedAt: '2026-10-17T10:00:00.000Z',
    archivedAt: null,
    deletedAt: null,
    convertedNoteId: null,
    attachments: [],
    ...overrides,
  };
}

describe('InboxPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listInboxItemsMock.mockResolvedValue([]);
    captureInboxItemMock.mockResolvedValue(fixture());
    archiveInboxItemMock.mockResolvedValue('item-1');
    restoreInboxItemMock.mockResolvedValue('item-1');
    convertInboxItemToNoteMock.mockResolvedValue('note-1');
    deleteInboxItemMock.mockResolvedValue('item-1');
    openInboxAssetsDirMock.mockResolvedValue(undefined);
    openDialogMock.mockResolvedValue(null);
    getTranscriptMock.mockResolvedValue(null);
    transcribeAudioMock.mockResolvedValue({
      text: '会议要点：先确认范围，再排期。',
      model: 'whisper-1',
      attachmentId: 'transcript-1',
      audioSizeBytes: 96_000,
      durationMs: 2_300,
    });
    vi.mocked(getSettings).mockResolvedValue({
      quick_note: { width: 400, height: 300 },
      trash: { retention_days: 30 },
      editor: { undo_steps: 100, cursor_style: 'accent', cursor_color: '#4f5a43', cursor_trail: true },
      todo: { done_retention_hours: 24 },
      launcher_trigger: { enabled: true, trigger: 'keyboard_or_mouse_middle', keyboard_shortcut: 'Ctrl+Alt+Space', excluded_apps: [] },
      translation: { target_language: 'zh-CN', model: '' },
      transcription: { model: 'whisper-1', language: '' },
      developer: { debug_mode: false },
    });
    vi.mocked(copyTextToClipboard).mockResolvedValue(undefined);
  });

  it('captures text and refreshes the Inbox list', async () => {
    render(<InboxPanel />);
    fireEvent.change(screen.getByRole('textbox', { name: '捕获内容' }), {
      target: { value: 'A useful link https://example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));

    await waitFor(() => {
      expect(captureInboxItemMock).toHaveBeenCalledWith({
        title: '',
        content: 'A useful link https://example.com',
        source: 'manager',
        context: 'manual-capture',
        imageDataUrl: undefined,
        filePaths: [],
      });
    });
    expect(listInboxItemsMock).toHaveBeenCalledWith(true);
  });

  it('captures a pasted clipboard image with its data and filename', async () => {
    render(<InboxPanel />);
    const image = new File(['png payload'], 'lecture.png', { type: 'image/png' });
    fireEvent.paste(screen.getByRole('textbox', { name: '捕获内容' }), {
      clipboardData: { files: [image] },
    });

    expect(await screen.findByText('图片：lecture.png')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    await waitFor(() => expect(captureInboxItemMock).toHaveBeenCalledWith(expect.objectContaining({
      imageDataUrl: expect.stringMatching(/^data:image\/png;base64,/),
      filePaths: [],
    })));
  });

  it('rejects oversized clipboard images before reading their contents', async () => {
    const image = new File(['x'], 'large.png', { type: 'image/png' });
    Object.defineProperty(image, 'size', { value: 32 * 1024 * 1024 + 1 });
    const readAsDataUrl = vi.spyOn(FileReader.prototype, 'readAsDataURL');
    render(<InboxPanel />);
    fireEvent.paste(screen.getByRole('textbox', { name: '捕获内容' }), {
      clipboardData: { files: [image] },
    });

    expect(await screen.findByRole('alert')).toHaveTextContent('图片超过 32 MB 限制');
    expect(readAsDataUrl).not.toHaveBeenCalled();
  });

  it('captures files returned by the native file picker', async () => {
    openDialogMock.mockResolvedValueOnce('C:\\docs\\brief.pdf' as never);
    render(<InboxPanel />);
    fireEvent.click(screen.getByRole('button', { name: '添加文件' }));

    expect(await screen.findByText('brief.pdf')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '保存到收集箱' }));
    await waitFor(() => expect(captureInboxItemMock).toHaveBeenCalledWith(expect.objectContaining({
      content: '',
      filePaths: ['C:\\docs\\brief.pdf'],
    })));
  });

  it('converts an item to a note, archives it, and offers state restoration', async () => {
    listInboxItemsMock.mockResolvedValueOnce([fixture()]).mockResolvedValueOnce([
      fixture({ archivedAt: '2026-10-17T11:00:00.000Z', convertedNoteId: 'note-1' }),
    ]).mockResolvedValue([]);
    render(<InboxPanel />);

    fireEvent.click(await screen.findByRole('button', { name: '整理为笔记' }));
    await waitFor(() => expect(convertInboxItemToNoteMock).toHaveBeenCalledWith('item-1'));
    expect(archiveInboxItemMock).not.toHaveBeenCalled();
    expect(await screen.findByText('已整理为笔记，可在“笔记”页面查看')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: '已归档' }));
    fireEvent.click(screen.getByRole('button', { name: '移回收集箱' }));
    await waitFor(() => expect(restoreInboxItemMock).toHaveBeenCalledWith('item-1'));
  });

  it('shows restore errors while viewing archived items', async () => {
    listInboxItemsMock.mockResolvedValueOnce([
      fixture({ archivedAt: '2026-10-17T11:00:00.000Z' }),
    ]);
    restoreInboxItemMock.mockRejectedValueOnce(new Error('恢复失败'));
    render(<InboxPanel />);
    fireEvent.click(screen.getByRole('tab', { name: '已归档' }));
    fireEvent.click(await screen.findByRole('button', { name: '移回收集箱' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('恢复失败');
  });

  it('opens captured links and attachments through native commands', async () => {
    listInboxItemsMock.mockResolvedValueOnce([
      fixture({
        kind: 'link',
        title: 'https://example.com',
        content: 'https://example.com',
        attachments: [{
          id: 'attachment-1', itemId: 'item-1', kind: 'file', name: 'brief.pdf',
          mimeType: 'application/pdf', filename: 'stored.pdf', sizeBytes: 20, createdAt: '2026-10-17T10:00:00.000Z',
        }],
      }),
    ]);
    render(<InboxPanel />);

    vi.mocked(openInboxLink).mockResolvedValue(undefined);
    vi.mocked(openInboxAttachment).mockResolvedValue(undefined);
    fireEvent.click(await screen.findByRole('button', { name: /^打开$/ }));
    fireEvent.click(screen.getByRole('button', { name: '打开附件 · brief.pdf' }));

    await waitFor(() => expect(openInboxLink).toHaveBeenCalledWith('item-1'));
    expect(openInboxAttachment).toHaveBeenCalledWith('attachment-1');
  });

  it('transcribes a captured recording after explicit confirmation', async () => {
    const audioItem = fixture({
      kind: 'audio',
      title: '录音 2026-10-17',
      attachments: [{
        id: 'audio-1', itemId: 'item-1', kind: 'audio', name: '会议录音.wav',
        mimeType: 'audio/wav', filename: 'stored.wav', sizeBytes: 96_000, createdAt: '2026-10-17T10:00:00.000Z',
      }],
    });
    // Keep the item rendered across the post-transcription list refresh.
    listInboxItemsMock.mockResolvedValueOnce([audioItem]).mockResolvedValue([audioItem]);
    render(<InboxPanel />);

    // The control stays disabled until the saved model preference loads.
    await waitFor(() => expect(screen.getByRole('button', { name: '转写录音' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: '转写录音' }));
    // Privacy confirmation gates the provider request.
    expect(transcribeAudioMock).not.toHaveBeenCalled();
    expect(screen.getByText(/隐私提示/).textContent).toContain('未确认前不会发送');
    expect(screen.getAllByText(/会议录音\.wav/).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole('button', { name: '确认并发送' }));
    await waitFor(() => expect(transcribeAudioMock).toHaveBeenCalledWith('item-1'));
    expect(await screen.findByText('会议要点：先确认范围，再排期。')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '复制文本' }));
    await waitFor(() => expect(copyTextToClipboard).toHaveBeenCalledWith('会议要点：先确认范围，再排期。'));
    expect(await screen.findByRole('status')).toHaveTextContent('已复制转写文本');
  });

  it('hides the transcription control for archived audio items', async () => {
    listInboxItemsMock.mockResolvedValueOnce([
      fixture({
        kind: 'audio',
        title: '录音 2026-10-17',
        archivedAt: '2026-10-17T11:00:00.000Z',
        attachments: [{
          id: 'audio-1', itemId: 'item-1', kind: 'audio', name: '会议录音.wav',
          mimeType: 'audio/wav', filename: 'stored.wav', sizeBytes: 96_000, createdAt: '2026-10-17T10:00:00.000Z',
        }],
      }),
    ]);
    render(<InboxPanel />);

    fireEvent.click(screen.getByRole('tab', { name: '已归档' }));
    expect(await screen.findByText('录音 2026-10-17')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '转写录音' })).toBeNull();
  });

  it('deletes an item after the user confirms and refreshes the list', async () => {
    listInboxItemsMock.mockResolvedValueOnce([fixture()]).mockResolvedValue([]);
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<InboxPanel />);

    expect(await screen.findByText('Captured thought')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    await waitFor(() => expect(deleteInboxItemMock).toHaveBeenCalledWith('item-1'));
    expect(confirmSpy).toHaveBeenCalledTimes(1);
    expect(listInboxItemsMock).toHaveBeenCalledTimes(2);
    expect(await screen.findByRole('status')).toHaveTextContent('已移到回收站');
    confirmSpy.mockRestore();
  });

  it('keeps the item when the delete confirmation is cancelled', async () => {
    listInboxItemsMock.mockResolvedValue([fixture()]);
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<InboxPanel />);

    expect(await screen.findByText('Captured thought')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    expect(confirmSpy).toHaveBeenCalledTimes(1);
    expect(deleteInboxItemMock).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it('surfaces a delete failure without dropping the item', async () => {
    listInboxItemsMock.mockResolvedValue([fixture()]);
    deleteInboxItemMock.mockRejectedValueOnce(new Error('删除失败：文件被占用'));
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<InboxPanel />);

    expect(await screen.findByText('Captured thought')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '删除' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('删除失败：文件被占用');
    expect(screen.getByText('Captured thought')).toBeInTheDocument();
    confirmSpy.mockRestore();
  });

  it('opens the resource folder from the toolbar', async () => {
    render(<InboxPanel />);

    fireEvent.click(screen.getByRole('button', { name: '打开资源文件夹' }));

    await waitFor(() => expect(openInboxAssetsDirMock).toHaveBeenCalledTimes(1));
  });
});
