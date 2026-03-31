// src/hooks/useAppearance.ts
import { useState, useEffect, useCallback, useMemo } from 'react';
import {
  loadAppearanceConfig,
  saveAppearanceConfig,
  resetAppearanceConfig,
  applyAlwaysOnTop,
  applyWindowOpacity,
} from '../lib/tauri/appearance';
import type {
  AppearanceConfig,
  ThemeMode,
  WindowAppearance,
  FontSettings,
} from '../types/appearance';
import { DEFAULT_APPEARANCE } from '../types/appearance';

export interface UseAppearanceReturn {
  /** 当前配置 */
  config: AppearanceConfig | null;
  /** 是否已加载完成 */
  isLoaded: boolean;
  /** 解析后的实际主题（处理 system 模式） */
  resolvedTheme: 'light' | 'dark';
  /** 设置主题 */
  setTheme: (theme: ThemeMode) => void;
  /** 设置窗口外观 */
  setWindowAppearance: (updates: Partial<WindowAppearance>) => void;
  /** 设置字体 */
  setFontSettings: (updates: Partial<FontSettings>) => void;
  /** 重置为默认配置 */
  reset: () => void;
  /** 更新单个窗口设置 */
  updateOpacity: (opacity: number) => void;
  updateCornerRadius: (radius: number) => void;
  updateAlwaysOnTop: (enabled: boolean) => void;
  updateBlur: (enabled: boolean) => void;
}

export function useAppearance(): UseAppearanceReturn {
  const [config, setConfig] = useState<AppearanceConfig | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);

  // 初始化加载配置
  useEffect(() => {
    loadAppearanceConfig()
      .then((loaded) => {
        setConfig(loaded);
        setIsLoaded(true);
      })
      .catch((error) => {
        console.error('Failed to load appearance config:', error);
        setConfig(DEFAULT_APPEARANCE);
        setIsLoaded(true);
      });
  }, []);

  // 监听系统主题变化（当 theme 为 system 时）
  useEffect(() => {
    if (config?.theme !== 'system') return;

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = () => {
      // 触发重新渲染即可，resolvedTheme 会自动计算
    };

    mediaQuery.addEventListener('change', handler);
    return () => mediaQuery.removeEventListener('change', handler);
  }, [config?.theme]);

  // 计算实际主题
  const resolvedTheme = useMemo((): 'light' | 'dark' => {
    if (!config) return 'dark';
    if (config.theme === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return config.theme;
  }, [config]);

  // 设置主题
  const setTheme = useCallback((theme: ThemeMode) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const next = { ...prev, theme };
      saveAppearanceConfig(next);
      return next;
    });
  }, []);

  // 设置窗口外观
  const setWindowAppearance = useCallback((updates: Partial<WindowAppearance>) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        window: { ...prev.window, ...updates },
      };
      saveAppearanceConfig(next);

      // 同步到后端
      if (updates.isAlwaysOnTop !== undefined) {
        applyAlwaysOnTop(updates.isAlwaysOnTop).catch(console.error);
      }
      if (updates.opacity !== undefined) {
        applyWindowOpacity(updates.opacity).catch(console.error);
      }

      return next;
    });
  }, []);

  // 设置字体
  const setFontSettings = useCallback((updates: Partial<FontSettings>) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        font: { ...prev.font, ...updates },
      };
      saveAppearanceConfig(next);
      return next;
    });
  }, []);

  // 重置配置
  const reset = useCallback(() => {
    const defaultConfig = resetAppearanceConfig();
    setConfig(defaultConfig);
  }, []);

  // 便捷更新方法
  const updateOpacity = useCallback((opacity: number) => {
    setWindowAppearance({ opacity });
  }, [setWindowAppearance]);

  const updateCornerRadius = useCallback((cornerRadius: number) => {
    setWindowAppearance({ cornerRadius });
  }, [setWindowAppearance]);

  const updateAlwaysOnTop = useCallback((isAlwaysOnTop: boolean) => {
    setWindowAppearance({ isAlwaysOnTop });
  }, [setWindowAppearance]);

  const updateBlur = useCallback((isBlurEnabled: boolean) => {
    setWindowAppearance({ isBlurEnabled });
  }, [setWindowAppearance]);

  return {
    config,
    isLoaded,
    resolvedTheme,
    setTheme,
    setWindowAppearance,
    setFontSettings,
    reset,
    updateOpacity,
    updateCornerRadius,
    updateAlwaysOnTop,
    updateBlur,
  };
}
