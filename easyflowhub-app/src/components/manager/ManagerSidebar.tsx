/**
 * ManagerSidebar - 管理中心侧边栏导航
 * 支持动态模块渲染
 */

import type { EffectiveExtensionContributions } from '../../types/scriptmgr';
import type { FeatureModule } from '../../modules/runtime';
import { ManagerExtensionEntries } from './ManagerExtensionEntries';

interface ManagerSidebarProps {
  modules: FeatureModule[];
  extensionEntries: NonNullable<EffectiveExtensionContributions['manager_modules']>;
  activePanel: string;
  onPanelChange: (panelId: string) => void;
}

/** Settings is pinned to the sidebar footer so it is always one click away. */
const SETTINGS_MODULE_ID = 'settings';

interface NavButtonProps {
  module: FeatureModule;
  isActive: boolean;
  onSelect: (panelId: string) => void;
}

function NavButton({ module, isActive, onSelect }: NavButtonProps) {
  return (
    <button
      type="button"
      onClick={() => onSelect(module.id)}
      className={`group flex w-full shrink-0 items-center gap-3 rounded-[18px] border px-3 py-2.5 text-left transition-all duration-200 ${
        isActive
          ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)] shadow-[0_18px_40px_rgba(46,58,49,0.12)]'
          : 'border-transparent bg-transparent text-[color:var(--manager-ink-muted)] hover:border-[color:var(--manager-border)] hover:bg-[color:var(--manager-accent-soft)] hover:text-[color:var(--manager-ink)]'
      }`}
      title={module.name}
    >
      <span
        className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-[14px] border text-sm ${
          isActive
            ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-white'
            : 'border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-[color:var(--manager-ink-soft)]'
        }`}
      >
        {module.icon}
      </span>
      <span className="min-w-0 flex-1 overflow-hidden">
        <span className="block truncate whitespace-nowrap text-sm font-medium tracking-[0.08em] text-current">
          {module.name}
        </span>
        <span className="mt-0.5 block truncate whitespace-nowrap text-[11px] uppercase tracking-[0.18em] text-[color:var(--manager-ink-subtle)]">
          {module.caption}
        </span>
      </span>
    </button>
  );
}

export function ManagerSidebar({
  modules,
  extensionEntries,
  activePanel,
  onPanelChange,
}: ManagerSidebarProps) {
  const settingsModule = modules.find((module) => module.id === SETTINGS_MODULE_ID);
  const workspaceModules = modules.filter((module) => module.id !== SETTINGS_MODULE_ID);

  return (
    <aside className="hidden w-[224px] min-w-[224px] max-w-[224px] shrink-0 flex-col gap-3 overflow-hidden rounded-[16px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-3 py-3 md:flex">
      <div className="shrink-0 space-y-2 px-1 py-1">
        <div>
          <p className="manager-kicker">EasyFlowHub</p>
          <h1 className="mt-1 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[26px] leading-[1.08] text-[color:var(--manager-ink-strong)]">
            管理中心
          </h1>
        </div>
        <p className="text-sm leading-6 text-[color:var(--manager-ink-soft)]">
          整理笔记、脚本与配置。
        </p>
      </div>

      <div className="manager-sidebar-scroll min-h-0 flex-1 overflow-y-auto overscroll-contain pr-1">
        <nav className="space-y-2 px-1 py-1">
          <p className="manager-kicker">Workspace</p>
          <div className="space-y-2 pt-1">
            {workspaceModules.map((module) => (
              <NavButton
                key={module.id}
                module={module}
                isActive={activePanel === module.id}
                onSelect={onPanelChange}
              />
            ))}
          </div>
        </nav>

        <ManagerExtensionEntries
          entries={extensionEntries}
          enabledModules={modules}
          activePanel={activePanel}
          onPanelChange={onPanelChange}
        />
      </div>

      <div className="shrink-0 border-t border-[color:var(--manager-border)] px-1 pt-2">
        {settingsModule ? (
          <NavButton
            module={settingsModule}
            isActive={activePanel === settingsModule.id}
            onSelect={onPanelChange}
          />
        ) : null}
        <p className="mt-1 px-1 text-[11px] leading-5 tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
          Zen layout
        </p>
      </div>
    </aside>
  );
}

// 保持向后兼容的类型导出（废弃，但保留以避免破坏性更改）
export type ManagerPanel =
  | 'notes'
  | 'todos'
  | 'scripts'
  | 'extensions'
  | 'relay'
  | 'mcp'
  | 'tasks'
  | 'components'
  | 'trash'
  | 'settings';
