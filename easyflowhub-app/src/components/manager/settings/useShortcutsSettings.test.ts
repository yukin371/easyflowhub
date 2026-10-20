import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useShortcutsSettings } from './useShortcutsSettings';
import { applyLauncherTriggerSettings } from '../../../lib/tauri/launcher';
import {
  DEFAULT_LAUNCHER_SETTINGS,
  LAUNCHER_SETTINGS_STORAGE_KEY,
} from '../../../types/launcherSettings';
import { SHORTCUT_STORAGE_KEY } from '../../../types/shortcut';

vi.mock('../../../lib/tauri/launcher', () => ({
  applyLauncherTriggerSettings: vi.fn(),
  getConfiguredSnowShotPath: vi.fn().mockReturnValue(undefined),
  saveConfiguredSnowShotPath: vi.fn(),
}));

const applyLauncherTriggerSettingsMock = vi.mocked(applyLauncherTriggerSettings);

describe('useShortcutsSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    applyLauncherTriggerSettingsMock.mockResolvedValue(undefined);
    // 原实现会 console.error 失败原因；测试里静音避免噪音。
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
  });

  it('保存配置按原顺序提交两个域，并同步普通设置的 launcher_trigger 镜像', async () => {
    const onLauncherCommitted = vi.fn();
    const { result } = renderHook(() => useShortcutsSettings({ onLauncherCommitted }));

    act(() => { result.current.launcher.change('activation', 'middleMouse'); });
    act(() => { result.current.shortcuts.change('save', 'Ctrl+Alt+F9'); });
    await act(async () => { result.current.save(); });

    // Launcher 域：先应用到 native（含持久化），再写 UI 镜像。
    expect(applyLauncherTriggerSettingsMock).toHaveBeenCalledTimes(1);
    expect(applyLauncherTriggerSettingsMock).toHaveBeenCalledWith({
      activation: 'middleMouse',
      keyboardShortcut: 'Ctrl+Alt+Space',
      excludedApps: [],
    });
    expect(JSON.parse(localStorage.getItem(LAUNCHER_SETTINGS_STORAGE_KEY) || '{}')).toMatchObject({
      activation: 'middleMouse',
      excludedApps: [],
    });
    // 快捷键域：localStorage 落盘。
    expect(JSON.parse(localStorage.getItem(SHORTCUT_STORAGE_KEY) || '{}')).toMatchObject({
      save: 'Ctrl+Alt+F9',
    });
    expect(result.current.launcher.saveState).toBe('saved');
    expect(result.current.shortcuts.saveState).toBe('saved');
    // F08：Launcher 提交成功后通知普通设置聚合刷新镜像。
    expect(onLauncherCommitted).toHaveBeenCalledWith({
      enabled: true,
      trigger: 'mouse_middle',
      keyboard_shortcut: 'Ctrl+Alt+Space',
      excluded_apps: [],
    });
  });

  it('Launcher 应用失败时两个域都不落盘，并保持未保存与错误提示', async () => {
    applyLauncherTriggerSettingsMock.mockRejectedValueOnce(new Error('快捷键已被占用'));
    const onLauncherCommitted = vi.fn();
    const { result } = renderHook(() => useShortcutsSettings({ onLauncherCommitted }));

    act(() => { result.current.launcher.change('keyboardShortcut', 'Ctrl+Alt+F9'); });
    act(() => { result.current.shortcuts.change('save', 'Ctrl+Alt+G9'); });
    await act(async () => { result.current.save(); });

    expect(localStorage.getItem(LAUNCHER_SETTINGS_STORAGE_KEY)).toBeNull();
    expect(localStorage.getItem(SHORTCUT_STORAGE_KEY)).toBeNull();
    expect(result.current.launcher.error).toBe('快捷键已被占用');
    expect(result.current.launcher.saveState).toBe('dirty');
    expect(result.current.shortcuts.saveState).toBe('dirty');
    expect(onLauncherCommitted).not.toHaveBeenCalled();
  });

  it('恢复默认会提交默认触发配置并同步镜像', async () => {
    const onLauncherCommitted = vi.fn();
    const { result } = renderHook(() => useShortcutsSettings({ onLauncherCommitted }));

    act(() => { result.current.launcher.change('activation', 'middleMouse'); });
    await act(async () => { result.current.reset(); });

    expect(applyLauncherTriggerSettingsMock).toHaveBeenCalledWith(DEFAULT_LAUNCHER_SETTINGS);
    expect(JSON.parse(localStorage.getItem(LAUNCHER_SETTINGS_STORAGE_KEY) || '{}')).toMatchObject({
      activation: 'keyboardOrMiddleMouse',
    });
    expect(onLauncherCommitted).toHaveBeenCalledWith({
      enabled: true,
      trigger: 'keyboard_or_mouse_middle',
      keyboard_shortcut: 'Ctrl+Alt+Space',
      excluded_apps: [],
    });
    expect(result.current.launcher.saveState).toBe('saved');
  });

  it('syncFromTrigger 把持久化触发配置同步进草稿并过滤空排除项', async () => {
    const { result } = renderHook(() => useShortcutsSettings());

    act(() => {
      result.current.launcher.syncFromTrigger({
        enabled: true,
        trigger: 'mouse_middle',
        keyboard_shortcut: 'Ctrl+Alt+L',
        excluded_apps: ['Code.exe', ''],
      });
    });

    expect(result.current.launcher.settings).toEqual({
      activation: 'middleMouse',
      keyboardShortcut: 'Ctrl+Alt+L',
      excludedApps: [{ id: 'code.exe', executable: 'Code.exe' }],
    });
  });

  it('Launcher 快捷键与应用快捷键冲突时给出提示', async () => {
    const { result } = renderHook(() => useShortcutsSettings());
    expect(result.current.launcherShortcutConflict).toBeUndefined();

    act(() => { result.current.launcher.change('keyboardShortcut', 'Ctrl+S'); });

    expect(result.current.launcherShortcutConflict).toBe('立即保存');
  });
});
