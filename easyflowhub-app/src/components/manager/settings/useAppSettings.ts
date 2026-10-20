/**
 * AppSettings 域：加载 / 草稿 / 提交。
 *
 * 普通设置聚合（quick_note / trash / editor / todo / translation /
 * transcription / developer）只有这里一个提交入口 save()，
 * 底层复用 typed bridge lib/tauri/settings，不在组件里散落 invoke。
 *
 * F08 不变量（普通保存不夹带 Launcher 草稿）：
 * save() 只回写 settings.launcher_trigger 这份“已提交镜像”
 * （初始为加载值，Launcher 自己提交成功后由 syncLauncherTrigger 刷新），
 * 永远不会读取快捷键区仍在编辑的 Launcher 草稿。
 * Launcher 草稿只能通过它自己的提交入口
 * （快捷键区“保存配置” -> apply_launcher_trigger_settings）落盘。
 */
import { useCallback, useState } from 'react';
import { getSettings, updateSettings } from '../../../lib/tauri/settings';
import { setDebugMode } from '../../../lib/tauri/logging';
import {
  DEFAULT_SETTINGS,
  type AppSettings,
  type LauncherTriggerSettings,
} from '../../../types/settings';

export type SettingsSaveState = 'saved' | 'dirty';
export type SettingsLoadState = 'loading' | 'ready' | 'failed';

/** 与原 SettingsPanel.loadSettings 相同的默认值补齐策略。 */
function mergeWithDefaults(settings: AppSettings): AppSettings {
  return {
    ...DEFAULT_SETTINGS,
    ...settings,
    translation: { ...DEFAULT_SETTINGS.translation, ...settings.translation },
    transcription: { ...DEFAULT_SETTINGS.transcription, ...settings.transcription },
    developer: { ...DEFAULT_SETTINGS.developer, ...settings.developer },
  };
}

export function useAppSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [saveState, setSaveState] = useState<SettingsSaveState>('saved');
  const [error, setError] = useState<string>();
  // Loading the persisted settings can fail (DB locked, native bridge down).
  // Saving while the form still holds defaults would overwrite the user's real
  // configuration, so saves stay blocked until a load succeeds.
  const [loadState, setLoadState] = useState<SettingsLoadState>('loading');

  /**
   * 重新加载持久化设置。加载成功时通过 onLoaded 回传补齐默认值后的结果，
   * 供面板把持久化的 launcher_trigger 同步进 Launcher 草稿。
   */
  const reload = useCallback((onLoaded?: (settings: AppSettings) => void) => {
    setLoadState('loading');
    getSettings().then((settings) => {
      const merged = mergeWithDefaults(settings);
      setSettings(merged);
      onLoaded?.(merged);
      setLoadState('ready');
    }).catch((error) => {
      console.error('Failed to load settings:', error);
      setLoadState('failed');
    });
  }, []);

  /** 修改某一设置域的草稿并标记未保存。 */
  const patch = <K extends keyof AppSettings>(category: K, value: Partial<AppSettings[K]>) => {
    setSettings((prev) => ({
      ...prev,
      [category]: { ...prev[category], ...value },
    }));
    setSaveState('dirty');
  };

  /**
   * Launcher 域提交成功后刷新镜像。普通保存随后回写的
   * launcher_trigger 即为最近一次已提交值，不会覆盖 Launcher 提交结果。
   */
  const syncLauncherTrigger = useCallback((trigger: LauncherTriggerSettings) => {
    setSettings((prev) => ({ ...prev, launcher_trigger: trigger }));
  }, []);

  /** 普通设置聚合的唯一提交入口。 */
  const save = async () => {
    setError(undefined);
    if (loadState !== 'ready') {
      setError('设置尚未成功加载，已阻止保存以避免覆盖现有配置。请先点击“重试加载”。');
      return;
    }
    try {
      // launcher_trigger 保持已提交镜像（展开自 settings），不读取 Launcher 草稿。
      const next: AppSettings = {
        ...settings,
        translation: { ...settings.translation, model: settings.translation.model.trim() },
        transcription: { ...settings.transcription, model: settings.transcription.model.trim() },
      };
      await updateSettings(next);
      // Apply the debug flag immediately so logging starts before the next restart.
      await setDebugMode(next.developer.debug_mode).catch(() => undefined);
      setSettings(next);
      setSaveState('saved');
    } catch (err) {
      console.error('Failed to save settings:', err);
      setError('设置保存失败，请重试。');
    }
  };

  return {
    settings,
    saveState,
    loadState,
    error,
    reload,
    save,
    patch,
    syncLauncherTrigger,
    setError,
  };
}

export type AppSettingsController = ReturnType<typeof useAppSettings>;
