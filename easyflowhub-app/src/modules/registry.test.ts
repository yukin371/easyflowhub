import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ModuleRegistry } from './registry';
import type { FeatureModule } from './types';

const mockInvoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

function module(id: string, defaultEnabled: boolean): FeatureModule {
  return {
    id,
    name: id,
    icon: 'X',
    caption: id,
    defaultEnabled,
    component: () => null,
  };
}

describe('ModuleRegistry.toggleModule rollback (F07)', () => {
  let registry: ModuleRegistry;

  beforeEach(() => {
    vi.clearAllMocks();
    registry = new ModuleRegistry();
    // scripts: default off, but the user previously persisted it ON.
    registry.registerModules([module('scripts', false), module('notes', true)]);
  });

  it('restores the exact previous persisted value when the save fails', async () => {
    // Seed a previously saved ON state (as if an earlier toggle succeeded).
    mockInvoke.mockResolvedValueOnce({ ok: true });
    await expect(registry.toggleModule('scripts', true)).resolves.toBe(true);
    expect(registry.isEnabled('scripts')).toBe(true);

    // The next save fails. scripts must return to the persisted ON state,
    // not to its defaultEnabled=false.
    mockInvoke.mockResolvedValueOnce({ ok: false, error: 'disk full' });
    await expect(registry.toggleModule('scripts', false)).resolves.toBe(false);

    expect(registry.isEnabled('scripts')).toBe(true);
  });

  it('falls back to the module default when there was no previous entry', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('bridge down'));
    await expect(registry.toggleModule('notes', false)).resolves.toBe(false);

    // notes had no explicit entry and defaults to ON, so it stays ON.
    expect(registry.isEnabled('notes')).toBe(true);
  });

  it('applies the new value when the save succeeds', async () => {
    mockInvoke.mockResolvedValueOnce({ ok: true });
    await expect(registry.toggleModule('scripts', true)).resolves.toBe(true);
    expect(registry.isEnabled('scripts')).toBe(true);
  });

  it('notifies subscribers on both success and rollback so the UI re-syncs', async () => {
    const listener = vi.fn();
    const unsubscribe = registry.subscribe(listener);

    mockInvoke.mockResolvedValueOnce({ ok: true });
    await registry.toggleModule('scripts', true);
    expect(listener).toHaveBeenCalledTimes(1);

    mockInvoke.mockResolvedValueOnce({ ok: false, error: 'nope' });
    await registry.toggleModule('scripts', false);
    // Rollback must also notify, otherwise the UI keeps showing the
    // optimistic value that was never persisted.
    expect(listener).toHaveBeenCalledTimes(2);

    unsubscribe();
  });
});
