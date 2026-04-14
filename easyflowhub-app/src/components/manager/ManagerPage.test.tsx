import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { extensionsApi } from '../../lib/api/scriptmgr';
import type { ExtensionContributionsResponse } from '../../types/scriptmgr';
import type { FeatureModule } from '../../modules';
import { useEnabledModules } from '../../modules';
import {
  MANAGER_OPEN_EXTENSION_EVENT,
  type ManagerExtensionNavigationDetail,
} from './shared/extensionNavigation';
import { ManagerPage } from './ManagerPage';

vi.mock('../../lib/api/scriptmgr', () => ({
  extensionsApi: {
    contributions: vi.fn(),
  },
}));

vi.mock('../../modules', () => ({
  useEnabledModules: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    onFocusChanged: vi.fn(async () => () => {}),
    listen: vi.fn(async () => () => {}),
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
});
