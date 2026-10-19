import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  clearFileIndex,
  copyFile,
  copyFilePath,
  deleteFile,
  getFileSearchStatus,
  listRecentFiles,
  moveFile,
  openFile,
  pauseFileIndexing,
  previewFile,
  refreshFileIndex,
  revealFileInExplorer,
  renameFile,
  resumeFileIndexing,
  searchFiles,
  setFileIndexPaused,
  setFileSearchRoot,
} from './fileSearch';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe('file search Tauri bridge', () => {
  beforeEach(() => invokeMock.mockReset());

  it('reads and updates the native index lifecycle state', async () => {
    const status = { generation: 3, paused: false, root: 'D:\\Files', count: 12 };
    invokeMock.mockResolvedValue(status);

    await expect(getFileSearchStatus()).resolves.toBe(status);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_get_status');
    await setFileSearchRoot('D:\\Files');
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_set_root', { root: 'D:\\Files' });
    await setFileSearchRoot(null);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_set_root', { root: null });
    await setFileIndexPaused(true);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_set_paused', { paused: true });
    await pauseFileIndexing();
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_set_paused', { paused: true });
    await resumeFileIndexing();
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_set_paused', { paused: false });
    await refreshFileIndex();
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_refresh_index');
    await clearFileIndex();
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_clear_index');
  });

  it('wraps search requests instead of leaking invoke into UI', async () => {
    invokeMock.mockResolvedValue({ items: [], total: 0, generation: 4 });
    const request = { query: 'report', extension: '.md', sort: 'recent' as const, limit: 20 };
    await searchFiles(request);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_search', { request });
    await searchFiles();
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_search', { request: { query: '' } });
  });

  it('forwards preview and path actions with the native safety boundary', async () => {
    invokeMock.mockResolvedValue({});
    await previewFile('D:\\Files\\readme.md', 4);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_preview', { path: 'D:\\Files\\readme.md', expectedGeneration: 4 });
    await openFile('D:\\Files\\readme.md', 4);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_open_path', { path: 'D:\\Files\\readme.md', expectedGeneration: 4 });
    await copyFilePath('D:\\Files\\readme.md', 4);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_copy_path', { path: 'D:\\Files\\readme.md', expectedGeneration: 4 });
    await revealFileInExplorer('D:\\Files\\readme.md', 4);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_reveal_in_explorer', { path: 'D:\\Files\\readme.md', expectedGeneration: 4 });
    await listRecentFiles();
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_recent', { limit: null });
    await listRecentFiles(5);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_recent', { limit: 5 });
    await renameFile('D:\\Files\\old.md', 'new.md', 4);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_rename_path', {
      path: 'D:\\Files\\old.md',
      newName: 'new.md',
      expectedGeneration: 4,
    });
    await moveFile('D:\\Files\\new.md', 'D:\\Archive', 4);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_move_path', {
      path: 'D:\\Files\\new.md',
      destinationDir: 'D:\\Archive',
      expectedGeneration: 4,
    });
    await copyFile('D:\\Files\\new.md', 'D:\\Archive', 4);
    expect(invokeMock).toHaveBeenLastCalledWith('file_search_copy_file', {
      path: 'D:\\Files\\new.md',
      destinationDir: 'D:\\Archive',
      expectedGeneration: 4,
    });
  });

  it('uses the recycle-bin command for deletion', async () => {
    invokeMock.mockResolvedValue({ ok: true, path: 'D:\\Files\\old.md', generation: 8 });
    await expect(deleteFile('D:\\Files\\old.md', 7)).resolves.toEqual({
      ok: true,
      path: 'D:\\Files\\old.md',
      generation: 8,
    });
    expect(invokeMock).toHaveBeenCalledWith('file_search_delete_to_recycle_bin', {
      path: 'D:\\Files\\old.md',
      expectedGeneration: 7,
    });
  });
});
