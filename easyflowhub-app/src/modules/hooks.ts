import { useEffect, useState } from 'react';
import { moduleRegistry } from './registry';
import type { FeatureModule } from './types';

function useModuleCollection(kind: 'enabled' | 'toggleable'): FeatureModule[] {
  const [modules, setModules] = useState<FeatureModule[]>(() =>
    kind === 'enabled' ? moduleRegistry.getEnabledModules() : moduleRegistry.getToggleableModules()
  );

  useEffect(() => {
    let mounted = true;

    const syncModules = () => {
      if (!mounted) {
        return;
      }

      setModules(
        kind === 'enabled' ? moduleRegistry.getEnabledModules() : moduleRegistry.getToggleableModules()
      );
    };

    const loadModules = async () => {
      try {
        await moduleRegistry.loadConfig();
      } catch (error) {
        console.error('Failed to load module config, using defaults:', error);
      }

      syncModules();
    };

    void loadModules();

    const unsubscribe = moduleRegistry.subscribe(syncModules);
    return () => {
      mounted = false;
      unsubscribe();
    };
  }, [kind]);

  return modules;
}

export function useEnabledModules(): FeatureModule[] {
  return useModuleCollection('enabled');
}

export function useToggleableModules(): FeatureModule[] {
  return useModuleCollection('toggleable');
}
