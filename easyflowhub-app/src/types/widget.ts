// src/types/widget.ts
//! Desktop Widget Types - 桌面小组件类型定义

/** 应用快捷方式 */
export interface AppShortcut {
  id: string;
  name: string;
  icon: string;
  execPath: string;
}

/** 文件夹组件配置 */
export interface FolderWidgetConfig {
  id: string;
  name: string;
  apps: AppShortcut[];
  position?: { x: number; y: number };
}

/** 组件类型 */
export type WidgetType = 'folder' | 'clock' | 'note' | 'system';

/** 组件窗口模式 */
export type WidgetMode = 'collapsed' | 'expanded';

/** 组件状态 */
export interface WidgetState {
  id: string;
  type: WidgetType;
  mode: WidgetMode;
  position: { x: number; y: number };
  size: { width: number; height: number };
}
