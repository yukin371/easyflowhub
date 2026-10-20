import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import type { AppSettingsController } from './useAppSettings';

interface TranscriptionSectionProps {
  app: AppSettingsController;
}

/** 转写设置：普通设置聚合的一个域，提交复用 app.save()。 */
export function TranscriptionSection({ app }: TranscriptionSectionProps) {
  return (
    <SettingsSectionCard
      kicker="Transcription"
      title="转写"
      description="录音结束后将音频发送到 Relay 配置的上游 Provider 转写为文字。音频不会自动上传，每次转写都需要在录音或收集箱中确认。"
    >
      <div className="grid gap-4 sm:grid-cols-2">
        <div>
          <label htmlFor="transcription-language" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
            音频语言
          </label>
          <select
            id="transcription-language"
            value={app.settings.transcription.language}
            onChange={(event) => app.patch('transcription', { language: event.target.value })}
            className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          >
            <option value="">自动识别</option>
            <option value="zh">中文</option>
            <option value="en">英语</option>
            <option value="ja">日语</option>
            <option value="ko">韩语</option>
          </select>
        </div>
        <div>
          <label htmlFor="transcription-model" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
            转写模型
          </label>
          <input
            id="transcription-model"
            value={app.settings.transcription.model}
            onChange={(event) => app.patch('transcription', { model: event.target.value })}
            placeholder="填写 Relay 中可用的语音转文字模型名称"
            autoComplete="off"
            className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          />
        </div>
      </div>
      <p className="mt-3 text-xs leading-6 text-[color:var(--manager-ink-muted)]">
        使用前请启动 Relay，并填写已配置的模型名称。单个录音上限 25 MB（约 4.5 分钟），超出时请分段录制。密钥由 Relay 管理，此处不保存任何凭据。
      </p>
      <div className="mt-4 flex items-center gap-4">
        <button
          type="button"
          onClick={() => void app.save()}
          disabled={app.loadState !== 'ready'}
          className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-[color:var(--manager-bg)] transition hover:bg-[color:var(--manager-accent-strong)]"
        >
          保存转写设置
        </button>
        <span className="text-xs text-[color:var(--manager-ink-subtle)]">
          {app.saveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
        </span>
      </div>
      {app.error && <p role="alert" className="mt-3 text-sm text-[color:var(--manager-danger)]">{app.error}</p>}
    </SettingsSectionCard>
  );
}
