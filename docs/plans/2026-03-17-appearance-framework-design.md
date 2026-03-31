# 桌面美化可扩展框架设计

## 概述

为 deskflow 应用设计可扩展的外观配置系统，采用分层架构实现主题、窗口外观、字体设置等功能。

## 架构方案

**方案 ：分层配置系统**

```
类型层 (types) → IPC层 (lib/tauri) → Hook层 (hooks) → UI组件层 (components)
```

优势：
- 职责分离，易于测试
- 后端可替换（目前 Tauri，可换其他）
- 配置可持久化
- 支持渐进增强

## 类型层设计

**文件**: `src/types/appearance.ts`

```typescript
/** 主题模式 */
export type ThemeMode = 'light' | 'dark' | 'system';

/** 窗口外观设置 */
export interface WindowAppearance {
  opacity: number;        // 0-1
  cornerRadius: number;   // px
  isAlwaysOnTop: boolean;
  isBlurEnabled: boolean;
}

/** 字体设置 */
export interface FontSettings {
  fontFamily: string;
  fontSize: number;       // px
  lineHeight: number;     // 1.0-2.0
}

/** 完整外观配置 */
export interface AppearanceConfig {
  theme: ThemeMode;
  window: WindowAppearance;
  font: FontSettings;
}

/** 默认配置 */
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
```

## IPC 层设计

**文件**: `src/lib/tauri/appearance.ts`

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { AppearanceConfig } from '../../types/appearance';
import { DEFAULT_APPEARANCE } from '../../types/appearance';

const STORAGE_KEY = 'deskflow-appearance';

/** 加载外观配置 */
export async function loadAppearanceConfig(): Promise<AppearanceConfig> {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) {
    try {
      return { ...DEFAULT_APPEARANCE, ...JSON.parse(stored) };
    } catch { /* ignore */ }
  }
  return DEFAULT_APPEARANCE;
}

/** 保存外观配置 */
export function saveAppearanceConfig(config: AppearanceConfig): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
}

/** 应用窗口置顶 */
export async function applyAlwaysOnTop(enabled: boolean): Promise<boolean> {
  return invoke('set_always_on_top', { enabled });
}

/** 应用窗口透明度 */
export async function applyWindowOpacity(opacity: number): Promise<void> {
  return invoke('set_window_opacity', { opacity });
}
```

## Hook 层设计

**文件**: `src/hooks/useAppearance.ts`

```typescript
import { useState, useEffect, useCallback, useMemo } from 'react';
import {
  loadAppearanceConfig,
  saveAppearanceConfig,
  applyAlwaysOnTop,
  applyWindowOpacity,
} from '../lib/tauri/appearance';
import type {
  AppearanceConfig,
  ThemeMode,
  WindowAppearance,
  FontSettings,
} from '../types/appearance';

export function useAppearance() {
  const [config, setConfig] = useState<AppearanceConfig | null>(null);
  const [isLoaded, setIsLoaded] = useState(false);

  // 初始化加载
  useEffect(() => {
    loadAppearanceConfig().then((cfg) => {
      setConfig(cfg);
      setIsLoaded(true);
    });
  }, []);

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

      // 应用到后端
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

  // 计算实际主题
  const resolvedTheme = useMemo(() => {
    if (!config) return 'dark';
    if (config.theme === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return config.theme;
  }, [config]);

  return {
    config,
    isLoaded,
    resolvedTheme,
    setTheme,
    setWindowAppearance,
    setFontSettings,
  };
}
```

## UI 组件层设计

**目录结构**:

```
components/beautification/
├── index.ts                    # 统一导出
├── BeautificationPage.tsx      # 页面容器
├── ThemeSection.tsx            # 主题选择区
├── WindowSection.tsx           # 窗口外观区
├── FontSection.tsx             # 字体设置区
└── PreviewPanel.tsx            # 实时预览面板
```

**BeautificationPage.tsx**:

```typescript
import { useAppearance } from '../../hooks/useAppearance';
import { ThemeSection } from './ThemeSection';
import { WindowSection } from './WindowSection';
import { FontSection } from './FontSection';
import { PreviewPanel } from './PreviewPanel';

export function BeautificationPage() {
  const { config, isLoaded, resolvedTheme, setTheme, setWindowAppearance, setFontSettings } = useAppearance();

  if (!isLoaded || !config) {
    return <LoadingState />;
  }

  return (
    <div className="h-full flex flex-col">
      <header className="p-4 border-b border-white/10">
        <h1 className="text-lg font-semibold">桌面美化</h1>
      </header>

      <main className="flex-1 overflow-auto p-6 space-y-8">
        <ThemeSection theme={config.theme} onThemeChange={setTheme} />
        <WindowSection window={config.window} onChange={setWindowAppearance} />
        <FontSection font={config.font} onChange={setFontSettings} />
        <PreviewPanel config={config} resolvedTheme={resolvedTheme} />
      </main>

      <footer className="p-4 border-t border-white/10 flex justify-end gap-3">
        <ResetButton />
      </footer>
    </div>
  );
}
```

## Rust 后端命令设计

**文件**: `src-tauri/src/appearance.rs`

```rust
use tauri::{command, AppHandle, Manager};

#[command]
pub async fn set_always_on_top(app: AppHandle, enabled: bool) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_always_on_top(enabled)
            .map_err(|e| e.to_string())?;
        Ok(enabled)
    } else {
        Err("Window not found".to_string())
    }
}

#[command]
pub async fn set_window_opacity(app: AppHandle, opacity: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_opacity(opacity)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

**注册命令** (lib.rs):

```rust
.invoke_handler(tauri::generate_handler![
    // ...existing commands...
    appearance::set_always_on_top,
    appearance::set_window_opacity,
])
```

## 数据流

```
┌─────────────────────────────────────────────────────────────┐
│                        UI 组件层                             │
│  BeautificationPage → ThemeSection / WindowSection / ...    │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                      Hook 层 (useAppearance)                 │
│  - 状态管理 (config, resolvedTheme)                          │
│  - 持久化协调 (saveAppearanceConfig)                         │
│  - 后端同步 (applyAlwaysOnTop, applyWindowOpacity)          │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                        IPC 层                                │
│  前端设置 (localStorage)  ←→  窗口效果 (Tauri invoke)        │
└─────────────────────────────────────────────────────────────┘
```

## 设置分类

| 设置项 | 存储位置 | 应用时机 |
|--------|----------|----------|
| theme | localStorage | 即时（CSS 切换） |
| fontFamily, fontSize, lineHeight | localStorage | 即时（CSS 变量） |
| opacity | localStorage + Rust | 即时 + 窗口 API |
| isAlwaysOnTop | localStorage + Rust | 即时 + 窗口 API |
| cornerRadius | localStorage | 即时（CSS 变量） |
| isBlurEnabled | localStorage | 即时（backdrop-filter） |

## 实现顺序

1. **类型层** → `types/appearance.ts`
2. **IPC 层** → `lib/tauri/appearance.ts`
3. **Hook 层** → `hooks/useAppearance.ts`
4. **UI 组件层** → `components/beautification/`
5. **Rust 后端** → `src-tauri/src/appearance.rs`

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

- `src/types/index.ts` - 导出新类型
- `src/lib/tauri/index.ts` - 导出新 IPC
- `src/components/DesktopBeautificationPage.tsx` - 迁移到新组件
- `src-tauri/src/lib.rs` - 注册 Rust 命令
- `src/styles.css` - 添加 CSS 变量支持
