import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { NumberSettingInput } from './NumberSettingInput';
import type { AppSettingsController } from './useAppSettings';

interface TrashSectionProps {
  app: AppSettingsController;
}

/** 回收站保留期（普通设置域）。 */
export function TrashSection({ app }: TrashSectionProps) {
  return (
    <SettingsSectionCard
      kicker="Trash"
      title="回收站"
      description="回收站中的笔记在指定天数后将自动永久删除。"
    >
      <div className="mt-6">
        <NumberSettingInput
          label="保留天数"
          min={1}
          max={90}
          fallback={30}
          commitOn="change"
          value={app.settings.trash.retention_days}
          commit={(retentionDays) => app.patch('trash', { retention_days: retentionDays })}
          hint="建议设置 7-30 天。超过保留期的笔记将被自动清除。"
        />
      </div>

      <div className="mt-4 flex items-center gap-4">
        <button
          onClick={() => void app.save()}
          disabled={app.loadState !== 'ready'}
          className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
        >
          保存设置
        </button>
        <span className="text-xs text-[color:var(--manager-ink-subtle)]">
          {app.saveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
        </span>
      </div>
    </SettingsSectionCard>
  );
}
