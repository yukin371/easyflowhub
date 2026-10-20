/**
 * 快捷键区组合入口：把「快捷键配置」与「Launcher 触发」两个域的
 * 加载 / 草稿 / 提交编排在一起（原 SettingsPanel 的
 * handleSaveShortcuts / handleResetShortcuts）。
 *
 * 每个域仍只有一个提交入口：
 * - Launcher 触发：save() / reset() 内的 applyToNative -> persistToStorage；
 * - 快捷键配置：save() / reset() 内的 persistToStorage（localStorage）。
 * 提交顺序与原实现一致：先应用 Launcher（失败即中止、两域都不落盘），
 * 再写快捷键 localStorage、Launcher UI 镜像，最后标记已保存。
 *
 * F08 不变量：Launcher 提交成功后通过 onLauncherCommitted 通知
 * AppSettings 聚合刷新 launcher_trigger 镜像，保证之后的普通保存
 * 回写的是已提交值而不是旧值或草稿。
 */
import { useMemo } from 'react';
import { DEFAULT_LAUNCHER_SETTINGS, type LauncherSettings } from '../../../types/launcherSettings';
import type { LauncherTriggerSettings } from '../../../types/settings';
import { DEFAULT_SHORTCUT_CONFIG, type ShortcutConfig } from '../../../types/shortcut';
import { toLauncherTriggerSettings, useLauncherSettings, type LauncherSettingsController } from './useLauncherSettings';
import { SHORTCUT_FIELDS, useShortcutSettings, type ShortcutSettingsController } from './useShortcutSettings';

export interface UseShortcutsSettingsOptions {
  /** Launcher 提交成功后同步普通设置聚合中的 launcher_trigger 镜像（F08）。 */
  onLauncherCommitted?: (trigger: LauncherTriggerSettings) => void;
}

export function useShortcutsSettings(options: UseShortcutsSettingsOptions = {}) {
  const launcher: LauncherSettingsController = useLauncherSettings();
  const shortcuts: ShortcutSettingsController = useShortcutSettings();

  const launcherShortcutConflict = useMemo(() => {
    const shortcut = launcher.settings.keyboardShortcut.trim().toLowerCase();
    if (!shortcut) return undefined;
    const conflict = SHORTCUT_FIELDS.find(
      ({ key }) => shortcuts.config[key].trim().toLowerCase() === shortcut
    );
    return conflict?.label;
  }, [launcher.settings.keyboardShortcut, shortcuts.config]);

  // 提交目标显式传递：恢复默认时 state 尚未刷新，不能依赖闭包旧值。
  const commit = async (
    launcherTarget: LauncherSettings,
    shortcutTarget: ShortcutConfig,
    failureFallback: string,
    logPrefix: string,
  ) => {
    try {
      await launcher.applyToNative(launcherTarget);
      shortcuts.persistToStorage(shortcutTarget);
      launcher.persistToStorage(launcherTarget);
      shortcuts.markSaved();
      launcher.markSaved();
      options.onLauncherCommitted?.(toLauncherTriggerSettings(launcherTarget));
    } catch (error) {
      console.error(logPrefix, error);
      launcher.markFailed(error instanceof Error ? error.message : failureFallback);
    }
  };

  /** 快捷键配置 + Launcher 触发的提交入口（保存配置按钮）。 */
  const save = async () => {
    await commit(
      launcher.settings,
      shortcuts.config,
      'Launcher 触发方式无法应用，请更换快捷键。',
      'Failed to apply Launcher trigger:',
    );
  };

  /** 两域恢复默认值并提交（恢复默认按钮）。 */
  const reset = async () => {
    shortcuts.resetToDefaults();
    launcher.resetToDefaults();
    await commit(
      DEFAULT_LAUNCHER_SETTINGS,
      DEFAULT_SHORTCUT_CONFIG,
      'Launcher 默认触发方式无法应用。',
      'Failed to reset Launcher trigger:',
    );
  };

  return {
    launcher,
    shortcuts,
    launcherShortcutConflict,
    save,
    reset,
  };
}

export type ShortcutsSectionController = ReturnType<typeof useShortcutsSettings>;
