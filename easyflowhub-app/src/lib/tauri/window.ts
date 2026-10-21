import { invoke } from '@tauri-apps/api/core';

/**
 * Window command bridge.
 *
 * Covers the low-level window commands that are not tied to notes, appearance
 * or the manager shell: hiding / closing the current window and toggling the
 * quick-note always-on-top flag. Keeping them here stops pages from hard-coding
 * command names in `invoke()`.
 */

/** Toggle the current window's always-on-top flag and return the new state. */
export async function toggleWindowAlwaysOnTop(): Promise<boolean> {
  return invoke<boolean>('toggle_always_on_top');
}

/** Hide the current window (the Launcher is dismissed through native state). */
export async function hideCurrentWindow(): Promise<void> {
  await invoke('hide_window');
}

/** Destroy the current window, used when the destroyed webview is going away. */
export async function closeCurrentWindow(): Promise<void> {
  await invoke('close_window');
}
