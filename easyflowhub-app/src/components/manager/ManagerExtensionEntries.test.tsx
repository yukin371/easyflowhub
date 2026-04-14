import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { EffectiveManagerModuleEntry } from '../../types/scriptmgr';
import type { FeatureModule } from '../../modules';
import {
  MANAGER_NAVIGATE_TO_EXTENSION_EVENT,
  type ManagerExtensionNavigationDetail,
} from './shared/extensionNavigation';
import { ManagerExtensionEntries } from './ManagerExtensionEntries';

const enabledModules: FeatureModule[] = [
  {
    id: 'relay',
    name: 'Relay',
    icon: 'RL',
    caption: 'RELAY',
    defaultEnabled: true,
    component: () => null,
  },
  {
    id: 'extensions',
    name: '扩展',
    icon: '拓',
    caption: 'Extensions',
    defaultEnabled: true,
    component: () => null,
  },
];

const builtinEntry: EffectiveManagerModuleEntry = {
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
};

const fallbackEntry: EffectiveManagerModuleEntry = {
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
};

describe('ManagerExtensionEntries', () => {
  it('命中 builtin id 时只跳转对应面板，不生成额外 sidebar builtin 项', () => {
    const onPanelChange = vi.fn();

    render(
      <>
        <div data-testid="workspace-modules">
          {enabledModules.map((module) => (
            <span key={module.id}>{module.name}</span>
          ))}
        </div>
        <ManagerExtensionEntries
          entries={[builtinEntry]}
          enabledModules={enabledModules}
          activePanel="notes"
          onPanelChange={onPanelChange}
        />
      </>
    );

    fireEvent.click(screen.getByRole('button', { name: '打开 relay 面板' }));

    expect(onPanelChange).toHaveBeenCalledWith('relay');
    expect(screen.getByTestId('workspace-modules')).toHaveTextContent('Relay');
    expect(screen.getByTestId('workspace-modules')).toHaveTextContent('扩展');
    expect(screen.getByTestId('workspace-modules')).not.toHaveTextContent('Relay Shortcut');
    expect(screen.getByRole('button', { name: '查看扩展详情' })).toBeInTheDocument();
  });

  it('未命中 builtin 时回退到 extensions 面板，并保留扩展详情按钮', () => {
    const onPanelChange = vi.fn();

    render(
      <ManagerExtensionEntries
        entries={[fallbackEntry]}
        enabledModules={enabledModules}
        activePanel="notes"
        onPanelChange={onPanelChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '打开 extensions 面板' }));

    expect(onPanelChange).toHaveBeenCalledWith('extensions');
    expect(screen.getByRole('button', { name: '查看扩展详情' })).toBeInTheDocument();
  });

  it('查看扩展详情按钮会派发扩展详情导航事件', () => {
    const onPanelChange = vi.fn();
    const detailHandler = vi.fn<(event: Event) => void>();

    window.addEventListener(MANAGER_NAVIGATE_TO_EXTENSION_EVENT, detailHandler);

    render(
      <ManagerExtensionEntries
        entries={[fallbackEntry]}
        enabledModules={enabledModules}
        activePanel="notes"
        onPanelChange={onPanelChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '查看扩展详情' }));

    expect(detailHandler).toHaveBeenCalledTimes(1);
    const [event] = detailHandler.mock.calls[0] ?? [];
    expect(event).toBeInstanceOf(CustomEvent);
    expect((event as CustomEvent<ManagerExtensionNavigationDetail>).detail).toEqual({
      extensionId: 'future-pack',
    });
    expect(onPanelChange).not.toHaveBeenCalled();

    window.removeEventListener(MANAGER_NAVIGATE_TO_EXTENSION_EVENT, detailHandler);
  });

  it('不把 manager_modules 贡献项当作 builtin workspace 模块来源', () => {
    const onPanelChange = vi.fn();

    render(
      <ManagerExtensionEntries
        entries={[fallbackEntry]}
        enabledModules={enabledModules.filter((module) => module.id !== 'extensions')}
        activePanel="notes"
        onPanelChange={onPanelChange}
      />
    );

    expect(screen.queryByRole('button', { name: '打开 future-panel 面板' })).not.toBeInTheDocument();
    expect(screen.getByText('当前阶段没有直接面板映射。')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看扩展详情' })).toBeInTheDocument();
    expect(onPanelChange).not.toHaveBeenCalled();
  });
});
