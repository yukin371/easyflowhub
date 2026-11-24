/**
 * Launcher 触发设置域：加载 / 草稿 / 提交原语。
 *
 * 该域的落盘只经过两条路径：
 * - 运行态 + native 持久化：applyToNative() -> lib/tauri/launcher
 *   的 apply_launcher_trigger_settings（native 内先注册再保存，
 *   见 src-tauri/src/lib.rs 的 apply_launcher_trigger_settings）；
 * - UI 镜像：persistToStorage() -> types/launcherSettings 的
 *   saveLauncherSettings（localStorage）。
 *
 * 提交顺序由快捷键区的组合入口（useShortcutsSettings）统一编排，
 * 这里不提供第二个保存按钮。
 */
import { useCallback, useEffect, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';
import {
  createExcludedApp,
  DEFAULT_LAUNCHER_SETTINGS,
  loadLauncherSettings,
  saveLauncherSettings,
  type LauncherActivation,
  type LauncherSettings,
} from '../../../types/launcherSettings';
import type { LauncherTriggerSettings } from '../../../types/settings';
import { formatShortcutFromKeyboardEvent } from '../../../types/shortcut';
import {
  applyLauncherTriggerSettings,
  getConfiguredSnowShotPath,
  saveConfiguredSnowShotPath,
} from '../../../lib/tauri/launcher';

/** 已提交的 UI 配置 -> native launcher_trigger 持久化形态。 */
export function toLauncherTriggerSettings(settings: LauncherSettings): LauncherTriggerSettings {
  return {
    enabled: true,
    trigger: settings.activation === 'keyboard'
      ? 'keyboard'
      : settings.activation === 'middleMouse'
        ? 'mouse_middle'
        : 'keyboard_or_mouse_middle',
    keyboard_shortcut: settings.keyboardShortcut,
    excluded_apps: settings.excludedApps.map((app) => app.executable),
  };
}

/**
 * 「自动避开冲突」的候选组合，按被第三方软件占用的概率从低到高排列。
 * 刻意避开 QQ 截图 / 微信 / 钉钉 / 输入法的常见默认键位。
 */
const LAUNCHER_SHORTCUT_CANDIDATES = [
  'Ctrl+Shift+Space',
  'Ctrl+Alt+K',
  'Ctrl+Alt+J',
  'Ctrl+Shift+F10',
  'Alt+Shift+Space',
];

export function useLauncherSettings() {
  const [settings, setSettings] = useState<LauncherSettings>(DEFAULT_LAUNCHER_SETTINGS);
  const [saveState, setSaveState] = useState<'saved' | 'dirty'>('saved');
  const [error, setError] = useState<string>();
  const [recording, setRecording] = useState(false);
  const [excludedAppInput, setExcludedAppInput] = useState('');
  const [snowShotPath, setSnowShotPath] = useState('');
  const [snowShotPathSaveState, setSnowShotPathSaveState] = useState<'saved' | 'dirty'>('saved');
  const [autoResolving, setAutoResolving] = useState(false);
  const [autoResolveNote, setAutoResolveNote] = useState<string>();

  useEffect(() => {
    setSettings(loadLauncherSettings());
    setSnowShotPath(getConfiguredSnowShotPath() ?? '');
  }, []);

  const change = <K extends keyof LauncherSettings>(key: K, value: LauncherSettings[K]) => {
    setSettings((previous) => ({ ...previous, [key]: value }));
    setSaveState('dirty');
    setError(undefined);
  };

  const toggleRecording = () => setRecording((previous) => !previous);

  const handleRecordKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === 'Escape') {
      setRecording(false);
      return;
    }

    const shortcut = formatShortcutFromKeyboardEvent(event.nativeEvent);
    if (!shortcut) return;

    change('keyboardShortcut', shortcut);
    setRecording(false);
  };

  const addExcludedApp = () => {
    const app = createExcludedApp(excludedAppInput);
    if (!app) return;
    setSettings((previous) => {
      if (previous.excludedApps.some((existing) => existing.id === app.id)) return previous;
      return { ...previous, excludedApps: [...previous.excludedApps, app] };
    });
    setSaveState('dirty');
    setExcludedAppInput('');
  };

  const removeExcludedApp = (id: string) => {
    setSettings((previous) => ({
      ...previous,
      excludedApps: previous.excludedApps.filter((app) => app.id !== id),
    }));
    setSaveState('dirty');
  };

  const updateSnowShotPath = (value: string) => {
    setSnowShotPath(value);
    setSnowShotPathSaveState('dirty');
  };

  const saveSnowShotPath = () => {
    saveConfiguredSnowShotPath(snowShotPath);
    setSnowShotPath((current) => current.trim());
    setSnowShotPathSaveState('saved');
  };

  /**
   * Launcher 域的运行态提交：应用到 native 全局输入层（快捷键冲突时抛出）。
   * 恢复默认等场景直接传入目标值，避免依赖尚未刷新的 state 闭包。
   */
  const applyToNative = async (target: LauncherSettings = settings) => {
    await applyLauncherTriggerSettings(target);
  };

  /** Launcher 域的 UI 镜像落盘（localStorage，供下次打开预填）。 */
  const persistToStorage = (target: LauncherSettings = settings) => {
    saveLauncherSettings(target);
  };

  const markSaved = () => setSaveState('saved');

  /** 提交失败：保持草稿未保存并展示失败原因。 */
  const markFailed = (message: string) => {
    setSaveState('dirty');
    setError(message);
  };

  /**
   * 冲突自愈：按「被占用概率从低到高」依次尝试候选组合。
   *
   * Windows 的 RegisterHotKey 是先到先得的独占注册，native 侧注册失败会
   * 直接抛错（`apply_launcher_trigger_settings` 先注册再落盘），因此这里
   * 不需要额外的探测接口：apply 成功即代表该组合可用，失败就试下一个。
   * 每次失败都不会影响已生效的旧组合（native 只在新组合注册成功后才替换）。
   */
  const autoResolveConflict = async () => {
    setAutoResolving(true);
    setAutoResolveNote(undefined);
    try {
      for (const candidate of LAUNCHER_SHORTCUT_CANDIDATES) {
        if (candidate === settings.keyboardShortcut) continue;
        const next: LauncherSettings = { ...settings, keyboardShortcut: candidate };
        try {
          await applyToNative(next);
        } catch {
          continue;
        }
        setSettings(next);
        persistToStorage(next);
        setSaveState('saved');
        setError(undefined);
        setAutoResolveNote(`已切换到 ${candidate}`);
        return candidate;
      }
      setSaveState('dirty');
      setError('候选组合均被占用。请手动录入其他组合，或在占用该组合的软件中解绑。');
      return null;
    } finally {
      setAutoResolving(false);
    }
  };

  const resetToDefaults = () => setSettings(DEFAULT_LAUNCHER_SETTINGS);

  /**
   * 把持久化的 launcher_trigger 同步进草稿（仅加载/重试加载时调用）。
   * 与原 SettingsPanel.loadSettings 的转换逻辑一致。
   */
  const syncFromTrigger = useCallback((trigger: LauncherTriggerSettings | undefined) => {
    if (!trigger) return;
    const activation: LauncherActivation = trigger.trigger === 'keyboard'
      ? 'keyboard'
      : trigger.trigger === 'mouse_middle'
        ? 'middleMouse'
        : 'keyboardOrMiddleMouse';
    setSettings({
      activation,
      keyboardShortcut: trigger.keyboard_shortcut,
      excludedApps: trigger.excluded_apps
        .map((executable) => createExcludedApp(executable))
        .filter((app): app is NonNullable<ReturnType<typeof createExcludedApp>> => Boolean(app)),
    });
  }, []);

  return {
    settings,
    saveState,
    error,
    recording,
    excludedAppInput,
    snowShotPath,
    snowShotPathSaveState,
    change,
    toggleRecording,
    handleRecordKeyDown,
    setExcludedAppInput,
    addExcludedApp,
    removeExcludedApp,
    updateSnowShotPath,
    saveSnowShotPath,
    applyToNative,
    persistToStorage,
    markSaved,
    markFailed,
    autoResolveConflict,
    autoResolving,
    autoResolveNote,
    resetToDefaults,
    syncFromTrigger,
  };
}

export type LauncherSettingsController = ReturnType<typeof useLauncherSettings>;
