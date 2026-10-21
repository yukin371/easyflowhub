import { beforeEach, describe, expect, it, vi } from 'vitest';
import { emitTo } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { openManagerSettings } from './manager';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ emitTo: vi.fn() }));

const invokeMock = vi.mocked(invoke);
const emitToMock = vi.mocked(emitTo);

describe('manager Tauri bridge', () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue(undefined as never);
    emitToMock.mockReset().mockResolvedValue(undefined as never);
  });

  it('shows Manager before emitting the requested settings section', async () => {
    await openManagerSettings('translation');

    expect(invokeMock).toHaveBeenCalledWith('show_manager_window');
    expect(emitToMock).toHaveBeenCalledWith('manager', 'easyflowhub:navigate-to-settings', {
      section: 'translation',
    });
    expect(invokeMock.mock.invocationCallOrder[0]).toBeLessThan(emitToMock.mock.invocationCallOrder[0]);
  });
});
