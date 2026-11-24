import { useEffect, useState } from 'react';
import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import { NumberSettingInput } from './NumberSettingInput';
import type { AppSettingsController } from './useAppSettings';
import { useAppearance } from '../../../hooks/useAppearance';

interface EditorSectionProps {
  app: AppSettingsController;
}

/** Dark-palette accent, used only until the first computed-style read lands. */
const FALLBACK_ACCENT = '#86abdc';

/**
 * Resolve the active accent to a concrete hex so the colour swatch can show
 * what "follow theme" currently looks like. Reading the CSS variable keeps the
 * preview honest when the theme flips, without duplicating the palette here.
 */
function useThemeAccent(): string {
  const { resolvedTheme } = useAppearance();
  const [accent, setAccent] = useState(FALLBACK_ACCENT);

  useEffect(() => {
    // The appearance hook writes `data-theme` in its own effect, so read on the
    // next tick — otherwise the old palette is still in effect.
    const timer = window.setTimeout(() => {
      const value = getComputedStyle(document.documentElement)
        .getPropertyValue('--manager-accent')
        .trim();
      if (value) setAccent(value);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [resolvedTheme]);

  return accent;
}

/** 编辑器设置（普通设置域）：撤销步数与光标样式。 */
export function EditorSection({ app }: EditorSectionProps) {
  const themeAccent = useThemeAccent();
  const followTheme = app.settings.editor.cursor_color.trim() === '';
  const shownColor = followTheme ? themeAccent : app.settings.editor.cursor_color;

  return (
    <SettingsSectionCard
      kicker="Editor"
      title="编辑器"
      description="编辑器保留的撤销步数与光标可见性。保持轻量，不引入复杂编辑器内核。"
    >
      <div className="mt-6 grid gap-4 lg:grid-cols-2">
        <NumberSettingInput
          label="撤销步数"
          min={10}
          max={500}
          fallback={100}
          commitOn="change"
          value={app.settings.editor.undo_steps}
          commit={(undoSteps) => app.patch('editor', { undo_steps: undoSteps })}
          hint="建议设置 50-200 步。使用 Ctrl+Z 撤销，Ctrl+Shift+Z 重做。"
        />
        <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
          <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
            光标风格
          </label>
          <select
            value={app.settings.editor.cursor_style}
            onChange={(e) => app.patch('editor', {
              cursor_style: e.target.value as AppSettingsController['settings']['editor']['cursor_style'],
            })}
            className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
          >
            <option value="default">默认</option>
            <option value="accent">强调</option>
            <option value="focus">聚焦</option>
          </select>
          <label className="mt-4 block text-sm font-medium text-[color:var(--manager-ink-strong)]">
            光标颜色
          </label>
          <label className="mt-2 flex items-center gap-3 text-sm text-[color:var(--manager-ink-soft)]">
            <input
              type="checkbox"
              checked={followTheme}
              onChange={(e) => app.patch('editor', { cursor_color: e.target.checked ? '' : themeAccent })}
            />
            <span>跟随主题色（推荐）</span>
          </label>
          <div className="mt-2 flex items-center gap-3">
            <input
              type="color"
              aria-label="光标颜色"
              value={shownColor}
              disabled={followTheme}
              onChange={(e) => app.patch('editor', { cursor_color: e.target.value })}
              className="h-10 w-14 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-1 disabled:cursor-not-allowed disabled:opacity-50"
            />
            <input
              type="text"
              aria-label="光标颜色值"
              value={shownColor}
              disabled={followTheme}
              placeholder="留空表示跟随主题"
              onChange={(e) => app.patch('editor', { cursor_color: e.target.value })}
              className="flex-1 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)] disabled:cursor-not-allowed disabled:opacity-50"
            />
          </div>
          <p className="mt-2 text-xs text-[color:var(--manager-ink-subtle)]">
            {followTheme
              ? `跟随主题：当前为 ${themeAccent}，切换明暗主题时自动调整。`
              : '已使用固定颜色，不再随主题变化。'}
          </p>
          <label className="mt-4 flex items-center gap-3 text-sm text-[color:var(--manager-ink-soft)]">
            <input
              type="checkbox"
              checked={app.settings.editor.cursor_trail}
              onChange={(e) => app.patch('editor', { cursor_trail: e.target.checked })}
            />
            <span>启用简易聚焦光标特效</span>
          </label>
        </div>
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
