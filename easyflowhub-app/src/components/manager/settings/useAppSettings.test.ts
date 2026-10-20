import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppSettings } from './useAppSettings';
import { getSettings, updateSettings } from '../../../lib/tauri/settings';
import { setDebugMode } from '../../../lib/tauri/logging';
import type { AppSettings, LauncherTriggerSettings } from '../../../types/settings';

vi.mock('../../../lib/tauri/settings', () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
}));

vi.mock('../../../lib/tauri/logging', () => ({
  setDebugMode: vi.fn(),
}));

const getSettingsMock = vi.mocked(getSettings);
const updateSettingsMock = vi.mocked(updateSettings);
const setDebugModeMock = vi.mocked(setDebugMode);

const appSettings: AppSettings = {
  quick_note: { width: 400, height: 300 },
  trash: { retention_days: 30 },
  editor: {
    undo_steps: 100,
    cursor_style: 'accent',
    cursor_color: '#4f5a43',
    cursor_trail: true,
  },
  todo: { done_retention_hours: 24 },
  translation: { target_language: 'zh-CN', model: '' },
  transcription: { model: '', language: '' },
  developer: { debug_mode: false },
  launcher_trigger: {
    enabled: true,
    trigger: 'keyboard_or_mouse_middle',
    keyboard_shortcut: 'Ctrl+Alt+Space',
    excluded_apps: [],
  },
};

describe('useAppSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSettingsMock.mockResolvedValue(appSettings);
    updateSettingsMock.mockImplementation(async (settings) => settings);
    setDebugModeMock.mockResolvedValue(true);
  });

  it('普通保存只回写已提交的 launcher_trigger 镜像，不携带未提交的 Launcher 草稿（F08）', async () => {
    const { result } = renderHook(() => useAppSettings());
    await act(async () => { result.current.reload(); });
    expect(result.current.loadState).toBe('ready');

    // Launcher 域在自己的提交入口成功后刷新镜像（useShortcutsSettings 编排）。
    const committedTrigger: LauncherTriggerSettings = {
      enabled: true,
      trigger: 'mouse_middle',
      keyboard_shortcut: 'Ctrl+Alt+L',
      excluded_apps: ['Code.exe'],
    };
    act(() => { result.current.syncLauncherTrigger(committedTrigger); });

    // 修改普通域草稿并保存。
    act(() => { result.current.patch('translation', { model: ' relay-model ' }); });
    await act(async () => { result.current.save(); });

    expect(updateSettingsMock).toHaveBeenCalledTimes(1);
    const payload = updateSettingsMock.mock.calls[0][0];
    // F08：普通保存回写已提交镜像，而不是 Launcher 草稿或旧加载值。
    expect(payload.launcher_trigger).toEqual(committedTrigger);
    expect(payload.translation).toEqual({ target_language: 'zh-CN', model: 'relay-model' });
    expect(result.current.saveState).toBe('saved');
  });

  it('加载失败时阻止保存，重试加载成功后恢复', async () => {
    getSettingsMock.mockRejectedValueOnce(new Error('database is locked'));
    const { result } = renderHook(() => useAppSettings());
    await act(async () => { result.current.reload(); });
    expect(result.current.loadState).toBe('failed');

    await act(async () => { result.current.save(); });
    expect(updateSettingsMock).not.toHaveBeenCalled();
    expect(result.current.error).toContain('设置尚未成功加载');

    await act(async () => { result.current.reload(); });
    expect(result.current.loadState).toBe('ready');
    await act(async () => { result.current.save(); });
    expect(updateSettingsMock).toHaveBeenCalledTimes(1);
  });

  it('保存时裁剪模型并立即应用调试模式', async () => {
    const { result } = renderHook(() => useAppSettings());
    await act(async () => { result.current.reload(); });

    act(() => { result.current.patch('transcription', { model: ' whisper-1 ', language: 'zh' }); });
    act(() => { result.current.patch('developer', { debug_mode: true }); });
    await act(async () => { result.current.save(); });

    expect(updateSettingsMock).toHaveBeenCalledWith({
      ...appSettings,
      transcription: { model: 'whisper-1', language: 'zh' },
      developer: { debug_mode: true },
    });
    expect(setDebugModeMock).toHaveBeenCalledWith(true);
    expect(result.current.error).toBeUndefined();
  });

  it('reload 合并旧设置默认值并回传合并结果', async () => {
    const { translation: _translation, ...legacySettings } = appSettings;
    getSettingsMock.mockResolvedValue(legacySettings as AppSettings);
    const onLoaded = vi.fn();
    const { result } = renderHook(() => useAppSettings());

    await act(async () => { result.current.reload(onLoaded); });

    expect(onLoaded).toHaveBeenCalledWith(appSettings);
    expect(result.current.settings.translation).toEqual({ target_language: 'zh-CN', model: '' });
  });
});
