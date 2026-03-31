/**
 * FeatureModule 类型定义
 * 定义模块系统的核心接口
 */

import type { ComponentType } from 'react';

/**
 * 功能模块接口
 * 所有内置模块和未来插件都需要实现这个接口
 */
export interface FeatureModule {
  /** 唯一标识符 */
  id: string;

  /** 显示名称 */
  name: string;

  /** 侧边栏图标（单字） */
  icon: string;

  /** 英文副标题 */
  caption: string;

  /** 默认是否启用 */
  defaultEnabled: boolean;

  /** 是否为核心模块（核心模块不可关闭） */
  isCore?: boolean;

  /** 面板组件 */
  component: ComponentType;

  // === TODO: 插件系统扩展点 ===
  // 描述信息
  // description?: string;
  // version?: string;
  // author?: string;

  // 生命周期钩子
  // onLoad?: () => Promise<void>;
  // onUnload?: () => void;
  // onActivate?: () => void;
  // onDeactivate?: () => void;

  // 权限声明
  // permissions?: string[];

  // 插件元数据
  // pluginPath?: string;
  // isBuiltin?: boolean;
}

/**
 * 模块配置
 */
export interface ModuleConfig {
  /** 是否启用 */
  enabled: boolean;
}

/**
 * 模块配置文件结构
 */
export interface ModulesConfig {
  /** 配置版本 */
  version: number;

  /** 各模块的配置 */
  modules: Record<string, ModuleConfig>;
}

/**
 * 模块配置文件响应
 */
export interface ModulesConfigResponse {
  ok: boolean;
  config: ModulesConfig | null;
  error?: string;
}

/**
 * 切换模块响应
 */
export interface ToggleModuleResponse {
  ok: boolean;
  enabled: boolean;
  error?: string;
}
