import { useEffect, useState } from 'react';
import { moduleRegistry } from './registry';
import type { FeatureModule } from './types';

type ModuleCollectionKind = 'enabled' | 'toggleable' | 'experimental' | 'standard-toggleable';

function fetchCollection(kind: ModuleCollectionKind): FeatureModule[] {
  switch (kind) {
    case 'enabled':
      return moduleRegistry.getEnabledModules();
    case 'experimental':
      return moduleRegistry.getExperimentalModules();
    case 'standard-toggleable':
      return moduleRegistry.getStandardToggleableModules();
    case 'toggleable':
    default:
      return moduleRegistry.getToggleableModules();
  }
}

function useModuleCollection(kind: ModuleCollectionKind): FeatureModule[] {
  const [modules, setModules] = useState<FeatureModule[]>(() => fetchCollection(kind));

  useEffect(() => {
    let mounted = true;

    const syncModules = () => {
      if (!mounted) {
        return;
      }

      setModules(fetchCollection(kind));
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

/** 实验性（预览）模块：默认关闭，只在「实验性功能」分区出现。 */
export function useExperimentalModules(): FeatureModule[] {
  return useModuleCollection('experimental');
}

/** 普通（非实验、非核心）可切换模块：用于「功能模块」分区。 */
export function useStandardToggleableModules(): FeatureModule[] {
  return useModuleCollection('standard-toggleable');
}
