/**
 * ModuleRegistry - 模块注册表
 * 管理所有功能模块的注册、配置和启用状态
 */

import { invoke } from '@tauri-apps/api/core';
import type { FeatureModule, ModulesConfig } from './types';

/**
 * 默认配置
 */
const DEFAULT_CONFIG: ModulesConfig = {
  version: 1,
  modules: {},
};

/**
 * 模块注册表
 * 单例模式，管理所有功能模块
 */
class ModuleRegistry {
  private modules = new Map<string, FeatureModule>();
  private config: ModulesConfig = { ...DEFAULT_CONFIG };
  private loaded = false;
  private listeners = new Set<() => void>();

  /**
   * 注册模块
   */
  registerModule(module: FeatureModule): void {
    if (this.modules.has(module.id)) {
      console.warn(`Module "${module.id}" already registered, skipping.`);
      return;
    }
    this.modules.set(module.id, module);
  }

  /**
   * 批量注册模块
   */
  registerModules(modules: FeatureModule[]): void {
    modules.forEach((m) => this.registerModule(m));
  }

  /**
   * 获取所有已注册的模块
   */
  getAllModules(): FeatureModule[] {
    return Array.from(this.modules.values());
  }

  /**
   * 获取启用的模块
   */
  getEnabledModules(): FeatureModule[] {
    return this.getAllModules().filter((m) => this.isEnabled(m.id));
  }

  /**
   * 获取可切换的模块（排除核心模块）
   */
  getToggleableModules(): FeatureModule[] {
    return this.getAllModules().filter((m) => !m.isCore);
  }

  /**
   * 获取核心模块
   */
  getCoreModules(): FeatureModule[] {
    return this.getAllModules().filter((m) => m.isCore);
  }

  /**
   * 根据 ID 获取模块
   */
  getModule(id: string): FeatureModule | undefined {
    return this.modules.get(id);
  }

  /**
   * 检查模块是否启用
   */
  isEnabled(id: string): boolean {
    // 如果配置文件中有记录，使用配置文件的值
    if (id in this.config.modules) {
      return this.config.modules[id].enabled;
    }
    // 否则使用模块的默认值
    const module = this.modules.get(id);
    return module?.defaultEnabled ?? false;
  }

  /**
   * 切换模块启用状态
   */
  async toggleModule(id: string, enabled: boolean): Promise<boolean> {
    const module = this.modules.get(id);
    if (!module) {
      console.warn(`Module "${id}" not found.`);
      return false;
    }

    if (module.isCore) {
      console.warn(`Core module "${id}" cannot be toggled.`);
      return false;
    }

    // 更新本地配置
    this.config.modules[id] = { enabled };

    // 持久化到配置文件
    try {
      const response = await invoke<{ ok: boolean; error?: string }>(
        'toggle_module_config',
        { moduleId: id, enabled }
      );
      if (!response.ok) {
        console.error('Failed to save module config:', response.error);
        // 回滚本地配置
        delete this.config.modules[id];
        return false;
      }
    } catch (error) {
      console.error('Failed to save module config:', error);
      delete this.config.modules[id];
      return false;
    }

    // 通知监听者
    this.notifyListeners();
    return true;
  }

  /**
   * 加载配置文件
   */
  async loadConfig(): Promise<void> {
    if (this.loaded) return;

    try {
      const response = await invoke<{ ok: boolean; config: ModulesConfig | null; error?: string }>(
        'load_modules_config'
      );

      if (response.ok && response.config) {
        this.config = response.config;
      } else {
        console.warn('No modules config found, using defaults.');
        // 根据模块默认值初始化配置
        this.initializeDefaultConfig();
      }
    } catch (error) {
      console.error('Failed to load modules config:', error);
      this.initializeDefaultConfig();
    }

    this.loaded = true;
  }

  /**
   * 保存配置文件
   */
  async saveConfig(): Promise<boolean> {
    try {
      const response = await invoke<{ ok: boolean; error?: string }>(
        'save_modules_config',
        { config: this.config }
      );
      return response.ok;
    } catch (error) {
      console.error('Failed to save modules config:', error);
      return false;
    }
  }

  /**
   * 初始化默认配置
   */
  private initializeDefaultConfig(): void {
    this.config = { ...DEFAULT_CONFIG };
    // 不预先填充，让 isEnabled 使用模块的 defaultEnabled
  }

  /**
   * 添加配置变更监听器
   */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /**
   * 通知所有监听者
   */
  private notifyListeners(): void {
    this.listeners.forEach((listener) => listener());
  }

  /**
   * 重置状态（用于测试）
   */
  reset(): void {
    this.modules.clear();
    this.config = { ...DEFAULT_CONFIG };
    this.loaded = false;
    this.listeners.clear();
  }
}

// 导出单例
export const moduleRegistry = new ModuleRegistry();

// 也可以导出类用于测试
export { ModuleRegistry };
