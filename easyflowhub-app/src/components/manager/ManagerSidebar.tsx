/**
 * ManagerSidebar - 管理中心侧边栏导航
 * 支持动态模块渲染
 */

import type { FeatureModule } from '../../modules';

interface ManagerSidebarProps {
  modules: FeatureModule[];
  activePanel: string;
  onPanelChange: (panelId: string) => void;
}

export function ManagerSidebar({ modules, activePanel, onPanelChange }: ManagerSidebarProps) {
  return (
    <aside className="hidden min-w-[196px] max-w-[196px] flex-col gap-3 rounded-[20px] bg-[rgba(255,251,245,0.44)] px-3 py-3 lg:flex">
      <div className="space-y-2 px-1 py-1">
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

      <nav className="space-y-2 px-1 py-1">
        <p className="manager-kicker">Workspace</p>
        <div className="space-y-2 pt-1">
          {modules.map((module) => {
            const isActive = activePanel === module.id;
            return (
              <button
                key={module.id}
                onClick={() => onPanelChange(module.id)}
                className={`group flex w-full items-center gap-3 rounded-[18px] border px-3 py-2.5 text-left transition-all duration-200 ${
                  isActive
                    ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)] shadow-[0_18px_40px_rgba(46,58,49,0.12)]'
                    : 'border-transparent bg-transparent text-[color:var(--manager-ink-muted)] hover:border-[color:var(--manager-border)] hover:bg-white/55 hover:text-[color:var(--manager-ink)]'
                }`}
                title={module.name}
              >
                <span
                  className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-[14px] border text-sm ${
                    isActive
                      ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-white'
                      : 'border-[color:var(--manager-border)] bg-white/70 text-[color:var(--manager-ink-soft)]'
                  }`}
                >
                  {module.icon}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium tracking-[0.08em] text-current">
                    {module.name}
                  </span>
                  <span className="mt-0.5 block text-[11px] uppercase tracking-[0.18em] text-[color:var(--manager-ink-subtle)]">
                    {module.caption}
                  </span>
                </span>
              </button>
            );
          })}
        </div>
      </nav>

      <div className="mt-auto px-1 pb-1">
        <p className="text-[11px] leading-5 tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
          Zen layout
        </p>
      </div>
    </aside>
  );
}

// 保持向后兼容的类型导出（废弃，但保留以避免破坏性更改）
export type ManagerPanel = 'notes' | 'todos' | 'scripts' | 'relay' | 'mcp' | 'tasks' | 'components' | 'trash' | 'settings';
