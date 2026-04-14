import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsPanel } from './SettingsPanel';
import { getSettings } from '../../lib/tauri/settings';
import { moduleRegistry, useToggleableModules } from '../../modules';
import { isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart';
import type { AppSettings } from '../../types/settings';
import type { FeatureModule } from '../../modules';

vi.mock('../../lib/tauri/settings', () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
}));

vi.mock('../../modules', () => ({
  moduleRegistry: {
    isEnabled: vi.fn(),
    toggleModule: vi.fn(),
  },
  useToggleableModules: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-autostart', () => ({
  isEnabled: vi.fn(),
  enable: vi.fn(),
  disable: vi.fn(),
}));

const getSettingsMock = vi.mocked(getSettings);
const useToggleableModulesMock = vi.mocked(useToggleableModules);
const isAutostartEnabledMock = vi.mocked(isAutostartEnabled);
const isModuleEnabledMock = vi.mocked(moduleRegistry.isEnabled);
const toggleModuleMock = vi.mocked(moduleRegistry.toggleModule);

const appSettings: AppSettings = {
  quick_note: { width: 400, height: 300 },
  trash: { retention_days: 30 },
  editor: {
    undo_steps: 100,
    cursor_style: 'accent',
    cursor_color: '#4f5a43',
    cursor_trail: true,
  },
  todo: { done_retention_hours: 24 },
};

const scriptsModule: FeatureModule = {
  id: 'scripts',
  name: '脚本',
  icon: '策',
  caption: 'Scripts',
  defaultEnabled: true,
  component: () => null,
};

describe('SettingsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getSettingsMock.mockResolvedValue(appSettings);
    isAutostartEnabledMock.mockResolvedValue(false);
    useToggleableModulesMock.mockReturnValue([scriptsModule]);
    isModuleEnabledMock.mockReturnValue(true);
    toggleModuleMock.mockResolvedValue({ ok: true, enabled: false } as never);
  });

  it('通过 useToggleableModules 渲染可切换模块列表', async () => {
    render(<SettingsPanel />);

    await screen.findByText('脚本');

    expect(useToggleableModulesMock).toHaveBeenCalled();
    expect(screen.getByText('脚本')).toBeInTheDocument();
    expect(screen.getByText('Scripts')).toBeInTheDocument();
  });

  it('点击模块开关时继续通过 moduleRegistry.toggleModule 切换状态', async () => {
    render(<SettingsPanel />);

    const moduleLabel = await screen.findByText('脚本');
    const moduleRow = moduleLabel.parentElement?.parentElement?.parentElement;
    const moduleToggle = moduleRow?.querySelector('button');

    expect(moduleToggle).toBeDefined();

    fireEvent.click(moduleToggle as HTMLButtonElement);

    await waitFor(() => {
      expect(toggleModuleMock).toHaveBeenCalledWith('scripts', false);
    });
  });
});
