import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { open } from '@tauri-apps/plugin-dialog';
import {
  clearFileIndex,
  copyFileSearchPath,
  copyIndexedFile,
  deleteToRecycleBin,
  getFileSearchStatus,
  listRecentFiles,
  moveIndexedFile,
  openFile,
  previewFile,
  refreshFileIndex,
  renameIndexedFile,
  revealFileInExplorer,
  searchFiles,
  setFileSearchPaused,
  setFileSearchRoot,
} from '../../../lib/tauri/fileSearch';
import { FilesPanel } from './FilesPanel';

const copyTextMock = vi.hoisted(() => vi.fn());
vi.mock('../../../lib/tauri/clipboard', () => ({ copyTextToClipboard: copyTextMock }));

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../../../lib/tauri/fileSearch', () => ({
  clearFileIndex: vi.fn(),
  copyFileSearchPath: vi.fn(),
  copyIndexedFile: vi.fn(),
  deleteToRecycleBin: vi.fn(),
  getFileSearchStatus: vi.fn(),
  listRecentFiles: vi.fn(),
  moveIndexedFile: vi.fn(),
  openFile: vi.fn(),
  previewFile: vi.fn(),
  refreshFileIndex: vi.fn(),
  renameIndexedFile: vi.fn(),
  revealFileInExplorer: vi.fn(),
  searchFiles: vi.fn(),
  setFileSearchPaused: vi.fn(),
  setFileSearchRoot: vi.fn(),
}));

const dialogMock = vi.mocked(open);
const statusMock = vi.mocked(getFileSearchStatus);
const setRootMock = vi.mocked(setFileSearchRoot);
const searchMock = vi.mocked(searchFiles);
const recentMock = vi.mocked(listRecentFiles);
const previewMock = vi.mocked(previewFile);
const copyPathMock = vi.mocked(copyFileSearchPath);
const deleteMock = vi.mocked(deleteToRecycleBin);
const clearMock = vi.mocked(clearFileIndex);
const refreshMock = vi.mocked(refreshFileIndex);
const pauseMock = vi.mocked(setFileSearchPaused);
const openFileMock = vi.mocked(openFile);
const revealMock = vi.mocked(revealFileInExplorer);
const renameMock = vi.mocked(renameIndexedFile);
const moveMock = vi.mocked(moveIndexedFile);
const copyMock = vi.mocked(copyIndexedFile);

const emptyStatus = { generation: 1, paused: false, root: null, count: 0 };
const rootedStatus = { generation: 4, paused: false, root: 'D:\\Work', count: 3 };
const fixture = {
  id: 'D:\\Work\\brief.txt',
  path: 'D:\\Work\\brief.txt',
  name: 'brief.txt',
  extension: 'txt',
  size: 2048,
  modifiedAt: '2026-10-17T10:00:00.000Z',
  isDir: false,
};
const response = { items: [fixture], total: 1, generation: 4 };

