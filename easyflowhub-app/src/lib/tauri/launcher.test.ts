import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  getConfiguredSnowShotPath,
  getLauncherAlwaysOnTop,
  hideLauncherWindow,
  launchSnowShot,
  openSnowShotInstallPage,
  onLauncherOpened,
  onLauncherFocused,
  onLauncherHidden,
  saveConfiguredSnowShotPath,
  setLauncherAlwaysOnTop,
  startLauncherDragging,
  replaceSelectedText,
  SNOW_SHOT_INSTALL_URL,
  SNOW_SHOT_PATH_STORAGE_KEY,
} from './launcher';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe('launcher Tauri bridge', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.mocked(listen).mockReset();
    localStorage.clear();
  });

  it('passes a trimmed custom Snow Shot path to the native launcher', async () => {
    invokeMock.mockResolvedValue({ status: 'launched' } as never);

    await expect(launchSnowShot('  C:\\Tools\\SnowShot.exe  ')).resolves.toEqual({ status: 'launched' });
    expect(invokeMock).toHaveBeenCalledWith('launch_snow_shot', {
      customPath: 'C:\\Tools\\SnowShot.exe',
    });
  });

  it('uses null when no custom path is configured', async () => {
    invokeMock.mockResolvedValue({ status: 'not_installed', installUrl: SNOW_SHOT_INSTALL_URL } as never);

    await expect(launchSnowShot()).resolves.toMatchObject({ status: 'not_installed' });
    expect(invokeMock).toHaveBeenCalledWith('launch_snow_shot', { customPath: null });
  });

  it('opens the native installation page command', async () => {
    invokeMock.mockResolvedValue(undefined as never);

    await openSnowShotInstallPage();
    expect(invokeMock).toHaveBeenCalledWith('open_snow_shot_install_page');
  });

  it('uses native window commands for close, drag and pin state', async () => {
    invokeMock.mockResolvedValue(true as never);

    await hideLauncherWindow();
    await startLauncherDragging();
    await expect(setLauncherAlwaysOnTop(true)).resolves.toBe(true);
    await expect(getLauncherAlwaysOnTop()).resolves.toBe(true);

    expect(invokeMock.mock.calls).toEqual([
      ['hide_launcher_window'],
      ['start_launcher_dragging'],
      ['set_launcher_always_on_top', { enabled: true }],
      ['get_launcher_always_on_top'],
    ]);
  });

  it('forwards explicit selected-text replacement to the native host', async () => {
    invokeMock.mockResolvedValue(undefined as never);
    await replaceSelectedText('译文');
    expect(invokeMock).toHaveBeenCalledWith('replace_selected_text', { text: '译文' });
  });

  it('subscribes to native launcher activation events', async () => {
    const callback = vi.fn();
    vi.mocked(listen).mockResolvedValue(vi.fn());

    await onLauncherOpened(callback);
    await onLauncherFocused(callback);

    expect(listen).toHaveBeenCalledWith('launcher-opened', callback);
    expect(listen).toHaveBeenCalledWith('launcher-focused', callback);
  });

  it('forwards every native dismissal without exposing the event payload and preserves unsubscribe', async () => {
    const onHidden = vi.fn();
    const unlisten = vi.fn();
    vi.mocked(listen).mockResolvedValue(unlisten);

    const stopListening = await onLauncherHidden(onHidden);

    expect(listen).toHaveBeenCalledWith('launcher-hidden', expect.any(Function));
    const listener = vi.mocked(listen).mock.calls[0][1];
    listener({ event: 'launcher-hidden', id: 1, payload: null });
    expect(onHidden).toHaveBeenCalledExactlyOnceWith();
    expect(stopListening).toBe(unlisten);
    stopListening();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('reads a configured path while treating blank and missing values as unset', () => {
    expect(getConfiguredSnowShotPath()).toBeUndefined();
    localStorage.setItem(SNOW_SHOT_PATH_STORAGE_KEY, '  C:\\Portable\\SnowShot.exe  ');
    expect(getConfiguredSnowShotPath()).toBe('C:\\Portable\\SnowShot.exe');
    localStorage.setItem(SNOW_SHOT_PATH_STORAGE_KEY, '   ');
    expect(getConfiguredSnowShotPath()).toBeUndefined();
  });

  it('saves and clears the optional custom Snow Shot path', () => {
    saveConfiguredSnowShotPath('  C:\\Portable\\SnowShot.exe  ');
    expect(localStorage.getItem(SNOW_SHOT_PATH_STORAGE_KEY)).toBe('C:\\Portable\\SnowShot.exe');
    saveConfiguredSnowShotPath('   ');
    expect(localStorage.getItem(SNOW_SHOT_PATH_STORAGE_KEY)).toBeNull();
  });

  it('persists and clears a custom Snow Shot path', () => {
    saveConfiguredSnowShotPath('  C:\\Portable\\SnowShot.exe  ');
    expect(localStorage.getItem(SNOW_SHOT_PATH_STORAGE_KEY)).toBe('C:\\Portable\\SnowShot.exe');
    saveConfiguredSnowShotPath('   ');
    expect(localStorage.getItem(SNOW_SHOT_PATH_STORAGE_KEY)).toBeNull();
  });
});

