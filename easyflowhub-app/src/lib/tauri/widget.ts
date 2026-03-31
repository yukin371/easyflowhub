// src/lib/tauri/widget.ts
//! Desktop Widget IPC Layer - 桌面小组件 IPC 层

import { invoke } from '@tauri-apps/api/core';
import type { AppShortcut } from '../../types/widget';

/**
 * 创建文件夹组件窗口
 */
export async function createFolderWidget(widgetId?: string): Promise<string> {
  return invoke<string>('create_folder_widget', { widgetId });
}

/**
 * 获取文件夹组件的默认应用列表
 */
export async function getFolderApps(): Promise<AppShortcut[]> {
  return invoke<AppShortcut[]>('get_folder_apps');
}

/**
 * 启动应用
 */
export async function launchApp(execPath: string): Promise<void> {
  return invoke('launch_app', { execPath });
}

/**
 * 关闭组件窗口
 */
export async function closeWidget(widgetId: string): Promise<void> {
  return invoke('close_widget', { widgetId });
}

/**
 * 设置组件窗口大小（用于展开/收起动画）
 */
export async function setWidgetSize(
  widgetId: string,
  width: number,
  height: number
): Promise<void> {
  return invoke('set_widget_size', { widgetId, width, height });
}
