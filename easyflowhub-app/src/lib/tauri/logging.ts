/**
 * Logging bridge - developer mode diagnostics.
 *
 * Logs live in a bounded native ring buffer; enabling debug mode additionally
 * appends them to `<app data>/logs/app.log`.
 */

import { invoke } from '@tauri-apps/api/core';

export async function getDebugLogs(limit = 200): Promise<string[]> {
  return await invoke('get_debug_logs', { limit });
}

export async function clearDebugLogs(): Promise<void> {
  await invoke('clear_debug_logs');
}

export async function setDebugMode(enabled: boolean): Promise<boolean> {
  return await invoke('set_debug_mode', { enabled });
}

export async function openDebugLogDir(): Promise<void> {
  await invoke('open_debug_log_dir');
}
