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
import { componentsModule } from './components';
import { trashModule } from './trash';
import { todosModule } from './todos';
import { settingsModule } from './settings';
import { filesModule } from './files';
import { downloadsModule } from './downloads';
import { voiceModule } from './voice';

/**
 * 所有内置模块列表
 *
 * 执行记录不再是独立模块：它作为「脚本」页的一个视图（运行记录 tab）呈现，
 * 避免用户为了看脚本输出而跳到另一个侧栏入口。
 */
export const builtinModules: FeatureModule[] = [
  notesModule,
  scriptsModule,
  extensionsModule,
  relayModule,
  mcpModule,
  componentsModule,
  trashModule,
  todosModule,
  settingsModule,
  filesModule,
  downloadsModule,
  voiceModule,
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
  componentsModule,
  trashModule,
  todosModule,
  settingsModule,
  filesModule,
  downloadsModule,
  voiceModule,
};
