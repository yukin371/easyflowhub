import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { EffectiveManagerModuleEntry } from '../../types/scriptmgr';
import type { FeatureModule } from '../../modules';
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

const entries: EffectiveManagerModuleEntry[] = [
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
];

describe('ManagerExtensionEntries', () => {
  it('navigates only for entries mapped to existing modules', () => {
    const onPanelChange = vi.fn();

    render(
      <ManagerExtensionEntries
        entries={entries}
        enabledModules={enabledModules}
        activePanel="notes"
        onPanelChange={onPanelChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '打开 relay 面板' }));
    expect(onPanelChange).toHaveBeenCalledWith('relay');
    expect(screen.getAllByRole('button', { name: '查看扩展详情' })).toHaveLength(2);
  });
});
