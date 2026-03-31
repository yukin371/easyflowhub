// Re-export Note and Tab types from note.ts
export type { Note, Tab } from './note';

// Window state
export interface WindowState {
  opacity: number;
  alwaysOnTop: boolean;
  isVisible: boolean;
}

// App settings
export interface AppSettings {
  theme: 'light' | 'dark' | 'system';
  opacity: number;
  fontSize: number;
  autoSave: boolean;
  globalShortcut: string;
}

// 外观相关类型
export * from './appearance';
