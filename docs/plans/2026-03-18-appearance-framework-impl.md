# 桌面美化可扩展框架 实现计划

> 该计划经过评估已经废弃。
> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 deskflow 构建可扩展的外观配置系统，支持主题、窗口外观、字体设置的统一管理。

**Architecture:** 分层架构 - 类型层定义契约，IPC层封装通信，Hook层管理状态，UI组件层负责展示。前端设置存储于 localStorage，窗口效果通过 Tauri invoke 与 Rust 后端同步。

**Tech Stack:** React 18, TypeScript 5, Tauri 2, Tailwind CSS

---

## Task 1: 类型层 - 定义外观配置类型

**Files:**
- Create: `src/types/appearance.ts`
- Modify: `src/types/index.ts`

**Step 1: 创建外观类型定义文件**

```typescript
// src/types/appearance.ts

/** 主题模式 */
export type ThemeMode = 'light' | 'dark' | 'system';

/** 窗口外观设置 */
export interface WindowAppearance {
  /** 窗口透明度 0-1 */
  opacity: number;
  /** 窗口圆角 px */
  cornerRadius: number;
  /** 窗口置顶 */
  isAlwaysOnTop: boolean;
  /** 模糊背景 */
  isBlurEnabled: boolean;
}

/** 字体设置 */
export interface FontSettings {
  /** 字体族 */
  fontFamily: string;
  /** 字体大小 px */
  fontSize: number;
  /** 行高 */
  lineHeight: number;
}

/** 完整外观配置 */
export interface AppearanceConfig {
  /** 主题模式 */
  theme: ThemeMode;
  /** 窗口外观 */
  window: WindowAppearance;
  /** 字体设置 */
  font: FontSettings;
}

/** 默认外观配置 */
export const DEFAULT_APPEARANCE: AppearanceConfig = {
  theme: 'dark',
  window: {
    opacity: 0.8,
    cornerRadius: 8,
    isAlwaysOnTop: true,
    isBlurEnabled: true,
  },
  font: {
    fontFamily: 'Consolas',
    fontSize: 14,
    lineHeight: 1.6,
  },
};

/** 可用的字体列表 */
export const AVAILABLE_FONTS = [
  'Consolas',
  'Monaco',
  'Courier New',
  'Segoe UI',
  'Arial',
  'Microsoft YaHei',
  'PingFang SC',
] as const;

/** 主题选项 */
export const THEME_OPTIONS: Array<{ id: ThemeMode; name: string; icon: string }> = [
  { id: 'dark', name: '深色', icon: '🌙' },
  { id: 'light', name: '浅色', icon: '☀️' },
  { id: 'system', name: '系统', icon: '⚙️' },
];
```

**Step 2: 更新类型导出**

在 `src/types/index.ts` 末尾添加：

```typescript
// 外观相关类型
export * from './appearance';
```

**Step 3: 验证类型编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 4: 提交**

```bash
git add src/types/appearance.ts src/types/index.ts
git commit -m "feat(types): add appearance configuration types"
```

---

## Task 2: IPC 层 - 封装外观配置通信

**Files:**
- Create: `src/lib/tauri/appearance.ts`
- Modify: `src/lib/tauri/index.ts`

**Step 1: 创建外观 IPC 封装**

```typescript
// src/lib/tauri/appearance.ts
import { invoke } from '@tauri-apps/api/core';
import type { AppearanceConfig } from '../../types/appearance';
import { DEFAULT_APPEARANCE } from '../../types/appearance';

const STORAGE_KEY = 'deskflow-appearance';

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
 * 应用窗口透明度（Tauri 后端）
 */
export async function applyWindowOpacity(opacity: number): Promise<void> {
  try {
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
```

**Step 2: 更新 IPC 导出**

在 `src/lib/tauri/index.ts` 添加：

```typescript
// 外观配置
export * from './appearance';
```

**Step 3: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 4: 提交**

```bash
git add src/lib/tauri/appearance.ts src/lib/tauri/index.ts
git commit -m "feat(ipc): add appearance configuration IPC layer"
```

