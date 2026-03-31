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
