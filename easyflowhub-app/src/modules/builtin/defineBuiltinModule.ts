import type { ComponentType } from 'react';
import type { FeatureModule } from '../types';

type BuiltinModuleDefinition = Omit<FeatureModule, 'component'>;

export function defineBuiltinModule(
  definition: BuiltinModuleDefinition,
  component: ComponentType
): FeatureModule {
  return {
    ...definition,
    component,
  };
}