---

## Task 3: Hook 层 - 外观状态管理

**Files:**
- Create: `src/hooks/useAppearance.ts`

**Step 1: 创建 useAppearance Hook**

```typescript
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
```

**Step 2: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 3: 提交**

```bash
git add src/hooks/useAppearance.ts
git commit -m "feat(hooks): add useAppearance state management hook"
```

---

## Task 4: UI 组件 - 主题选择区块

**Files:**
- Create: `src/components/beautification/ThemeSection.tsx`

**Step 1: 创建主题选择组件**

```typescript
// src/components/beautification/ThemeSection.tsx
import type { ThemeMode } from '../../types/appearance';
import { THEME_OPTIONS } from '../../types/appearance';

interface ThemeSectionProps {
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
}

export function ThemeSection({ theme, onThemeChange }: ThemeSectionProps) {
  return (
    <section>
      <h2 className="text-sm font-medium text-gray-300 mb-4">主题</h2>
      <div className="grid grid-cols-3 gap-3">
        {THEME_OPTIONS.map((option) => (
          <button
            key={option.id}
            onClick={() => onThemeChange(option.id)}
            className={`
              p-4 rounded-lg border-2 flex flex-col items-center gap-2 transition-all
              ${theme === option.id
                ? 'border-blue-500 bg-blue-900/20'
                : 'border-white/10 hover:border-white/20 bg-white/5'
              }
            `}
          >
            <span className="text-2xl">{option.icon}</span>
            <span className="text-sm text-gray-300">{option.name}</span>
          </button>
        ))}
      </div>
    </section>
  );
}
```

**Step 2: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 3: 提交**

```bash
git add src/components/beautification/ThemeSection.tsx
git commit -m "feat(ui): add ThemeSection component"
```

---

## Task 5: UI 组件 - 窗口外观区块

**Files:**
- Create: `src/components/beautification/WindowSection.tsx`

**Step 1: 创建窗口外观组件**

```typescript
// src/components/beautification/WindowSection.tsx
import type { WindowAppearance } from '../../types/appearance';

interface WindowSectionProps {
  window: WindowAppearance;
  onChange: (updates: Partial<WindowAppearance>) => void;
}

export function WindowSection({ window: windowConfig, onChange }: WindowSectionProps) {
  return (
    <section>
      <h2 className="text-sm font-medium text-gray-300 mb-4">窗口外观</h2>
      <div className="space-y-4">
        {/* 透明度 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">透明度</label>
            <span className="text-sm text-gray-400">{Math.round(windowConfig.opacity * 100)}%</span>
          </div>
          <input
            type="range"
            min="0.3"
            max="1"
            step="0.05"
            value={windowConfig.opacity}
            onChange={(e) => onChange({ opacity: parseFloat(e.target.value) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>

        {/* 圆角大小 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">圆角大小</label>
            <span className="text-sm text-gray-400">{windowConfig.cornerRadius}px</span>
          </div>
          <input
            type="range"
            min="0"
            max="20"
            step="1"
            value={windowConfig.cornerRadius}
            onChange={(e) => onChange({ cornerRadius: parseInt(e.target.value, 10) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>

        {/* 开关选项 */}
        <div className="space-y-2">
          <label className="flex items-center gap-3 text-sm text-gray-300 cursor-pointer">
            <input
              type="checkbox"
              checked={windowConfig.isAlwaysOnTop}
              onChange={(e) => onChange({ isAlwaysOnTop: e.target.checked })}
              className="w-4 h-4 rounded border-gray-600 bg-white/10 text-blue-600 focus:ring-blue-500"
            />
            窗口置顶
          </label>
          <label className="flex items-center gap-3 text-sm text-gray-300 cursor-pointer">
            <input
              type="checkbox"
              checked={windowConfig.isBlurEnabled}
              onChange={(e) => onChange({ isBlurEnabled: e.target.checked })}
              className="w-4 h-4 rounded border-gray-600 bg-white/10 text-blue-600 focus:ring-blue-500"
            />
            模糊背景
          </label>
        </div>
      </div>
    </section>
  );
}
```

