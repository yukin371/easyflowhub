import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import type { AppSettingsController } from './useAppSettings';

interface TranslationSectionProps {
  app: AppSettingsController;
}

/** 翻译设置：普通设置聚合的一个域，提交复用 app.save()。 */
export function TranslationSection({ app }: TranslationSectionProps) {
  return (
    <SettingsSectionCard
      kicker="Translation"
      title="翻译"
      description="复用本机 Relay 的服务和密钥，从快捷窗口翻译剪贴板文本。"
    >
      <div className="grid gap-4 sm:grid-cols-2">
        <div>
          <label htmlFor="translation-target-language" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
            目标语言
          </label>
          <select
            id="translation-target-language"
            value={app.settings.translation.target_language}
            onChange={(event) => app.patch('translation', {
              target_language: event.target.value as AppSettingsController['settings']['translation']['target_language'],
            })}
            className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          >
            <option value="zh-CN">简体中文</option>
            <option value="en">英语</option>
            <option value="ja">日语</option>
            <option value="ko">韩语</option>
          </select>
        </div>
        <div>
          <label htmlFor="translation-model" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
            翻译模型
          </label>
          <input
            id="translation-model"
            value={app.settings.translation.model}
            onChange={(event) => app.patch('translation', { model: event.target.value })}
            placeholder="填写 Relay 中可用的模型名称"
            autoComplete="off"
            className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          />
        </div>
      </div>
      <p className="mt-3 text-xs leading-6 text-[color:var(--manager-ink-muted)]">
        使用前请启动 Relay，并填写已配置的模型名称。
      </p>
      <div className="mt-4 flex items-center gap-4">
        <button
          type="button"
          onClick={() => void app.save()}
          disabled={app.loadState !== 'ready'}
          className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-[color:var(--manager-bg)] transition hover:bg-[color:var(--manager-accent-strong)]"
        >
          保存翻译设置
        </button>
        <span className="text-xs text-[color:var(--manager-ink-subtle)]">
          {app.saveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
        </span>
      </div>
      {app.error && <p role="alert" className="mt-3 text-sm text-[color:var(--manager-danger)]">{app.error}</p>}
    </SettingsSectionCard>
  );
}
