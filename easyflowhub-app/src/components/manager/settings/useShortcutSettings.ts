/**
 * 快捷键配置域（localStorage）：加载 / 草稿 / 落盘原语。
 *
 * 该域只通过 types/shortcut 的 loadShortcutConfig / SHORTCUT_STORAGE_KEY
 * 持久化；提交顺序由快捷键区组合入口（useShortcutsSettings）统一编排。
 */
import { useEffect, useMemo, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';
import {
  DEFAULT_SHORTCUT_CONFIG,
  formatShortcutFromKeyboardEvent,
  loadShortcutConfig,
  SHORTCUT_STORAGE_KEY,
  type ShortcutConfig,
} from '../../../types/shortcut';


export interface ShortcutField {
  key: keyof ShortcutConfig;
  label: string;
  description: string;
}

export const SHORTCUT_FIELDS: ShortcutField[] = [
  { key: 'save', label: '立即保存', description: '在编辑态直接执行一次保存，不等待防抖。' },
  { key: 'close', label: '退出编辑', description: '离开当前编辑态并触发必要的收尾保存。' },
  { key: 'quickSwitcher', label: '唤起切换器', description: '在快速笔记里呼出最近笔记切换面板。' },
  { key: 'toggleAlwaysOnTop', label: '切换置顶', description: '快速笔记窗口在置顶和取消置顶之间切换。' },
  { key: 'cutLine', label: '剪切行', description: '编辑器命令，默认避开系统 Ctrl+X。' },
  { key: 'deleteLine', label: '删除行', description: '编辑器命令，和关闭当前快速笔记分离。' },
  { key: 'duplicateLine', label: '复制行', description: '复制当前行到下一行。' },
  { key: 'insertLineBelow', label: '下方插入', description: '在当前行下方插入空行。' },
  { key: 'insertLineAbove', label: '上方插入', description: '在当前行上方插入空行。' },
  { key: 'heading1', label: '一级标题', description: '将当前行或选区设为一级标题。' },
  { key: 'heading2', label: '二级标题', description: '将当前行或选区设为二级标题。' },
  { key: 'heading3', label: '三级标题', description: '将当前行或选区设为三级标题。' },
  { key: 'clearHeading', label: '清除标题', description: '移除当前行或选区的标题前缀。' },
];

export function useShortcutSettings() {
  const [config, setConfig] = useState<ShortcutConfig>(DEFAULT_SHORTCUT_CONFIG);
  const [saveState, setSaveState] = useState<'saved' | 'dirty'>('saved');
  const [recordingKey, setRecordingKey] = useState<keyof ShortcutConfig | null>(null);

  useEffect(() => {
    setConfig(loadShortcutConfig());
  }, []);

  const change = (key: keyof ShortcutConfig, value: string) => {
    setConfig((prev) => ({ ...prev, [key]: value }));
    setSaveState('dirty');
  };

  const toggleRecording = (key: keyof ShortcutConfig) => {
    setRecordingKey((prev) => (prev === key ? null : key));
  };

  const handleRecordKeyDown = (key: keyof ShortcutConfig, event: ReactKeyboardEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === 'Escape' && recordingKey === key) {
      setRecordingKey(null);
      return;
    }

    const shortcut = formatShortcutFromKeyboardEvent(event.nativeEvent);
    if (!shortcut) {
      return;
    }

    change(key, shortcut);
    setRecordingKey(null);
  };

  /** 快捷键域的唯一落盘入口（localStorage）；恢复默认时直接传入默认值。 */
  const persistToStorage = (target: ShortcutConfig = config) => {
    localStorage.setItem(SHORTCUT_STORAGE_KEY, JSON.stringify(target));
  };

  const markSaved = () => setSaveState('saved');

  const resetToDefaults = () => setConfig(DEFAULT_SHORTCUT_CONFIG);

  const changedCount = useMemo(
    () =>
      SHORTCUT_FIELDS.filter(
        ({ key }) => config[key].trim() !== DEFAULT_SHORTCUT_CONFIG[key].trim()
      ).length,
    [config]
  );

  return {
    config,
    saveState,
    recordingKey,
    changedCount,
    change,
    toggleRecording,
    handleRecordKeyDown,
    persistToStorage,
    markSaved,
    resetToDefaults,
  };
}

export type ShortcutSettingsController = ReturnType<typeof useShortcutSettings>;
