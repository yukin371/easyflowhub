import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { copyTextToClipboard, copyTextWithFallback, readTextFromClipboard, readTextWithFallback } from './clipboard';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe('clipboard Tauri bridge', () => {
  beforeEach(() => invokeMock.mockReset());

  it('writes text through the native command', async () => {
    invokeMock.mockResolvedValue(undefined);
    await copyTextToClipboard('D:\\Work\\report.txt');
    expect(invokeMock).toHaveBeenCalledWith('copy_text_to_clipboard', { text: 'D:\\Work\\report.txt' });
  });

  it('reads text through the native command', async () => {
    invokeMock.mockResolvedValue('selected text');
    await expect(readTextFromClipboard()).resolves.toBe('selected text');
    expect(invokeMock).toHaveBeenCalledWith('read_text_from_clipboard');
  });

  it('falls back to browser clipboard when native read is unavailable', async () => {
    invokeMock.mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { readText: vi.fn().mockResolvedValue('preview text') } });
    await expect(readTextWithFallback()).resolves.toBe('preview text');
  });

  it('prefers native write and falls back only when native is unavailable', async () => {
    invokeMock.mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText: vi.fn() } });
    await copyTextWithFallback('native');
    expect(invokeMock).toHaveBeenCalledWith('copy_text_to_clipboard', { text: 'native' });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
  });

});

