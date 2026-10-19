/**
 * Runtime barrel for the module system.
 *
 * This entry intentionally excludes `builtin` (which binds panels to module
 * ids). Panels consume the registry through this barrel, so the dependency
 * direction stays `components -> runtime -> registry` instead of
 * `components -> modules -> builtin -> components`.
 */

export type {
  FeatureModule,
  ModuleConfig,
  ModulesConfig,
  ModulesConfigResponse,
  ToggleModuleResponse,
} from './types';

export { moduleRegistry, ModuleRegistry } from './registry';
export { useEnabledModules, useToggleableModules } from './hooks';