**Step 2: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 3: 提交**

```bash
git add src/components/beautification/WindowSection.tsx
git commit -m "feat(ui): add WindowSection component"
```

---

## Task 6: UI 组件 - 字体设置区块

**Files:**
- Create: `src/components/beautification/FontSection.tsx`

**Step 1: 创建字体设置组件**

```typescript
// src/components/beautification/FontSection.tsx
import type { FontSettings } from '../../types/appearance';
import { AVAILABLE_FONTS } from '../../types/appearance';

interface FontSectionProps {
  font: FontSettings;
  onChange: (updates: Partial<FontSettings>) => void;
}

export function FontSection({ font, onChange }: FontSectionProps) {
  return (
    <section>
      <h2 className="text-sm font-medium text-gray-300 mb-4">字体设置</h2>
      <div className="space-y-4">
        {/* 字体选择 */}
        <div>
          <label className="block text-sm text-gray-400 mb-2">字体</label>
          <select
            value={font.fontFamily}
            onChange={(e) => onChange({ fontFamily: e.target.value })}
            className="w-full px-3 py-2 bg-white/5 border border-white/10 rounded-lg text-sm text-gray-200 focus:outline-none focus:border-blue-500"
          >
            {AVAILABLE_FONTS.map((fontName) => (
              <option key={fontName} value={fontName} style={{ fontFamily: fontName }}>
                {fontName}
              </option>
            ))}
          </select>
        </div>

        {/* 字体大小 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">字体大小</label>
            <span className="text-sm text-gray-400">{font.fontSize}px</span>
          </div>
          <input
            type="range"
            min="12"
            max="24"
            step="1"
            value={font.fontSize}
            onChange={(e) => onChange({ fontSize: parseInt(e.target.value, 10) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>

        {/* 行高 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">行高</label>
            <span className="text-sm text-gray-400">{font.lineHeight.toFixed(1)}</span>
          </div>
          <input
            type="range"
            min="1"
            max="2"
            step="0.1"
            value={font.lineHeight}
            onChange={(e) => onChange({ lineHeight: parseFloat(e.target.value) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>
      </div>
    </section>
  );
}
```

**Step 2: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 3: 提交**

```bash
git add src/components/beautification/FontSection.tsx
git commit -m "feat(ui): add FontSection component"
```

---

## Task 7: UI 组件 - 预览面板

**Files:**
- Create: `src/components/beautification/PreviewPanel.tsx`

**Step 1: 创建预览面板组件**

```typescript
// src/components/beautification/PreviewPanel.tsx
import type { AppearanceConfig } from '../../types/appearance';

interface PreviewPanelProps {
  config: AppearanceConfig;
  resolvedTheme: 'light' | 'dark';
}

export function PreviewPanel({ config, resolvedTheme }: PreviewPanelProps) {
  const { window: windowConfig, font } = config;

  const previewStyle: React.CSSProperties = {
    backgroundColor:
      resolvedTheme === 'dark'
        ? `rgba(17, 24, 39, ${windowConfig.opacity})`
        : `rgba(255, 255, 255, ${windowConfig.opacity})`,
    backdropFilter: windowConfig.isBlurEnabled ? 'blur(10px)' : 'none',
    borderRadius: `${windowConfig.cornerRadius}px`,
    color: resolvedTheme === 'dark' ? '#e5e7eb' : '#1f2937',
  };

  const editorStyle: React.CSSProperties = {
    fontFamily: font.fontFamily,
    fontSize: `${font.fontSize}px`,
    lineHeight: font.lineHeight,
  };

  return (
    <section>
      <h2 className="text-sm font-medium text-gray-300 mb-4">预览</h2>
      <div className="p-4 rounded-lg border border-white/10 bg-white/5">
        <div className="p-6 rounded-lg border border-white/10" style={previewStyle}>
          <div className="text-sm mb-2 opacity-70">窗口预览</div>
          <div className="p-4 bg-black/20 rounded" style={editorStyle}>
            <h3 className="font-medium mb-2">示例笔记</h3>
            <p className="opacity-70">
              这是一个示例笔记，用于预览字体和窗口设置效果。
              <br />
              字体: {font.fontFamily}
              <br />
              大小: {font.fontSize}px | 行高: {font.lineHeight.toFixed(1)}
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
```

