import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsPanel } from './SettingsPanel';
import { getSettings, updateSettings } from '../../lib/tauri/settings';
import { moduleRegistry, useToggleableModules } from '../../modules/runtime';
import { isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart';
import type { AppSettings } from '../../types/settings';
import type { FeatureModule } from '../../modules/runtime';
import { MANAGER_OPEN_SETTINGS_SECTION_EVENT } from '../../lib/tauri/manager';

vi.mock('../../lib/tauri/settings', () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
}));

vi.mock('../../lib/tauri/launcher', () => ({
  applyLauncherTriggerSettings: vi.fn().mockResolvedValue(undefined),
  getConfiguredSnowShotPath: vi.fn().mockReturnValue(undefined),
  saveConfiguredSnowShotPath: vi.fn(),
}));

vi.mock('../../modules/runtime', () => ({
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
const updateSettingsMock = vi.mocked(updateSettings);
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
  translation: { target_language: 'zh-CN', model: '' },
  transcription: { model: '', language: '' },
  developer: { debug_mode: false },
  launcher_trigger: {
    enabled: true,
    trigger: 'keyboard_or_mouse_middle',
    keyboard_shortcut: 'Ctrl+Alt+Space',
    excluded_apps: [],
  },
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
    localStorage.clear();
    getSettingsMock.mockResolvedValue(appSettings);
    updateSettingsMock.mockImplementation(async (settings) => settings);
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

  it('响应 Manager 的设置导航事件并选中翻译分区', async () => {
    render(<SettingsPanel />);

    const translationNav = (await screen.findAllByRole('button')).find((button) => button.textContent?.includes('翻译'));
    expect(translationNav).toBeDefined();
    expect(translationNav).not.toHaveAttribute('aria-current', 'true');

    window.dispatchEvent(new CustomEvent(MANAGER_OPEN_SETTINGS_SECTION_EVENT, {
      detail: { section: 'translation' },
    }));

    await waitFor(() => expect(translationNav).toHaveAttribute('aria-current', 'true'));
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

  it('配置 Launcher 入口和排除软件后写入独立配置', async () => {
    render(<SettingsPanel />);

    const activation = await screen.findByLabelText('Launcher 唤起方式');
    fireEvent.change(activation, { target: { value: 'middleMouse' } });

    const excludedAppInput = screen.getByLabelText('添加排除软件');
    fireEvent.change(excludedAppInput, { target: { value: 'Code.exe' } });
    fireEvent.click(screen.getByRole('button', { name: '添加' }));
    expect(screen.getByText('Code.exe')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '保存配置' }));

    await waitFor(() => {
      expect(JSON.parse(localStorage.getItem('easyflowhub_launcher_settings_v1') || '{}')).toMatchObject({
        activation: 'middleMouse',
        excludedApps: [{ id: 'code.exe', executable: 'Code.exe' }],
      });
    });
  });

  it('普通保存不携带未提交的 Launcher 草稿（F08）', async () => {
    render(<SettingsPanel />);

    // 只修改 Launcher 草稿，不点“保存配置”（Launcher 域的提交入口）。
    const activation = await screen.findByLabelText('Launcher 唤起方式');
    fireEvent.change(activation, { target: { value: 'middleMouse' } });

    // 从翻译区提交普通设置。
    fireEvent.change(screen.getByLabelText('翻译模型'), { target: { value: 'relay-model' } });
    fireEvent.click(screen.getByRole('button', { name: '保存翻译设置' }));

    await waitFor(() => {
      expect(updateSettingsMock).toHaveBeenCalledWith({
        ...appSettings,
        translation: { target_language: 'zh-CN', model: 'relay-model' },
      });
    });

    // launcher_trigger 回写已加载/已提交值，而不是 middleMouse 草稿。
    const payload = updateSettingsMock.mock.calls[0][0];
    expect(payload.launcher_trigger).toEqual(appSettings.launcher_trigger);
    // Launcher 的 UI 镜像也不落盘：草稿仍在等待自己的提交入口。
    expect(localStorage.getItem('easyflowhub_launcher_settings_v1')).toBeNull();
  });

  it('保存目标语言与模型时复用应用设置且不增加密钥', async () => {
    render(<SettingsPanel />);

    const targetLanguage = await screen.findByLabelText('目标语言');
    fireEvent.change(targetLanguage, { target: { value: 'en' } });
    fireEvent.change(screen.getByLabelText('翻译模型'), { target: { value: ' relay-model ' } });
    fireEvent.click(screen.getByRole('button', { name: '保存翻译设置' }));

    await waitFor(() => {
      expect(updateSettingsMock).toHaveBeenCalledWith({
        ...appSettings,
        translation: { target_language: 'en', model: 'relay-model' },
      });
    });
    expect(screen.getByLabelText('翻译模型')).toHaveValue('relay-model');
    expect(screen.queryByLabelText(/API.*(密钥|Key)/i)).not.toBeInTheDocument();
  });

  it('旧设置缺少翻译字段时补齐默认目标语言且不预选模型', async () => {
    const { translation: _, ...legacySettings } = appSettings;
    getSettingsMock.mockResolvedValue(legacySettings as AppSettings);
    render(<SettingsPanel />);

    const targetLanguage = await screen.findByLabelText('目标语言');
    expect(targetLanguage).toHaveValue('zh-CN');
    expect(screen.getByLabelText('翻译模型')).toHaveValue('');
    fireEvent.click(screen.getByRole('button', { name: '保存翻译设置' }));

    await waitFor(() => {
      expect(updateSettingsMock).toHaveBeenCalledWith(appSettings);
    });
  });

  it('部分翻译设置补齐目标语言并保留已有模型', async () => {
    getSettingsMock.mockResolvedValue({
      ...appSettings,
      translation: { model: 'existing-model' },
    } as AppSettings);
    render(<SettingsPanel />);

    await waitFor(() => expect(screen.getByLabelText('翻译模型')).toHaveValue('existing-model'));
    expect(screen.getByLabelText('目标语言')).toHaveValue('zh-CN');
  });

  it('保存转写模型与语言时只写入偏好，不引入凭据字段', async () => {
    render(<SettingsPanel />);

    fireEvent.change(await screen.findByLabelText('音频语言'), { target: { value: 'zh' } });
    fireEvent.change(screen.getByLabelText('转写模型'), { target: { value: ' whisper-1 ' } });
    fireEvent.click(screen.getByRole('button', { name: '保存转写设置' }));

    await waitFor(() => {
      expect(updateSettingsMock).toHaveBeenCalledWith({
        ...appSettings,
        transcription: { model: 'whisper-1', language: 'zh' },
        developer: { debug_mode: false },
      });
    });
    expect(screen.getByLabelText('转写模型')).toHaveValue('whisper-1');
    expect(screen.queryByLabelText(/API.*(密钥|Key)/i)).not.toBeInTheDocument();
  });

  it('旧设置缺少转写字段时补齐自动识别与空模型', async () => {
    const { transcription: _, ...legacySettings } = appSettings;
    getSettingsMock.mockResolvedValue(legacySettings as AppSettings);
    render(<SettingsPanel />);

    expect(await screen.findByLabelText('音频语言')).toHaveValue('');
    expect(screen.getByLabelText('转写模型')).toHaveValue('');
  });

  it('设置加载失败时阻止保存，避免用默认值覆盖现有配置', async () => {
    getSettingsMock.mockRejectedValueOnce(new Error('database is locked'));
    render(<SettingsPanel />);

    expect(await screen.findByRole('alert')).toHaveTextContent('设置加载失败');

    // Only the app-settings saves are blocked; the shortcuts config uses its
    // own localStorage store and stays independent.
    const settingsSaves = screen.getAllByRole('button', { name: '保存设置' });
    expect(settingsSaves.length).toBeGreaterThan(0);
    expect(settingsSaves.every((button) => button.hasAttribute('disabled'))).toBe(true);

    fireEvent.click(settingsSaves[0]);
    expect(updateSettingsMock).not.toHaveBeenCalled();

    // Retrying a successful load re-enables saving.
    getSettingsMock.mockResolvedValueOnce(appSettings);
    fireEvent.click(screen.getByRole('button', { name: '重试加载' }));

    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
    expect(screen.getAllByRole('button', { name: '保存设置' }).some((button) => !button.hasAttribute('disabled'))).toBe(true);
  });
});
