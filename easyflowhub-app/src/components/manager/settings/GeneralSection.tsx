import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { useAppearance } from '../../../hooks/useAppearance';
import { THEME_OPTIONS, type ThemeMode } from '../../../types/appearance';
import { useAutostartSettings } from './useAutostartSettings';

/** 通用设置：开机自启动与界面主题。 */
export function GeneralSection() {
  const autostart = useAutostartSettings();
  const { config: appearanceConfig, setTheme } = useAppearance();

  return (
    <SettingsSectionCard
      kicker="General"
      title="通用"
      description="应用启动行为等基础配置。"
    >
      <div className="space-y-2">
        <div className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
              🚀
            </span>
            <div>
              <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
                开机自启动
              </p>
              <p className="text-xs text-[color:var(--manager-ink-subtle)]">
                系统启动时自动运行 EasyFlowHub
              </p>
            </div>
          </div>
          <button
            onClick={() => void autostart.toggle()}
            className={`relative h-6 w-11 rounded-full transition-colors ${
              autostart.enabled
                ? 'bg-[color:var(--manager-accent)]'
                : 'bg-[color:var(--manager-border)]'
            }`}
          >
            <span
              className={`absolute top-1 h-4 w-4 rounded-full bg-[color:var(--manager-ink-strong)] transition-transform ${
                autostart.enabled ? 'left-6' : 'left-1'
              }`}
            />
          </button>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-4 rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
              ◐
            </span>
            <div>
              <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">界面主题</p>
              <p className="text-xs text-[color:var(--manager-ink-subtle)]">切换深色、浅色或跟随 Windows 设置。</p>
            </div>
          </div>
          <div className="flex items-center gap-1 rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-1" role="group" aria-label="界面主题">
            {THEME_OPTIONS.map((option) => {
              const selected = appearanceConfig?.theme === option.id;
              return (
                <button
                  key={option.id}
                  type="button"
                  aria-pressed={selected}
                  onClick={() => setTheme(option.id as ThemeMode)}
                  className={`rounded-[10px] px-2.5 py-1.5 text-xs transition ${
                    selected
                      ? 'bg-[color:var(--manager-accent)] text-white shadow-sm'
                      : 'text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel-strong)]'
                  }`}
                >
                  <span aria-hidden="true">{option.icon}</span>
                  <span className="ml-1">{option.name}</span>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </SettingsSectionCard>
  );
}
