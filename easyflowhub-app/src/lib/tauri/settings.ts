/**
 * Settings IPC Wrapper
 * Provides typed frontend API for settings commands
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  AppSettings,
  GetSettingsResponse,
  UpdateSettingsResponse,
} from '../../types/settings';

export async function getSettings(): Promise<AppSettings> {
  const response: GetSettingsResponse = await invoke('get_settings');
  return response.settings;
}

export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  const response: UpdateSettingsResponse = await invoke('update_settings', { settings });
  return response.settings;
}
