import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { openDebugLogDir } from '../../../lib/tauri/logging';
import type { AppSettingsController } from './useAppSettings';
import { useDebugLogs } from './useDebugLogs';

interface DeveloperSectionProps {
  app: AppSettingsController;
}

/** 开发者设置：调试开关（普通设置域）+ 本机日志查看。 */
export function DeveloperSection({ app }: DeveloperSectionProps) {
  const logs = useDebugLogs();

  return (
    <SettingsSectionCard
      kicker="Developer"
      title="开发者模式"
      description="用于排查启动、服务连接与脚本执行问题。日志只保存在本机，不会上传；调试模式只影响日志详细程度与落盘，不改变任何安全边界。"
    >
      <label className="flex items-start gap-3 rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-3">
        <input
          type="checkbox"
          aria-label="开启调试日志"
          checked={app.settings.developer.debug_mode}
          onChange={(event) => app.patch('developer', { debug_mode: event.target.checked })}
          className="mt-1 h-4 w-4"
        />
        <span className="min-w-0">
          <span className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
            开启调试日志
          </span>
          <span className="mt-1 block text-xs leading-6 text-[color:var(--manager-ink-muted)]">
            开启后日志会追加写入应用数据目录下的 logs/app.log（超过 2 MB 自动轮转）。
          </span>
        </span>
      </label>

      <div className="mt-4 flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={() => void app.save()}
          disabled={app.loadState !== 'ready'}
          className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-[color:var(--manager-bg)] transition hover:bg-[color:var(--manager-accent-strong)]"
        >
          保存开发者设置
        </button>
        <span className="text-xs text-[color:var(--manager-ink-subtle)]">
          {app.saveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
        </span>
        <button
          type="button"
          onClick={() => void logs.refresh()}
          disabled={logs.loading}
          className="rounded-full border border-[color:var(--manager-border)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] disabled:opacity-60"
        >
          {logs.loading ? '读取中…' : '刷新日志'}
        </button>
        <button
          type="button"
          onClick={() => void logs.clear()}
          className="rounded-full border border-[color:var(--manager-border)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-danger)] hover:text-[color:var(--manager-danger)]"
        >
          清空
        </button>
        <button
          type="button"
          onClick={() => void openDebugLogDir().catch((err) => app.setError(String(err)))}
          className="rounded-full border border-[color:var(--manager-border)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
        >
          打开日志目录
        </button>
      </div>

      <pre
        aria-label="调试日志"
        className="mt-4 max-h-[280px] overflow-auto whitespace-pre-wrap break-words rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-3 font-mono text-[11px] leading-6 text-[color:var(--manager-ink-soft)]"
      >
        {logs.lines.length > 0 ? logs.lines.join('\n') : logs.error ? `无法读取日志：${logs.error}` : '暂无日志'}
      </pre>
      {app.error && <p role="alert" className="mt-3 text-sm text-[color:var(--manager-danger)]">{app.error}</p>}
    </SettingsSectionCard>
  );
}
