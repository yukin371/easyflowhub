import { emitTo } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

/** Cross-window event consumed by ManagerPage for settings navigation. */
export const MANAGER_NAVIGATE_TO_SETTINGS_EVENT = 'easyflowhub:navigate-to-settings';
/** In-manager event consumed by SettingsPanel after the settings module mounts. */
export const MANAGER_OPEN_SETTINGS_SECTION_EVENT = 'easyflowhub:open-settings-section';

export type ManagerSettingsSection =
  | 'general'
  | 'modules'
  | 'shortcuts'
  | 'translation'
  | 'transcription'
  | 'developer'
  | 'quickNote'
  | 'todo'
  | 'trash'
  | 'editor';

export interface ManagerSettingsNavigationDetail {
  section: ManagerSettingsSection;
}

/** Show the Manager window and select a settings section after it mounts. */
export async function openManagerSettings(section: ManagerSettingsSection): Promise<void> {
  await invoke('show_manager_window');
  await emitTo('manager', MANAGER_NAVIGATE_TO_SETTINGS_EVENT, { section });
}
