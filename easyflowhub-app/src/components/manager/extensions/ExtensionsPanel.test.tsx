import '@testing-library/jest-dom/vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { extensionsApi, mcpApi } from '../../../lib/api/scriptmgr';
import type {
  ExtensionContributionsResponse,
  ExtensionsResponse,
  MCPServerCatalogEntry,
} from '../../../types/scriptmgr';
import {
  MANAGER_OPEN_EXTENSION_EVENT,
  type ManagerExtensionNavigationDetail,
} from '../shared/extensionNavigation';
import { ExtensionsPanel } from './ExtensionsPanel';

vi.mock('../../../lib/api/scriptmgr', () => ({
  extensionsApi: {
    list: vi.fn(),
    contributions: vi.fn(),
  },
  mcpApi: {
    listServers: vi.fn(),
  },
}));

const listExtensionsMock = vi.mocked(extensionsApi.list);
const listContributionsMock = vi.mocked(extensionsApi.contributions);
const listServersMock = vi.mocked(mcpApi.listServers);

const extensionsResponse: ExtensionsResponse = {
  ok: true,
  roots: ['extensions'],
  count: 2,
  extensions: [
    {
      manifest_path: 'extensions/sample/plugin.json',
      root: 'extensions',
      status: 'loaded',
      manifest: {
        id: 'sample-pack',
        name: 'Sample Pack',
        version: '1.0.0',
        description: 'Loaded extension',
      },
    },
    {
      manifest_path: 'extensions/broken/plugin.json',
      root: 'extensions',
      status: 'error',
      error: 'invalid manifest',
    },
  ],
};

const contributionsResponse: ExtensionContributionsResponse = {
  ok: true,
  roots: ['extensions'],
  contributions: {
    manager_modules: [
      {
        id: 'relay',
        name: 'Relay Shortcut',
        caption: 'relay',
        icon: 'R',
        description: 'open relay panel',
        source: {
          extension_id: 'sample-pack',
          extension_name: 'Sample Pack',
          extension_version: '1.0.0',
          manifest_path: 'extensions/sample/plugin.json',
          root: 'extensions',
        },
      },
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
    script_roots: [
      {
        path: 'extensions/sample/scripts',
        source: {
          extension_id: 'sample-pack',
          extension_name: 'Sample Pack',
          extension_version: '1.0.0',
          manifest_path: 'extensions/sample/plugin.json',
          root: 'extensions',
        },
      },
    ],
    relay_providers: [],
    relay_routes: [],
    mcp_servers: [],
  },
};

const emptyContributionsResponse: ExtensionContributionsResponse = {
  ok: true,
  roots: [],
  contributions: {
    manager_modules: [],
    script_roots: [],
    relay_providers: [],
    relay_routes: [],
    mcp_servers: [],
  },
};

const serversResponse: MCPServerCatalogEntry[] = [
  {
    key: 'sample-server',
    name: 'Sample Server',
    command: 'sample',
    status: 'extension',
    source: 'extension:sample-pack',
  },
  {
    key: 'conflicted-server',
    name: 'Conflicted Server',
    command: 'conflicted',
    status: 'conflicted',
    source: 'extension:future-pack',
    conflict_with: 'persisted:conflicted-server',
  },
];

const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;

describe('ExtensionsPanel', () => {
  beforeEach(() => {
    listExtensionsMock.mockResolvedValue(extensionsResponse);
    listContributionsMock.mockResolvedValue(contributionsResponse);
    listServersMock.mockResolvedValue(serversResponse);

    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });

    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback: FrameRequestCallback) => {
      callback(0);
      return 0;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
      configurable: true,
      value: originalScrollIntoView,
    });
  });

  it('将 manager entry 贡献渲染为只读审计目录，而不是第二个可操作入口', async () => {
    render(<ExtensionsPanel />);

    await screen.findByText('Relay Shortcut');

    expect(screen.getByText('只读目录，操作入口仅保留在侧边栏')).toBeInTheDocument();
    expect(
      screen.getByText('这里只保留扩展声明、来源与受控映射审计；不再提供第二个 manager entry 可操作入口。')
    ).toBeInTheDocument();
    expect(screen.getAllByText('ManagerSidebar → ManagerExtensionEntries')).toHaveLength(2);
    expect(screen.getAllByText('仅允许受控宿主切换到既有 builtin 面板')).toHaveLength(2);
    expect(screen.queryByRole('button', { name: /查看扩展详情/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /打开 .* 面板/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '刷新' })).toBeInTheDocument();
  });

  it('按来源扩展的加载状态标记 manager entry audit status', async () => {
    render(<ExtensionsPanel />);

    await screen.findByText('Future Panel');

    expect(screen.getByText('来源扩展已加载')).toBeInTheDocument();
    expect(screen.getByText('来源扩展当前未解析')).toBeInTheDocument();
  });

  it('响应扩展详情深链事件并滚动到目标扩展卡片', async () => {
    render(<ExtensionsPanel />);

    const title = await screen.findByText('Sample Pack');
    const targetCard = title.closest('[data-extension-id="sample-pack"]') as HTMLElement | null;

    expect(targetCard).not.toBeNull();

    window.dispatchEvent(
      new CustomEvent<ManagerExtensionNavigationDetail>(MANAGER_OPEN_EXTENSION_EVENT, {
        detail: { extensionId: 'sample-pack' },
      })
    );

    await waitFor(() => {
      expect(HTMLElement.prototype.scrollIntoView).toHaveBeenCalled();
    });
    expect(targetCard?.className).toContain('border-[color:var(--manager-accent)]');
  });

  it('没有 manager entry contribution 时显示只读审计空态', async () => {
    listContributionsMock.mockResolvedValueOnce(emptyContributionsResponse);

    render(<ExtensionsPanel />);

    await screen.findByText('当前没有 manager entry audit 条目。');

    expect(screen.getByText('extensions')).toBeInTheDocument();
    expect(screen.getByText('当前没有 manager entry audit 条目。')).toBeInTheDocument();
    expect(screen.queryByText('Relay Shortcut')).not.toBeInTheDocument();
  });

  it('扩展贡献接口失败时保留错误提示，并继续显示已加载的扩展列表', async () => {
    listContributionsMock.mockRejectedValueOnce(new Error('加载扩展贡献失败'));

    render(<ExtensionsPanel />);

    await screen.findByText('加载扩展贡献失败');

    expect(screen.getByText('加载扩展贡献失败')).toBeInTheDocument();
    expect(screen.getByText('Sample Pack')).toBeInTheDocument();
    expect(screen.getByText('当前没有 manager entry audit 条目。')).toBeInTheDocument();
  });

  it('扩展列表接口失败时仍保留 contribution 审计信息与 roots 回退', async () => {
    listExtensionsMock.mockRejectedValueOnce(new Error('加载扩展列表失败'));

    render(<ExtensionsPanel />);

    await screen.findByText('加载扩展列表失败');

    expect(screen.getByText('加载扩展列表失败')).toBeInTheDocument();
    expect(screen.getByText('Relay Shortcut')).toBeInTheDocument();
    expect(screen.getByText('extensions')).toBeInTheDocument();
    expect(screen.getAllByText('来源扩展当前未解析')).toHaveLength(2);
  });
});
