import { beforeEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_LAUNCHER_SETTINGS,
  createExcludedApp,
  loadLauncherSettings,
  normalizeLauncherSettings,
  saveLauncherSettings,
} from './launcherSettings';

describe('launcher settings', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('normalizes activation, shortcut and duplicate process names', () => {
    expect(normalizeLauncherSettings({
      activation: 'invalid' as never,
      keyboardShortcut: '  Ctrl+Alt+L  ',
      excludedApps: [
        { id: 'ignored', executable: 'Code.exe' },
        { id: 'ignored-again', executable: ' code.EXE ' },
        { id: 'idea', executable: 'idea64.exe', label: 'IntelliJ IDEA' },
      ],
    })).toEqual({
      activation: DEFAULT_LAUNCHER_SETTINGS.activation,
      keyboardShortcut: 'Ctrl+Alt+L',
      excludedApps: [
        { id: 'code.exe', executable: 'Code.exe' },
        { id: 'idea64.exe', executable: 'idea64.exe', label: 'IntelliJ IDEA' },
      ],
    });
  });

  it('persists settings and falls back to defaults for malformed values', () => {
    const settings = {
      ...DEFAULT_LAUNCHER_SETTINGS,
      activation: 'middleMouse' as const,
      keyboardShortcut: 'Ctrl+Alt+L',
      excludedApps: [createExcludedApp('Code.exe')!],
    };

    saveLauncherSettings(settings);
    expect(loadLauncherSettings()).toEqual(settings);

    localStorage.setItem('easyflowhub_launcher_settings_v1', '{broken');
    expect(loadLauncherSettings()).toEqual(DEFAULT_LAUNCHER_SETTINGS);
  });

  it('rejects empty excluded app values', () => {
    expect(createExcludedApp('   ')).toBeNull();
  });
});