describe('FilesPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    statusMock.mockResolvedValue(emptyStatus);
    setRootMock.mockResolvedValue(rootedStatus);
    searchMock.mockResolvedValue(response);
    recentMock.mockResolvedValue({ items: [], total: 0, generation: 4 });
    previewMock.mockResolvedValue({ kind: 'text', path: fixture.path, content: 'hello', mimeType: 'text/plain', imageDataUrl: null, size: 5, truncated: false });
    copyPathMock.mockResolvedValue(fixture.path);
    deleteMock.mockResolvedValue({ ok: true, path: fixture.path, generation: 5 });
    clearMock.mockResolvedValue({ ...rootedStatus, count: 0, generation: 5 });
    refreshMock.mockResolvedValue(rootedStatus);
    pauseMock.mockResolvedValue({ ...rootedStatus, paused: true, generation: 5 });
    openFileMock.mockResolvedValue(fixture);
    revealMock.mockResolvedValue(fixture.path);
    renameMock.mockResolvedValue({ entry: { ...fixture, name: 'renamed.txt', path: 'D:\\Work\\renamed.txt', id: 'D:\\Work\\renamed.txt' }, generation: 5 });
    moveMock.mockResolvedValue({ entry: fixture, generation: 5 });
    copyMock.mockResolvedValue({ entry: fixture, generation: 5 });
    dialogMock.mockResolvedValue(null);
    vi.stubGlobal('confirm', vi.fn(() => true));
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
    copyTextMock.mockResolvedValue(undefined);
  });

  it('requires a root before search and sets one through the native bridge', async () => {
    render(<FilesPanel />);
    expect(await screen.findByText('先选择一个索引目录')).toBeInTheDocument();
    expect(screen.getByRole('searchbox')).toBeDisabled();

    dialogMock.mockResolvedValueOnce('D:\\Work' as never);
    fireEvent.click(screen.getAllByRole('button', { name: '选择目录' })[0]);

    await waitFor(() => expect(setRootMock).toHaveBeenCalledWith('D:\\Work'));
    expect(await screen.findByPlaceholderText('输入名称或路径，自动搜索')).toBeEnabled();
  });

  it('shows a desktop service message when the browser preview has no native bridge', async () => {
    statusMock.mockRejectedValueOnce(new TypeError("Cannot read properties of undefined (reading 'invoke')"));
    render(<FilesPanel />);

    expect(await screen.findByRole('alert')).toHaveTextContent('文件服务暂不可用，请在桌面应用中打开 EasyFlowHub。');
  });

  it('searches and previews a result, passing the result generation to path actions', async () => {
    statusMock.mockResolvedValue(rootedStatus);
    recentMock.mockResolvedValue(response);
    render(<FilesPanel />);
    const input = await screen.findByPlaceholderText('输入名称或路径，自动搜索');
    fireEvent.change(input, { target: { value: 'brief' } });

    expect((await screen.findAllByText('brief.txt')).length).toBeGreaterThan(0);
    await waitFor(() => expect(previewMock).toHaveBeenCalledWith(fixture.path, 4));

    fireEvent.click(screen.getByRole('button', { name: '复制路径' }));
    await waitFor(() => expect(copyPathMock).toHaveBeenCalledWith(fixture.path, 4));
    await waitFor(() => expect(copyTextMock).toHaveBeenCalledWith(fixture.path));
    fireEvent.click(screen.getByRole('button', { name: '打开' }));
    await waitFor(() => expect(openFileMock).toHaveBeenCalledWith(fixture.path, 4));
  });

  it('keeps a native clipboard error visible for path copy', async () => {
    statusMock.mockResolvedValue(rootedStatus);
    recentMock.mockResolvedValue(response);
    copyTextMock.mockRejectedValueOnce(new Error('无法写入系统剪贴板'));
    render(<FilesPanel />);
    await screen.findAllByText('brief.txt');
    fireEvent.click(screen.getByRole('button', { name: '复制路径' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('无法写入系统剪贴板');
    expect(screen.getAllByText('brief.txt').length).toBeGreaterThan(0);
  });

  it('shows recent files and small image previews without widening the asset protocol scope', async () => {
    statusMock.mockResolvedValue(rootedStatus);
    const imageFile = { ...fixture, id: 'D:\\Work\\diagram.png', path: 'D:\\Work\\diagram.png', name: 'diagram.png', extension: 'png' };
    recentMock.mockResolvedValue({ items: [imageFile], total: 1, generation: 4 });
    previewMock.mockResolvedValue({
      kind: 'image',
      path: imageFile.path,
      content: null,
      mimeType: 'image/png',
      imageDataUrl: 'data:image/png;base64,AAAA',
      size: 4,
      truncated: false,
    });
    render(<FilesPanel />);

    expect(await screen.findByText('最近使用')).toBeInTheDocument();
    expect(await screen.findByAltText('diagram.png')).toHaveAttribute('src', 'data:image/png;base64,AAAA');
    await waitFor(() => expect(previewMock).toHaveBeenCalledWith(imageFile.path, 4));

    fireEvent.click(screen.getByRole('button', { name: '暂停' }));
    await waitFor(() => expect(pauseMock).toHaveBeenCalledWith(true));
  });

  it('reloads recent results after pause and refresh so previews and actions use the new generation', async () => {
    statusMock.mockResolvedValue(rootedStatus);
    pauseMock.mockResolvedValue({ ...rootedStatus, paused: true, generation: 5 });
    pauseMock.mockResolvedValueOnce({ ...rootedStatus, paused: true, generation: 5 })
      .mockResolvedValueOnce({ ...rootedStatus, paused: false, generation: 6 });
    refreshMock.mockResolvedValue({ ...rootedStatus, paused: false, generation: 7 });
    recentMock
      .mockResolvedValueOnce(response)
      .mockResolvedValueOnce({ ...response, generation: 5 })
      .mockResolvedValueOnce({ ...response, generation: 6 })
      .mockResolvedValueOnce({ ...response, generation: 7 });
    render(<FilesPanel />);

    expect(await screen.findByText('最近使用')).toBeInTheDocument();
    await waitFor(() => expect(previewMock).toHaveBeenCalledWith(fixture.path, 4));

    fireEvent.click(screen.getByRole('button', { name: '暂停' }));
    await waitFor(() => expect(recentMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(previewMock).toHaveBeenCalledWith(fixture.path, 5));
    fireEvent.click(screen.getByRole('button', { name: '复制路径' }));
    await waitFor(() => expect(copyPathMock).toHaveBeenCalledWith(fixture.path, 5));

    fireEvent.click(screen.getByRole('button', { name: '继续索引' }));
    await waitFor(() => expect(recentMock).toHaveBeenCalledTimes(3));
    await waitFor(() => expect(previewMock).toHaveBeenCalledWith(fixture.path, 6));

    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    await waitFor(() => expect(recentMock).toHaveBeenCalledTimes(4));
    await waitFor(() => expect(previewMock).toHaveBeenCalledWith(fixture.path, 7));
    fireEvent.click(screen.getByRole('button', { name: '复制路径' }));
    await waitFor(() => expect(copyPathMock).toHaveBeenLastCalledWith(fixture.path, 7));
  });

  it('shows native errors and disables duplicate operations while a file is busy', async () => {
    statusMock.mockResolvedValue(rootedStatus);
    recentMock.mockResolvedValue(response);
    let resolveDelete: ((value: { ok: boolean; path: string; generation: number }) => void) | undefined;
    deleteMock.mockImplementationOnce(() => new Promise((resolve) => { resolveDelete = resolve; }));
    render(<FilesPanel />);
    await screen.findAllByText('brief.txt');
    fireEvent.click(screen.getByRole('button', { name: '回收站' }));
    expect(screen.getByRole('button', { name: '处理中…' })).toBeDisabled();
    resolveDelete?.({ ok: true, path: fixture.path, generation: 5 });
    await waitFor(() => expect(screen.queryByText('brief.txt')).not.toBeInTheDocument());

    cleanup();
    statusMock.mockRejectedValueOnce(new Error('权限不足'));
    render(<FilesPanel />);
    expect(await screen.findByRole('alert')).toHaveTextContent('权限不足');
  });

  it('reloads status and results when a mutation reports a stale index', async () => {
    statusMock.mockResolvedValueOnce(rootedStatus).mockResolvedValueOnce({ ...rootedStatus, generation: 5, count: 0 });
    recentMock
      .mockResolvedValueOnce(response)
      .mockResolvedValueOnce({ items: [], total: 0, generation: 5 });
    copyPathMock.mockRejectedValueOnce(new Error('文件操作已完成，但索引刷新失败：目录暂时不可访问'));
    render(<FilesPanel />);
    await screen.findAllByText('brief.txt');

    fireEvent.click(screen.getByRole('button', { name: '复制路径' }));

    await waitFor(() => expect(statusMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(recentMock).toHaveBeenCalledTimes(2));
    expect(screen.getByText('0 个文件已索引')).toBeInTheDocument();
    expect(screen.queryByText('brief.txt')).not.toBeInTheDocument();
    expect(await screen.findByRole('alert')).toHaveTextContent('索引刷新失败');
  });

  it('clears the index and preserves the selected root', async () => {
    statusMock.mockResolvedValue(rootedStatus);
    recentMock.mockResolvedValue(response);
    render(<FilesPanel />);
    await screen.findAllByText('brief.txt');
    fireEvent.click(screen.getByRole('button', { name: '清除索引' }));
    await waitFor(() => expect(clearMock).toHaveBeenCalledTimes(1));
    expect(await screen.findByText('索引记录已清除。')).toBeInTheDocument();
    expect(screen.getByText('D:\\Work')).toBeInTheDocument();
  });
});

