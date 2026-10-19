/** Launcher trigger settings shared by the Manager UI and the native host. */

export type LauncherActivation = 'keyboard' | 'middleMouse' | 'keyboardOrMiddleMouse';

export interface LauncherExcludedApp {
  /** Windows process name or executable path used by the native foreground-window check. */
  executable: string;
  /** Stable, case-insensitive id derived from the executable value. */
  id: string;
  label?: string;
}

export interface LauncherSettings {
  activation: LauncherActivation;
  keyboardShortcut: string;
  excludedApps: LauncherExcludedApp[];
}

export const LAUNCHER_SETTINGS_STORAGE_KEY = 'easyflowhub_launcher_settings_v1';

export const DEFAULT_LAUNCHER_SETTINGS: LauncherSettings = {
  activation: 'keyboardOrMiddleMouse',
  keyboardShortcut: 'Ctrl+Alt+Space',
  excludedApps: [],
};

function getStorage(): Storage | undefined {
  return typeof globalThis.localStorage === 'undefined' ? undefined : globalThis.localStorage;
}

function normalizeExecutable(value: string): string {
  return value.trim().toLowerCase();
}

export function createExcludedApp(executable: string, label?: string): LauncherExcludedApp | null {
  const normalized = normalizeExecutable(executable);
  if (!normalized) return null;

  return {
    id: normalized,
    executable: executable.trim(),
    ...(label?.trim() ? { label: label.trim() } : {}),
  };
}

export function normalizeLauncherSettings(value: Partial<LauncherSettings> | null | undefined): LauncherSettings {
  const activation = value?.activation;
  const normalizedActivation: LauncherActivation =
    activation === 'keyboard' || activation === 'middleMouse' || activation === 'keyboardOrMiddleMouse'
      ? activation
      : DEFAULT_LAUNCHER_SETTINGS.activation;
  const keyboardShortcut = typeof value?.keyboardShortcut === 'string' && value.keyboardShortcut.trim()
    ? value.keyboardShortcut.trim()
    : DEFAULT_LAUNCHER_SETTINGS.keyboardShortcut;
  const excludedApps = Array.isArray(value?.excludedApps)
    ? value.excludedApps.reduce<LauncherExcludedApp[]>((result, item) => {
        if (!item || typeof item.executable !== 'string') return result;
        const app = createExcludedApp(item.executable, typeof item.label === 'string' ? item.label : undefined);
        if (!app || result.some((existing) => existing.id === app.id)) return result;
        result.push(app);
        return result;
      }, [])
    : [];

  return { activation: normalizedActivation, keyboardShortcut, excludedApps };
}

export function loadLauncherSettings(storage: Storage | undefined = getStorage()): LauncherSettings {
  if (!storage) return DEFAULT_LAUNCHER_SETTINGS;

  try {
    const raw = storage.getItem(LAUNCHER_SETTINGS_STORAGE_KEY);
    if (!raw) return DEFAULT_LAUNCHER_SETTINGS;
    return normalizeLauncherSettings(JSON.parse(raw) as Partial<LauncherSettings>);
  } catch {
    return DEFAULT_LAUNCHER_SETTINGS;
  }
}

export function saveLauncherSettings(
  settings: LauncherSettings,
  storage: Storage | undefined = getStorage(),
): void {
  if (!storage) return;
  storage.setItem(LAUNCHER_SETTINGS_STORAGE_KEY, JSON.stringify(normalizeLauncherSettings(settings)));
}
