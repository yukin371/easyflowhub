import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { NumberSettingInput } from './NumberSettingInput';
import type { AppSettingsController } from './useAppSettings';

interface QuickNoteSectionProps {
  app: AppSettingsController;
}

/** 快速笔记窗口默认尺寸（普通设置域）。 */
export function QuickNoteSection({ app }: QuickNoteSectionProps) {
  return (
    <SettingsSectionCard
      kicker="Quick Note"
      title="快速笔记窗口"
      description="新建快速笔记窗口的默认尺寸。"
    >
      <div className="mt-6 grid gap-4 sm:grid-cols-2">
        <NumberSettingInput
          label="宽度 (px)"
          min={200}
          max={800}
          fallback={400}
          value={app.settings.quick_note.width}
          commit={(width) => app.patch('quick_note', { width })}
        />
        <NumberSettingInput
          label="高度 (px)"
          min={150}
          max={600}
          fallback={300}
          value={app.settings.quick_note.height}
          commit={(height) => app.patch('quick_note', { height })}
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
