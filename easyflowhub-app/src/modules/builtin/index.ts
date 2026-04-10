/**
 * 内置模块统一导出和注册
 */
import type { FeatureModule } from '../types';
import { moduleRegistry } from '../registry';

import { notesModule } from './notes';
import { scriptsModule } from './scripts';
import { relayModule } from './relay';
import { mcpModule } from './mcp';
import { extensionsModule } from './extensions';
import { tasksModule } from './tasks';
import { componentsModule } from './components';
import { trashModule } from './trash';
import { todosModule } from './todos';
import { settingsModule } from './settings';

/**
 * 所有内置模块列表
 */
export const builtinModules: FeatureModule[] = [
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
];

/**
 * 注册所有内置模块
 */
export function registerBuiltinModules(): void {
  moduleRegistry.registerModules(builtinModules);
}

// 导出各个模块供直接使用
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
};
