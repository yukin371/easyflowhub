import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  cancelDownload,
  copyDownloadPath,
  enqueueDownload,
  getDownloadSettings,
  listDownloads,
  openDownload,
  pauseDownload,
  retryDownload,
  resumeDownload,
  revealDownload,
  updateDownloadSettings,
} from './download';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

describe('download Tauri bridge', () => {
  beforeEach(() => invokeMock.mockReset());

  it('forwards settings and enqueue requests with stable camelCase fields', async () => {
    invokeMock.mockResolvedValue({});
    const settings = { targetDir: 'D:\\Downloads', conflictPolicy: 'rename' as const };
    await getDownloadSettings();
    expect(invokeMock).toHaveBeenLastCalledWith('download_get_settings');
    await updateDownloadSettings(settings);
    expect(invokeMock).toHaveBeenLastCalledWith('download_update_settings', { settings });
    const request = { url: 'https://example.com/a.zip', fileName: 'a.zip', targetDir: 'D:\\Downloads', conflictPolicy: 'fail' as const };
    await enqueueDownload(request);
    expect(invokeMock).toHaveBeenLastCalledWith('download_enqueue', request);
  });

  it('wraps queue lifecycle and result actions', async () => {
    invokeMock.mockResolvedValue({});
    await listDownloads();
    expect(invokeMock).toHaveBeenLastCalledWith('download_list');
    await pauseDownload('a');
    expect(invokeMock).toHaveBeenLastCalledWith('download_pause', { id: 'a' });
    await resumeDownload('a');
    expect(invokeMock).toHaveBeenLastCalledWith('download_resume', { id: 'a' });
    await cancelDownload('a');
    expect(invokeMock).toHaveBeenLastCalledWith('download_cancel', { id: 'a' });
    await retryDownload('a');
    expect(invokeMock).toHaveBeenLastCalledWith('download_retry', { id: 'a' });
    await openDownload('a');
    expect(invokeMock).toHaveBeenLastCalledWith('download_open', { id: 'a' });
    await revealDownload('a');
    expect(invokeMock).toHaveBeenLastCalledWith('download_reveal', { id: 'a' });
    await copyDownloadPath('a');
    expect(invokeMock).toHaveBeenLastCalledWith('download_copy_path', { id: 'a' });
  });
});
