import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { moduleRegistry, useToggleableModules } from '../../../modules/runtime';

/** 功能模块设置：复用 module registry，不在面板内自建启停状态。 */
export function ModulesSection() {
  const toggleableModules = useToggleableModules();

  // 切换模块启用状态
  const handleToggleModule = async (moduleId: string, enabled: boolean) => {
    await moduleRegistry.toggleModule(moduleId, enabled);
  };

  return (
    <SettingsSectionCard
      kicker="Modules"
      title="功能模块"
      description="启用或禁用应用功能模块。禁用的模块不会显示在侧边栏中。"
    >
      <div className="space-y-2">
        {toggleableModules.map((module) => {
          const isEnabled = moduleRegistry.isEnabled(module.id);
          return (
            <div
              key={module.id}
              className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3"
            >
              <div className="flex items-center gap-3">
                <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
                  {module.icon}
                </span>
                <div>
                  <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    {module.name}
                  </p>
                  <p className="text-xs text-[color:var(--manager-ink-subtle)]">
                    {module.caption}
                  </p>
                </div>
              </div>
              <button
                onClick={() => void handleToggleModule(module.id, !isEnabled)}
                className={`relative h-6 w-11 rounded-full transition-colors ${
                  isEnabled
                    ? 'bg-[color:var(--manager-accent)]'
                    : 'bg-[color:var(--manager-border)]'
                }`}
              >
                <span
                  className={`absolute top-1 h-4 w-4 rounded-full bg-[color:var(--manager-ink-strong)] transition-transform ${
                    isEnabled ? 'left-6' : 'left-1'
                  }`}
                />
              </button>
            </div>
          );
        })}
      </div>

      {toggleableModules.length === 0 && (
        <p className="text-sm text-[color:var(--manager-ink-subtle)]">
          所有模块已启用。
        </p>
      )}
    </SettingsSectionCard>
  );
}
