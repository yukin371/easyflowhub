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

/** Broadcast changes between multiple React roots/windows sharing a webview. */
export const APPEARANCE_CHANGED_EVENT = 'easyflowhub:appearance-changed';

function broadcastAppearanceConfig(config: AppearanceConfig): void {
  if (typeof window === 'undefined') return;
  window.dispatchEvent(new CustomEvent<AppearanceConfig>(APPEARANCE_CHANGED_EVENT, { detail: config }));
}

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
  // Keep the system preference in state so the `system` mode reacts when
  // Windows switches between light and dark while EasyFlowHub is open.
  const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return 'light';
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  });

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

  // Manager mounts more than one appearance-aware component, and Tauri may
  // host several windows at once. Keep hook instances synchronized instead of
  // allowing a stale instance to overwrite a freshly selected theme.
  useEffect(() => {
    if (typeof window === 'undefined') return;

    const handleAppearanceChanged = (event: Event) => {
      const detail = (event as CustomEvent<AppearanceConfig>).detail;
      if (detail?.theme && detail.window && detail.font) {
        setConfig(detail);
        setIsLoaded(true);
      }
    };
    const handleStorage = (event: StorageEvent) => {
      if (event.key !== 'easyflowhub-appearance') return;
      void loadAppearanceConfig().then((next) => {
        setConfig(next);
        setIsLoaded(true);
      });
    };

    window.addEventListener(APPEARANCE_CHANGED_EVENT, handleAppearanceChanged);
    window.addEventListener('storage', handleStorage);
    return () => {
      window.removeEventListener(APPEARANCE_CHANGED_EVENT, handleAppearanceChanged);
      window.removeEventListener('storage', handleStorage);
    };
  }, []);

  // 监听系统主题变化（当 theme 为 system 时）
  useEffect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return;
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = (event: MediaQueryListEvent) => setSystemTheme(event.matches ? 'dark' : 'light');

    // Match the current value even when the preference changed before this
    // effect was attached (for example while the settings window was closed).
    setSystemTheme(mediaQuery.matches ? 'dark' : 'light');

    if (mediaQuery.addEventListener) {
      mediaQuery.addEventListener('change', handler);
      return () => mediaQuery.removeEventListener('change', handler);
    }

    // Safari/WebView fallback for older MediaQueryList implementations.
    mediaQuery.addListener(handler);
    return () => mediaQuery.removeListener(handler);
  }, []);

  // 计算实际主题
  const resolvedTheme = useMemo((): 'light' | 'dark' => {
    if (!config) return 'dark';
    if (config.theme === 'system') {
      return systemTheme;
    }
    return config.theme;
  }, [config, systemTheme]);

  // Every window uses the same persisted appearance. Applying the resolved
  // theme here keeps launcher, notes, widgets and manager in sync even when
  // they are mounted in separate Tauri windows.
  useEffect(() => {
    if (!isLoaded) return;
    const root = document.documentElement;
    root.dataset.theme = resolvedTheme;
    root.dataset.blur = config?.window.isBlurEnabled === false ? 'off' : 'on';
    root.style.colorScheme = resolvedTheme;
  }, [config?.window.isBlurEnabled, isLoaded, resolvedTheme]);

  // 设置主题
  const setTheme = useCallback((theme: ThemeMode) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const next = { ...prev, theme };
      saveAppearanceConfig(next);
      broadcastAppearanceConfig(next);
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
      broadcastAppearanceConfig(next);

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
      broadcastAppearanceConfig(next);
      return next;
    });
  }, []);

  // 重置配置
  const reset = useCallback(() => {
    const defaultConfig = resetAppearanceConfig();
    broadcastAppearanceConfig(defaultConfig);
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
