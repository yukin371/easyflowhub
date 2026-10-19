import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { LauncherSettings } from '../../types/launcherSettings';

/** Official Windows download page used when Snow Shot is unavailable. */
export const SNOW_SHOT_INSTALL_URL = 'https://snowshot.top/download.html?os=windows';

export type SnowShotLaunchStatus = 'launched' | 'not_installed' | 'error';

export interface SnowShotLaunchResult {
  status: SnowShotLaunchStatus;
  message?: string;
  installUrl?: string;
}

/** Context captured by the native trigger before the launcher takes focus. */
export interface LauncherOpenedPayload {
  selectedText?: string | null;
}

/** Optional user override for a portable or non-standard Snow Shot install. */
export const SNOW_SHOT_PATH_STORAGE_KEY = 'easyflowhub.snow-shot.path';

export function getConfiguredSnowShotPath(storage: Storage | undefined = typeof globalThis.localStorage === 'undefined' ? undefined : globalThis.localStorage): string | undefined {
  try {
    const value = storage?.getItem(SNOW_SHOT_PATH_STORAGE_KEY)?.trim();
    return value || undefined;
  } catch {
    return undefined;
  }
}

/** Persist an optional portable/non-standard Snow Shot executable path. */
export function saveConfiguredSnowShotPath(
  path: string,
  storage: Storage | undefined = typeof globalThis.localStorage === 'undefined' ? undefined : globalThis.localStorage,
): void {
  try {
    const normalized = path.trim();
    if (normalized) storage?.setItem(SNOW_SHOT_PATH_STORAGE_KEY, normalized);
    else storage?.removeItem(SNOW_SHOT_PATH_STORAGE_KEY);
  } catch {
    // Settings storage is best-effort; native discovery remains available.
  }
}

/** Ask the native host to locate and launch Snow Shot. */
export async function launchSnowShot(customPath?: string): Promise<SnowShotLaunchResult> {
  return invoke<SnowShotLaunchResult>('launch_snow_shot', {
    customPath: customPath?.trim() || null,
  });
}

/** Open Snow Shot's official Windows installation page in the default browser. */
export async function openSnowShotInstallPage(): Promise<void> {
  await invoke('open_snow_shot_install_page');
}

/** Hide the current Launcher webview while keeping it ready for the next trigger. */
export async function hideLauncherWindow(): Promise<void> {
  await invoke('hide_launcher_window');
}

/** Start dragging the frameless Launcher through the native window runtime. */
export async function startLauncherDragging(): Promise<void> {
  await invoke('start_launcher_dragging');
}

/** Toggle the Launcher pin state without changing other windows. */
export async function setLauncherAlwaysOnTop(enabled: boolean): Promise<boolean> {
  return invoke<boolean>('set_launcher_always_on_top', { enabled });
}

/** Read the native pin state when the Launcher page mounts or reloads. */
export async function getLauncherAlwaysOnTop(): Promise<boolean> {
  return invoke<boolean>('get_launcher_always_on_top');
}

/** Every native reveal resets the panel's keyboard entry point, even after hiding it. */
export function onLauncherOpened(onOpened: (payload?: LauncherOpenedPayload) => void): Promise<UnlistenFn> {
  // Keep the callback identity stable for consumers that register lifecycle
  // listeners directly; the native event wrapper is normalized by LauncherPage
  // so older integrations receiving the complete event remain compatible.
  return listen('launcher-opened', onOpened as (event: unknown) => void);
}

/** Focus the search entry point whenever Windows activates the Launcher window. */
export function onLauncherFocused(onFocused: () => void): Promise<UnlistenFn> {
  return listen('launcher-focused', onFocused);
}

/** Native dismissals include close, Esc, repeated triggers and system close. */
export function onLauncherHidden(onHidden: () => void): Promise<UnlistenFn> {
  return listen<null>('launcher-hidden', () => onHidden());
}

/** Apply Launcher trigger settings to the native global input layer. */
export async function applyLauncherTriggerSettings(settings: LauncherSettings): Promise<void> {
  await invoke('apply_launcher_trigger_settings', { config: settings });
}

/** Return the executable owning the current Windows foreground window. */
export async function getForegroundProcessName(): Promise<string | null> {
  return invoke<string | null>('get_foreground_process_name');
}

/** Replace the captured source selection in its original foreground window. */
export async function replaceSelectedText(text: string): Promise<void> {
  await invoke('replace_selected_text', { text });
}
