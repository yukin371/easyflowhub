/**
 * Autostart IPC wrapper.
 *
 * The native `autostart` module owns both the Windows startup entry and the
 * persisted user intent, so the UI never talks to the plugin directly: reading
 * the plugin's `isEnabled()` alone cannot tell "the user turned it off" from
 * "something deleted the entry".
 */

import { invoke } from '@tauri-apps/api/core';

export interface AutostartState {
  /** Whether the Windows startup entry currently exists. */
  system_enabled: boolean;
  /** Persisted intent; `null` when no intent has been recorded yet. */
  user_enabled: boolean | null;
  /** Set when this call repaired an entry that had been removed externally. */
  repaired: boolean;
  /** `false` when the startup entry cannot be read or written at all. */
  available: boolean;
}

export async function getAutostartState(): Promise<AutostartState> {
  return invoke<AutostartState>('get_autostart_state');
}

export async function setAutostartState(enabled: boolean): Promise<AutostartState> {
  return invoke<AutostartState>('set_autostart_state', { enabled });
}
