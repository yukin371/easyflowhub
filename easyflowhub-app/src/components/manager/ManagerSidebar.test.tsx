import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { FeatureModule } from '../../modules';
import { ManagerSidebar } from './ManagerSidebar';

function module(id: string, name: string, icon: string): FeatureModule {
  return {
    id,
    name,
    icon,
    caption: name.toUpperCase(),
    defaultEnabled: true,
    component: () => <div>{name}</div>,
  };
}

const modules: FeatureModule[] = [
  module('notes', '笔记', '墨'),
  module('scripts', '脚本', '策'),
  module('settings', '设置', '⚙'),
];

describe('ManagerSidebar', () => {
  it('pins settings below the workspace list with its own icon', () => {
    render(
      <ManagerSidebar
        modules={modules}
        extensionEntries={[]}
        activePanel="notes"
        onPanelChange={vi.fn()}
      />
    );

    const notesButton = screen.getByRole('button', { name: /笔记/ });
    const settingsButton = screen.getByRole('button', { name: /设置/ });

    // Settings must come after every workspace entry in document order.
    expect(notesButton.compareDocumentPosition(settingsButton) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(settingsButton.textContent).toContain('⚙');
    expect(settingsButton.textContent).not.toContain('序');
  });

  it('reports the settings panel id when the pinned entry is clicked', () => {
    const onPanelChange = vi.fn();
    render(
      <ManagerSidebar
        modules={modules}
        extensionEntries={[]}
        activePanel="notes"
        onPanelChange={onPanelChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /设置/ }));
    expect(onPanelChange).toHaveBeenCalledWith('settings');
  });

  it('marks only the active panel and keeps settings reachable when it is active', () => {
    render(
      <ManagerSidebar
        modules={modules}
        extensionEntries={[]}
        activePanel="settings"
        onPanelChange={vi.fn()}
      />
    );

    const settingsButton = screen.getByRole('button', { name: /设置/ });
    expect(settingsButton.className).toContain('border-[color:var(--manager-accent)]');
    expect(screen.getByRole('button', { name: /笔记/ }).className).not.toContain(
      'border-[color:var(--manager-accent)]'
    );
  });
});
