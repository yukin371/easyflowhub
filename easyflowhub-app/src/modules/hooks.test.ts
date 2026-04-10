import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { moduleRegistry } from './registry';
import { useEnabledModules, useToggleableModules } from './hooks';
import type { FeatureModule } from './types';

const mockInvoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

const coreModule: FeatureModule = {
  id: 'notes',
  name: '笔记',
  icon: '墨',
  caption: 'Notes',
  defaultEnabled: true,
  isCore: true,
  component: () => null,
};

const optionalModule: FeatureModule = {
  id: 'scripts',
  name: '脚本',
  icon: '策',
  caption: 'Scripts',
  defaultEnabled: false,
  component: () => null,
};

describe('module hooks', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    moduleRegistry.reset();
    moduleRegistry.registerModules([coreModule, optionalModule]);

    mockInvoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'load_modules_config') {
        return {
          ok: true,
          config: {
            version: 1,
            modules: {
              scripts: { enabled: true },
            },
          },
        };
      }

      if (command === 'toggle_module_config') {
        return {
          ok: true,
          enabled: args?.enabled,
        };
      }

      return { ok: true };
    });
  });

  it('loads enabled modules from persisted config', async () => {
    const { result } = renderHook(() => useEnabledModules());

    await waitFor(() => {
      expect(result.current.map((module) => module.id)).toEqual(['notes', 'scripts']);
    });
  });

  it('updates enabled modules after toggling', async () => {
    const { result } = renderHook(() => useEnabledModules());

    await waitFor(() => {
      expect(result.current.map((module) => module.id)).toEqual(['notes', 'scripts']);
    });

    await act(async () => {
      await moduleRegistry.toggleModule('scripts', false);
    });

    expect(result.current.map((module) => module.id)).toEqual(['notes']);
  });

  it('returns only non-core modules for settings', async () => {
    const { result } = renderHook(() => useToggleableModules());

    await waitFor(() => {
      expect(result.current.map((module) => module.id)).toEqual(['scripts']);
    });
  });
});
