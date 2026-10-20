import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { NumberSettingInput } from './NumberSettingInput';
import type { AppSettingsController } from './useAppSettings';

interface TodoSectionProps {
  app: AppSettingsController;
}

/** 待办保留期（普通设置域）。 */
export function TodoSection({ app }: TodoSectionProps) {
  return (
    <SettingsSectionCard
      kicker="Todos"
      title="待办"
      description="已完成的待办会在保留期内以删除线显示，方便确认和撤销误操作。"
    >
      <div className="mt-6">
        <NumberSettingInput
          label="完成项保留时间（小时）"
          min={1}
          max={168}
          fallback={24}
          value={app.settings.todo.done_retention_hours}
          commit={(doneRetentionHours) => app.patch('todo', { done_retention_hours: doneRetentionHours })}
          hint="默认 24 小时。设为 0 则不保留已完成项。最长 168 小时（7天）。"
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