**Step 2: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 3: 提交**

```bash
git add src/components/beautification/PreviewPanel.tsx
git commit -m "feat(ui): add PreviewPanel component"
```

---

## Task 8: UI 组件 - 页面容器与导出

**Files:**
- Create: `src/components/beautification/BeautificationPage.tsx`
- Create: `src/components/beautification/index.ts`

**Step 1: 创建页面容器组件**

```typescript
// src/components/beautification/BeautificationPage.tsx
import { useAppearance } from '../../hooks/useAppearance';
import { ThemeSection } from './ThemeSection';
import { WindowSection } from './WindowSection';
import { FontSection } from './FontSection';
import { PreviewPanel } from './PreviewPanel';

export function BeautificationPage() {
  const {
    config,
    isLoaded,
    resolvedTheme,
    setTheme,
    setWindowAppearance,
    setFontSettings,
    reset,
  } = useAppearance();

  if (!isLoaded || !config) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-900/80">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-blue-500 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-gray-900/80">
      {/* Header */}
      <header className="p-4 border-b border-white/10">
        <h1 className="text-lg font-semibold text-gray-200">桌面美化</h1>
      </header>

      {/* Main content */}
      <main className="flex-1 overflow-auto p-6 space-y-8">
        <ThemeSection theme={config.theme} onThemeChange={setTheme} />
        <WindowSection window={config.window} onChange={setWindowAppearance} />
        <FontSection font={config.font} onChange={setFontSettings} />
        <PreviewPanel config={config} resolvedTheme={resolvedTheme} />
      </main>

      {/* Action bar */}
      <footer className="p-4 border-t border-white/10 bg-black/30 flex items-center justify-end gap-3">
        <button
          onClick={reset}
          className="px-4 py-2 bg-white/10 text-gray-300 text-sm rounded hover:bg-white/15 transition-colors"
        >
          恢复默认
        </button>
      </footer>
    </div>
  );
}
```

**Step 2: 创建统一导出**

```typescript
// src/components/beautification/index.ts
export { BeautificationPage } from './BeautificationPage';
export { ThemeSection } from './ThemeSection';
export { WindowSection } from './WindowSection';
export { FontSection } from './FontSection';
export { PreviewPanel } from './PreviewPanel';
```

**Step 3: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 4: 提交**

```bash
git add src/components/beautification/
git commit -m "feat(ui): add BeautificationPage container and exports"
```

---

## Task 9: 集成 - 更新路由使用新组件

**Files:**
- Modify: `src/components/PageRouter.tsx`

**Step 1: 更新 PageRouter 使用新组件**

在 `PageRouter.tsx` 中，找到 `beautification` case，替换为：

```typescript
// 在文件顶部添加导入
import { BeautificationPage } from './beautification';

// 在 switch 语句中
case 'beautification':
  return <BeautificationPage />;
```

**Step 2: 删除旧组件（可选）**

如果旧的 `DesktopBeautificationPage.tsx` 不再需要：

```bash
rm src/components/DesktopBeautificationPage.tsx
```

**Step 3: 验证编译**

```bash
cd deskflow/deskflow-app && npx tsc --noEmit
```

Expected: 无类型错误

**Step 4: 提交**

```bash
git add src/components/PageRouter.tsx
git rm src/components/DesktopBeautificationPage.tsx  # 如果删除
git commit -m "refactor: integrate new BeautificationPage into router"
```

---

## Task 10: Rust 后端 - 窗口控制命令

**Files:**
- Create: `src-tauri/src/appearance.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建 Rust 命令模块**

```rust
// src-tauri/src/appearance.rs
use tauri::{command, AppHandle, Manager};

