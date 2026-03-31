// src/lib/tauri/appearance.ts
import { invoke } from '@tauri-apps/api/core';
import type { AppearanceConfig } from '../../types/appearance';
import { DEFAULT_APPEARANCE } from '../../types/appearance';

const STORAGE_KEY = 'easyflowhub-appearance';

/**
 * 从本地存储加载外观配置
 */
export async function loadAppearanceConfig(): Promise<AppearanceConfig> {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      // 深度合并，确保新增字段有默认值
      return {
        theme: parsed.theme ?? DEFAULT_APPEARANCE.theme,
        window: { ...DEFAULT_APPEARANCE.window, ...parsed.window },
        font: { ...DEFAULT_APPEARANCE.font, ...parsed.font },
      };
    }
  } catch (error) {
    console.error('Failed to load appearance config:', error);
  }
  return { ...DEFAULT_APPEARANCE };
}

/**
 * 保存外观配置到本地存储
 */
export function saveAppearanceConfig(config: AppearanceConfig): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  } catch (error) {
    console.error('Failed to save appearance config:', error);
  }
}

/**
 * 重置外观配置为默认值
 */
export function resetAppearanceConfig(): AppearanceConfig {
  localStorage.removeItem(STORAGE_KEY);
  return { ...DEFAULT_APPEARANCE };
}

/**
 * 应用窗口置顶设置（Tauri 后端）
 */
export async function applyAlwaysOnTop(enabled: boolean): Promise<boolean> {
  try {
    return await invoke<boolean>('set_always_on_top', { enabled });
  } catch (error) {
    console.error('Failed to apply always on top:', error);
    throw error;
  }
}

/**
 * 应用窗口透明度（通过 CSS 实现）
 * Tauri 2 不支持运行时修改窗口透明度，透明度需在窗口创建时设置
 * 此函数通过修改 CSS 变量实现视觉透明效果
 */
export async function applyWindowOpacity(opacity: number): Promise<void> {
  try {
    // 通过 CSS 变量控制视觉透明度
    document.documentElement.style.setProperty('--window-opacity', String(opacity));
    // 同时调用后端命令（占位实现，保持 API 兼容性）
    await invoke('set_window_opacity', { opacity });
  } catch (error) {
    console.error('Failed to apply window opacity:', error);
    throw error;
  }
}

/**
 * 获取当前窗口状态
 */
export async function getWindowState(): Promise<{ isAlwaysOnTop: boolean }> {
  try {
    return await invoke('get_window_state');
  } catch (error) {
    console.error('Failed to get window state:', error);
    return { isAlwaysOnTop: false };
  }
}
