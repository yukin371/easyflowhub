import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { moduleRegistry, useExperimentalModules } from '../../../modules/runtime';
import { useTranslationExperimental } from '../../../hooks/useTranslationExperimental';

/**
 * 实验性功能（预览）设置。
 *
 * 这些能力尚未稳定，默认关闭。文件 / 下载 / 录音 是模块，启用状态由 module
 * registry 持久化；翻译不是模块（是 Launcher 动作 + 设置段），用独立的
 * experimental 标记。开启时给出「开发中」提示。
 */
export function ExperimentalSection() {
  const experimentalModules = useExperimentalModules();
  const translation = useTranslationExperimental();

  const handleToggleModule = async (moduleId: string, enabled: boolean) => {
    await moduleRegistry.toggleModule(moduleId, enabled);
  };

  return (
    <SettingsSectionCard
      kicker="Experimental"
      title="实验性功能"
      description="以下功能仍在开发中，可能无法正常使用或随时变化。默认关闭，可按需手动开启。"
    >
      <div
        role="note"
        className="mb-4 flex items-start gap-3 rounded-[14px] border border-[color:var(--manager-warning)]/40 bg-[color:var(--manager-warning)]/10 px-4 py-3"
      >
        <span aria-hidden="true" className="mt-0.5 text-base text-[color:var(--manager-warning)]">⚠</span>
        <p className="text-xs leading-6 text-[color:var(--manager-ink-soft)]">
          开发中功能：这些能力尚未定型，界面与行为可能在后续版本调整，数据或结果可能不可用。仅供试用，请勿依赖它完成重要工作。
        </p>
      </div>

      <div className="space-y-2">
        {experimentalModules.map((module) => {
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
                  <p className="flex items-center gap-2 text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    {module.name}
                    <span className="rounded-[5px] border border-[color:var(--manager-warning)]/40 px-1.5 py-0.5 text-[10px] text-[color:var(--manager-warning)]">
                      实验
                    </span>
                  </p>
                  <p className="text-xs text-[color:var(--manager-ink-subtle)]">
                    {module.caption}
                  </p>
                </div>
              </div>
              <button
                onClick={() => void handleToggleModule(module.id, !isEnabled)}
                aria-label={`切换 ${module.name}`}
                aria-pressed={isEnabled}
                className={`relative h-6 w-11 rounded-full transition-colors ${
                  isEnabled ? 'bg-[color:var(--manager-accent)]' : 'bg-[color:var(--manager-border)]'
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

        {/* 翻译不是模块，用独立 experimental 标记门控。 */}
        <div className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
              译
            </span>
            <div>
              <p className="flex items-center gap-2 text-sm font-medium text-[color:var(--manager-ink-strong)]">
                翻译
                <span className="rounded-[5px] border border-[color:var(--manager-warning)]/40 px-1.5 py-0.5 text-[10px] text-[color:var(--manager-warning)]">
                  实验
                </span>
              </p>
              <p className="text-xs text-[color:var(--manager-ink-subtle)]">
                Translation
              </p>
            </div>
          </div>
          <button
            onClick={() => translation.setEnabled(!translation.enabled)}
            aria-label="切换 翻译"
            aria-pressed={translation.enabled}
            className={`relative h-6 w-11 rounded-full transition-colors ${
              translation.enabled ? 'bg-[color:var(--manager-accent)]' : 'bg-[color:var(--manager-border)]'
            }`}
          >
            <span
              className={`absolute top-1 h-4 w-4 rounded-full bg-[color:var(--manager-ink-strong)] transition-transform ${
                translation.enabled ? 'left-6' : 'left-1'
              }`}
            />
          </button>
        </div>
      </div>
    </SettingsSectionCard>
  );
}
