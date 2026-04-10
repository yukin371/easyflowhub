import type { EffectiveExtensionContributions } from '../../types/scriptmgr';
import type { FeatureModule as ManagerFeatureModule } from '../../modules';
import { navigateToManagerExtension } from './shared/extensionNavigation';

type ExtensionManagerEntry = NonNullable<
  EffectiveExtensionContributions['manager_modules']
>[number];

interface ManagerExtensionEntriesProps {
  entries: ExtensionManagerEntry[];
  enabledModules: ManagerFeatureModule[];
  activePanel: string;
  onPanelChange: (panelId: string) => void;
}

function resolveTargetPanel(
  entry: ExtensionManagerEntry,
  enabledModules: ManagerFeatureModule[]
): string | null {
  const match = enabledModules.find((module) => module.id === entry.id);
  if (match) {
    return match.id;
  }

  const extensionsPanel = enabledModules.find((module) => module.id === 'extensions');
  return extensionsPanel?.id ?? null;
}

export function ManagerExtensionEntries({
  entries,
  enabledModules,
  activePanel,
  onPanelChange,
}: ManagerExtensionEntriesProps) {
  if (entries.length === 0) {
    return null;
  }

  return (
    <section className="space-y-2 px-1 py-1">
      <div>
        <p className="manager-kicker">Extensions</p>
        <p className="mt-1 text-xs leading-5 text-[color:var(--manager-ink-subtle)]">
          受控 extension entry，只能跳转到已有面板。
        </p>
      </div>

      <div className="space-y-2">
        {entries.map((entry) => {
          const targetPanel = resolveTargetPanel(entry, enabledModules);
          const canNavigate = Boolean(targetPanel);
          const isActive = canNavigate && activePanel === targetPanel;

          return (
            <div
              key={`${entry.source.extension_id}:${entry.id}`}
              className="rounded-[16px] border border-[color:var(--manager-border)] bg-white/45 px-3 py-3"
            >
              <div className="flex items-start justify-between gap-2">
                <div className="min-w-0">
                  <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    {entry.name}
                  </p>
                  <p className="mt-0.5 text-[11px] uppercase tracking-[0.16em] text-[color:var(--manager-ink-subtle)]">
                    {entry.caption || entry.id}
                  </p>
                </div>
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-white/70 text-sm text-[color:var(--manager-ink-soft)]">
                  {entry.icon || 'EX'}
                </span>
              </div>

              {entry.description && (
                <p className="mt-2 text-xs leading-5 text-[color:var(--manager-ink-soft)]">
                  {entry.description}
                </p>
              )}

              <p className="mt-2 break-all text-[11px] leading-5 text-[color:var(--manager-ink-subtle)]">
                source: {entry.source.extension_name} ({entry.source.extension_id})
              </p>

              <div className="mt-3 flex gap-2">
                {canNavigate ? (
                  <button
                    onClick={() => {
                      if (targetPanel) {
                        onPanelChange(targetPanel);
                      }
                    }}
                    className={`flex-1 rounded-full border px-3 py-2 text-xs transition ${
                      isActive
                        ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-accent)]'
                        : 'border-[color:var(--manager-border)] bg-white/70 text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]'
                    }`}
                  >
                    {isActive && targetPanel === entry.id ? '当前已在对应面板' : `打开 ${targetPanel} 面板`}
                  </button>
                ) : (
                  <p className="flex-1 rounded-[12px] border border-dashed border-[color:var(--manager-border)] px-3 py-2 text-[11px] leading-5 text-[color:var(--manager-ink-subtle)]">
                    当前阶段没有直接面板映射。
                  </p>
                )}

                <button
                  onClick={() => navigateToManagerExtension(entry.source.extension_id)}
                  className="rounded-full border border-[color:var(--manager-accent)] bg-white px-3 py-2 text-xs text-[color:var(--manager-accent)] transition hover:bg-[color:var(--manager-accent)] hover:text-white"
                >
                  查看扩展详情
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
