/**
 * 模块系统统一导出
 */

// 类型
export type { FeatureModule, ModuleConfig, ModulesConfig, ModulesConfigResponse, ToggleModuleResponse } from './types';

// 注册表
export { moduleRegistry, ModuleRegistry } from './registry';
export { useEnabledModules, useToggleableModules } from './hooks';

// 内置模块
export { builtinModules, registerBuiltinModules } from './builtin';
export {
  notesModule,
  scriptsModule,
  extensionsModule,
  relayModule,
  mcpModule,
  tasksModule,
  componentsModule,
  trashModule,
  todosModule,
  settingsModule,
} from './builtin';
