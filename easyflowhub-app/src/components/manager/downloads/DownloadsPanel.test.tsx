import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DownloadsPanel } from './DownloadsPanel';
import * as downloadApi from '../../../lib/tauri/download';
import { captureInboxItem } from '../../../lib/tauri/inbox';

vi.mock('../../../lib/tauri/download', () => ({
  getDownloadSettings: vi.fn(),
  updateDownloadSettings: vi.fn(),
  listDownloads: vi.fn(),
  enqueueDownload: vi.fn(),
  pauseDownload: vi.fn(),
  resumeDownload: vi.fn(),
  cancelDownload: vi.fn(),
  retryDownload: vi.fn(),
  openDownload: vi.fn(),
  revealDownload: vi.fn(),
  copyDownloadPath: vi.fn(),
}));

vi.mock('../../../lib/tauri/inbox', () => ({ captureInboxItem: vi.fn() }));
const copyTextMock = vi.hoisted(() => vi.fn());
vi.mock('../../../lib/tauri/clipboard', () => ({ copyTextToClipboard: copyTextMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const settings = { targetDir: null, conflictPolicy: 'rename' as const };
const queued = {
  id: 'download-1', url: 'https://example.com/file.zip', fileName: 'file.zip', targetDir: null, targetPath: null,
  status: 'queued' as const, downloadedBytes: 0, totalBytes: null, error: null, retryCount: 0,
  createdAt: '2026-10-17T00:00:00Z', updatedAt: '2026-10-17T00:00:00Z',
};

describe('DownloadsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(downloadApi.getDownloadSettings).mockResolvedValue(settings);
    vi.mocked(downloadApi.listDownloads).mockResolvedValue([]);
    vi.mocked(downloadApi.updateDownloadSettings).mockImplementation(async (next) => next);
    vi.mocked(downloadApi.enqueueDownload).mockResolvedValue(queued);
    vi.mocked(downloadApi.pauseDownload).mockResolvedValue({ ...queued, status: 'paused' });
    vi.mocked(downloadApi.resumeDownload).mockResolvedValue({ ...queued, status: 'downloading' });
    vi.mocked(downloadApi.cancelDownload).mockResolvedValue({ ...queued, status: 'cancelled' });
    vi.mocked(captureInboxItem).mockResolvedValue({} as never);
    copyTextMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('validates an empty URL before invoking native queue', async () => {
    render(<DownloadsPanel />);
    await screen.findByText('暂无下载，粘贴 URL 后加入队列。');
    fireEvent.click(screen.getByRole('button', { name: '加入队列' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('请输入下载 URL。');
    expect(downloadApi.enqueueDownload).not.toHaveBeenCalled();
  });

  it('enqueues a valid URL and renders it in the queue', async () => {
    render(<DownloadsPanel />);
    await screen.findByText('暂无下载，粘贴 URL 后加入队列。');
    fireEvent.change(screen.getByLabelText('下载 URL'), { target: { value: queued.url } });
    fireEvent.click(screen.getByRole('button', { name: '加入队列' }));
    await waitFor(() => expect(downloadApi.enqueueDownload).toHaveBeenCalledWith({
      url: queued.url, fileName: undefined, targetDir: undefined, conflictPolicy: 'rename',
    }));
    expect(await screen.findByText('file.zip')).toBeInTheDocument();
  });

  it('can pause an active item and add a completed item to Inbox', async () => {
    const completed = { ...queued, status: 'completed' as const, targetPath: 'D:\\Downloads\\file.zip' };
    vi.mocked(downloadApi.listDownloads).mockResolvedValueOnce([queued]).mockResolvedValue([completed]);
    vi.mocked(downloadApi.copyDownloadPath).mockResolvedValue(completed.targetPath);
    render(<DownloadsPanel />);
    expect(await screen.findByText('排队中')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '暂停' }));
    await waitFor(() => expect(downloadApi.pauseDownload).toHaveBeenCalledWith(queued.id));

    vi.mocked(downloadApi.listDownloads).mockResolvedValue([completed]);
    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '加入收集箱' })).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: '加入收集箱' }));
    await waitFor(() => expect(captureInboxItem).toHaveBeenCalledWith({
      filePaths: [completed.targetPath], source: 'download', context: 'download',
    }));
  });

  it('copies a completed path through the native clipboard bridge', async () => {
    const completed = { ...queued, status: 'completed' as const, targetPath: 'D:\\Downloads\\file.zip' };
    vi.mocked(downloadApi.listDownloads).mockResolvedValue([completed]);
    vi.mocked(downloadApi.copyDownloadPath).mockResolvedValue(completed.targetPath);
    render(<DownloadsPanel />);
    expect(await screen.findByText('已完成')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '复制路径' }));
    await waitFor(() => expect(downloadApi.copyDownloadPath).toHaveBeenCalledWith(completed.id));
    await waitFor(() => expect(copyTextMock).toHaveBeenCalledWith(completed.targetPath));
    expect(await screen.findByRole('status')).toHaveTextContent('路径已复制。');
  });

  it('keeps a native clipboard error visible without changing the queue', async () => {
    const completed = { ...queued, status: 'completed' as const, targetPath: 'D:\\Downloads\\file.zip' };
    vi.mocked(downloadApi.listDownloads).mockResolvedValue([completed]);
    vi.mocked(downloadApi.copyDownloadPath).mockResolvedValue(completed.targetPath);
    copyTextMock.mockRejectedValueOnce(new Error('无法写入系统剪贴板'));
    render(<DownloadsPanel />);
    expect(await screen.findByText('已完成')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '复制路径' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('无法写入系统剪贴板');
    expect(screen.getByText('已完成')).toBeInTheDocument();
  });

  it('requires explicit confirmation before using overwrite policy', async () => {
    vi.mocked(downloadApi.getDownloadSettings).mockResolvedValue({ targetDir: null, conflictPolicy: 'overwrite' });
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<DownloadsPanel />);
    await screen.findByText('暂无下载，粘贴 URL 后加入队列。');
    fireEvent.change(screen.getByLabelText('下载 URL'), { target: { value: queued.url } });
    fireEvent.click(screen.getByRole('button', { name: '加入队列' }));
    expect(confirm).toHaveBeenCalledWith('覆盖策略会替换同名文件，确定继续吗？');
    expect(downloadApi.enqueueDownload).not.toHaveBeenCalled();
  });

  it('surfaces a failed item and lets the user retry it', async () => {
    const failed = { ...queued, status: 'failed' as const, error: '连接下载地址失败', retryCount: 1 };
    const retried = { ...failed, status: 'queued' as const, error: null, retryCount: 2 };
    vi.mocked(downloadApi.listDownloads).mockResolvedValue([failed]);
    vi.mocked(downloadApi.retryDownload).mockResolvedValue(retried);
    render(<DownloadsPanel />);
    expect(await screen.findByText('失败')).toBeInTheDocument();
    expect(screen.getByText('连接下载地址失败')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    await waitFor(() => expect(downloadApi.retryDownload).toHaveBeenCalledWith(failed.id));
    await waitFor(() => expect(screen.getByText('排队中')).toBeInTheDocument());
  });

  it('resumes a paused item and cancels an active item', async () => {
    const paused = { ...queued, status: 'paused' as const };
    vi.mocked(downloadApi.listDownloads).mockResolvedValue([paused]);
    render(<DownloadsPanel />);
    expect(await screen.findByText('已暂停')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '继续' }));
    await waitFor(() => expect(downloadApi.resumeDownload).toHaveBeenCalledWith(paused.id));

    cleanup();
    vi.mocked(downloadApi.listDownloads).mockResolvedValue([queued]);
    render(<DownloadsPanel />);
    expect(await screen.findByText('排队中')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '取消' }));
    await waitFor(() => expect(downloadApi.cancelDownload).toHaveBeenCalledWith(queued.id));
  });
});