/// 设置窗口置顶
#[command]
pub async fn set_always_on_top(app: AppHandle, enabled: bool) -> Result<bool, String> {
    // 获取主窗口
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(enabled)
            .map_err(|e| format!("Failed to set always on top: {}", e))?;
        Ok(enabled)
    } else {
        // 尝试获取当前焦点窗口
        if let Some(window) = app.get_focused_window() {
            window
                .set_always_on_top(enabled)
                .map_err(|e| format!("Failed to set always on top: {}", e))?;
            Ok(enabled)
        } else {
            Err("No window found".to_string())
        }
    }
}

/// 设置窗口透明度
#[command]
pub async fn set_window_opacity(app: AppHandle, opacity: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_opacity(opacity)
            .map_err(|e| format!("Failed to set opacity: {}", e))?;
    }
    Ok(())
}

/// 获取窗口状态
#[command]
pub async fn get_window_state(app: AppHandle) -> Result<WindowState, String> {
    if let Some(window) = app.get_webview_window("main") {
        let is_always_on_top = window
            .is_always_on_top()
            .map_err(|e| format!("Failed to get always on top state: {}", e))?;
        Ok(WindowState { is_always_on_top })
    } else {
        Ok(WindowState {
            is_always_on_top: false,
        })
    }
}

/// 窗口状态
#[derive(serde::Serialize)]
pub struct WindowState {
    pub is_always_on_top: bool,
}
```

**Step 2: 在 lib.rs 中注册命令**

```rust
// 在 lib.rs 中添加模块声明
mod appearance;

// 在 invoke_handler 中注册命令
.invoke_handler(tauri::generate_handler![
    // ... 现有命令 ...
    appearance::set_always_on_top,
    appearance::set_window_opacity,
    appearance::get_window_state,
])
```

**Step 3: 验证 Rust 编译**

```bash
cd deskflow/deskflow-app/src-tauri && cargo check
```

Expected: 无编译错误

**Step 4: 提交**

```bash
git add src-tauri/src/appearance.rs src-tauri/src/lib.rs
git commit -m "feat(rust): add window appearance commands"
```

---

## Task 11: 集成测试与验证

**Step 1: 启动开发服务器**

```bash
cd deskflow/deskflow-app && npm run tauri dev
```

**Step 2: 手动测试清单**

- [ ] 打开美化页面
- [ ] 切换主题（深色/浅色/系统）
- [ ] 调整透明度滑块，观察窗口变化
- [ ] 调整圆角大小，观察预览变化
- [ ] 切换窗口置顶，验证窗口行为
- [ ] 切换模糊背景，观察预览变化
- [ ] 更改字体设置，观察预览变化
- [ ] 点击"恢复默认"，验证配置重置
- [ ] 刷新页面，验证配置持久化

**Step 3: 修复发现的问题**

如有问题，创建修复提交：

```bash
git add <fixed-files>
git commit -m "fix: <description>"
```

---

## Task 12: 最终提交与文档更新

**Step 1: 更新架构文档**

在 `src/ARCHITECTURE.md` 中添加外观系统的说明（可选）。

**Step 2: 最终提交**

```bash
git add -A
git commit -m "feat: complete appearance framework implementation"
```

---

## 文件清单

### 新增文件
- `src/types/appearance.ts`
- `src/lib/tauri/appearance.ts`
- `src/hooks/useAppearance.ts`
- `src/components/beautification/index.ts`
- `src/components/beautification/BeautificationPage.tsx`
- `src/components/beautification/ThemeSection.tsx`
- `src/components/beautification/WindowSection.tsx`
- `src/components/beautification/FontSection.tsx`
- `src/components/beautification/PreviewPanel.tsx`
- `src-tauri/src/appearance.rs`

### 修改文件
- `src/types/index.ts`
- `src/lib/tauri/index.ts`
- `src/components/PageRouter.tsx`
- `src-tauri/src/lib.rs`

### 删除文件
- `src/components/DesktopBeautificationPage.tsx` (旧组件)
