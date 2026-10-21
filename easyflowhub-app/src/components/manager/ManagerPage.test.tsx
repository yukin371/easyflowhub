import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { extensionsApi } from '../../lib/api/scriptmgr';
import type { ExtensionContributionsResponse } from '../../types/scriptmgr';
import type { FeatureModule } from '../../modules/runtime';
import { useEnabledModules } from '../../modules/runtime';
import {
  MANAGER_OPEN_EXTENSION_EVENT,
  type ManagerExtensionNavigationDetail,
} from './shared/extensionNavigation';
import {
  MANAGER_NAVIGATE_TO_SETTINGS_EVENT,
  MANAGER_OPEN_SETTINGS_SECTION_EVENT,
  type ManagerSettingsNavigationDetail,
} from '../../lib/tauri/manager';
import { ManagerPage } from './ManagerPage';

vi.mock('../../lib/api/scriptmgr', () => ({
  extensionsApi: {
    contributions: vi.fn(),
  },
}));

vi.mock('../../modules/runtime', () => ({
  useEnabledModules: vi.fn(),
}));

const windowState = vi.hoisted(() => {
  const listeners = new Map<string, (event: unknown) => void>();
  return {
    listeners,
    listen: vi.fn(async (event: string, handler: (event: unknown) => void) => {
      listeners.set(event, handler);
      return () => listeners.delete(event);
    }),
  };
});

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    onFocusChanged: vi.fn(async () => () => {}),
    listen: windowState.listen,
  })),
}));

const contributionsMock = vi.mocked(extensionsApi.contributions);
const useEnabledModulesMock = vi.mocked(useEnabledModules);

const enabledModules: FeatureModule[] = [
  {
    id: 'notes',
    name: 'Notes',
    icon: 'N',
    caption: 'NOTES',
    defaultEnabled: true,
    component: () => <div>Notes panel</div>,
  },
  {
    id: 'extensions',
    name: '扩展',
    icon: '拓',
    caption: 'Extensions',
    defaultEnabled: true,
    component: () => <div>Extensions panel</div>,
  },
];

const contributionsResponse: ExtensionContributionsResponse = {
  ok: true,
  roots: ['extensions'],
  contributions: {
    manager_modules: [
      {
        id: 'future-panel',
        name: 'Future Panel',
        caption: 'future',
        icon: 'F',
        description: 'not mapped yet',
        source: {
          extension_id: 'future-pack',
          extension_name: 'Future Pack',
          extension_version: '1.0.0',
          manifest_path: 'extensions/future/plugin.json',
          root: 'extensions',
        },
      },
    ],
  },
};

describe('ManagerPage', () => {
  beforeEach(() => {
    windowState.listeners.clear();
    useEnabledModulesMock.mockReturnValue(enabledModules);
    contributionsMock.mockResolvedValue(contributionsResponse);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('在页面级联动中通过 extension entry 详情按钮切换到 extensions 面板并派发打开事件', async () => {
    const detailHandler = vi.fn<(event: Event) => void>();
    window.addEventListener(MANAGER_OPEN_EXTENSION_EVENT, detailHandler);

    render(<ManagerPage />);

    await screen.findByText('Future Panel');
    expect(screen.getByText('Notes panel')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '查看扩展详情' }));
    await waitFor(() => {
      expect(screen.getByText('Extensions panel')).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(detailHandler).toHaveBeenCalledTimes(1);
    });
    const [event] = detailHandler.mock.calls[0] ?? [];
    expect((event as CustomEvent<ManagerExtensionNavigationDetail>).detail).toEqual({
      extensionId: 'future-pack',
    });

    window.removeEventListener(MANAGER_OPEN_EXTENSION_EVENT, detailHandler);
  });

  it('收到跨窗口设置导航后切换 Manager 并转发翻译分区', async () => {
    const settingsModule: FeatureModule = {
      id: 'settings',
      name: '设置',
      icon: '⚙',
      caption: 'Settings',
      defaultEnabled: true,
      component: () => <div>Settings panel</div>,
    };
    useEnabledModulesMock.mockReturnValue([...enabledModules, settingsModule]);
    const detailHandler = vi.fn<(event: Event) => void>();
    window.addEventListener(MANAGER_OPEN_SETTINGS_SECTION_EVENT, detailHandler);

    render(<ManagerPage />);
    await waitFor(() => expect(windowState.listeners.has(MANAGER_NAVIGATE_TO_SETTINGS_EVENT)).toBe(true));
    windowState.listeners.get(MANAGER_NAVIGATE_TO_SETTINGS_EVENT)?.({
      payload: { section: 'translation' },
    } satisfies { payload: ManagerSettingsNavigationDetail });

    expect(await screen.findByText('Settings panel')).toBeInTheDocument();
    await waitFor(() => expect(detailHandler).toHaveBeenCalledTimes(1));
    const [event] = detailHandler.mock.calls[0] ?? [];
    expect((event as CustomEvent<ManagerSettingsNavigationDetail>).detail).toEqual({ section: 'translation' });

    window.removeEventListener(MANAGER_OPEN_SETTINGS_SECTION_EVENT, detailHandler);
  });

  it('面板渲染异常时保留侧边栏并给出可恢复的错误提示', async () => {
    const brokenModule: FeatureModule = {
      id: 'broken',
      name: 'Broken',
      icon: 'B',
      caption: 'BROKEN',
      defaultEnabled: true,
      component: () => {
        throw new Error('panel exploded');
      },
    };
    useEnabledModulesMock.mockReturnValue([...enabledModules, brokenModule]);

    render(<ManagerPage />);
    expect(await screen.findByText('Notes panel')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Broken/ }));

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('该面板加载失败，其他页面仍可正常使用。');
    expect(alert).toHaveTextContent('panel exploded');

    // The shell must survive: sidebar entries remain clickable.
    expect(screen.getByRole('button', { name: /Notes/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Broken/ })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Notes/ }));
    expect(await screen.findByText('Notes panel')).toBeInTheDocument();
  });
});
