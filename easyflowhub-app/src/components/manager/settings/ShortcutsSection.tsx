import type { LauncherActivation } from '../../../types/launcherSettings';
import { SHORTCUT_FIELDS } from './useShortcutSettings';
import type { ShortcutsSectionController } from './useShortcutsSettings';

interface ShortcutsSectionProps {
  domain: ShortcutsSectionController;
}

/**
 * 快捷键区：快捷键配置 + Launcher 触发方式 + Snow Shot 路径。
 * 保存 / 恢复默认两个按钮是本区唯一的提交入口，编排逻辑在
 * useShortcutsSettings（每域一个提交入口，顺序与失败语义保持不变）。
 */
export function ShortcutsSection({ domain }: ShortcutsSectionProps) {
  const { launcher, shortcuts, launcherShortcutConflict, save, reset } = domain;

  return (
    <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-5">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <p className="manager-kicker">Shortcuts</p>
          <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">快捷键配置</h3>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => void save()}
            className="rounded-full bg-[color:var(--manager-accent)] px-4 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
          >
            保存配置
          </button>
          <button
            onClick={() => void reset()}
            className="rounded-full border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
          >
            恢复默认
          </button>
        </div>
      </div>
      <p className="mb-4 text-xs text-[color:var(--manager-ink-subtle)]">
        {shortcuts.saveState === 'saved' ? '✓ 配置已保存' : '○ 存在未保存修改'} · {shortcuts.changedCount} 项已自定义
      </p>

      <div className="mb-5 rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-accent-soft)]/45 p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">Launcher 唤起方式</p>
            <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">
              选择键盘、鼠标中键，或同时启用两种入口。排除软件在前台运行时不会响应全局唤起。
            </p>
          </div>
          <span className="text-xs text-[color:var(--manager-ink-subtle)]">
            {launcher.saveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
          </span>
        </div>
        {launcher.error ? (
          <p className="mt-3 rounded-[12px] border border-[color:var(--manager-danger)]/40 bg-[color:var(--manager-danger)]/12 px-3 py-2 text-xs leading-5 text-[color:var(--manager-danger)]" role="alert">
            {launcher.error}
          </p>
        ) : null}

        <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(0,180px)_minmax(0,1fr)] lg:items-center">
          <label className="text-sm text-[color:var(--manager-ink-soft)]" htmlFor="launcher-activation">
            触发入口
          </label>
          <select
            id="launcher-activation"
            aria-label="Launcher 唤起方式"
            value={launcher.settings.activation}
            onChange={(event) => launcher.change('activation', event.target.value as LauncherActivation)}
            className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
          >
            <option value="keyboardOrMiddleMouse">键盘快捷键 + 鼠标中键</option>
            <option value="keyboard">仅键盘快捷键</option>
            <option value="middleMouse">仅鼠标中键</option>
          </select>
        </div>

        <div className="mt-3 grid gap-3 lg:grid-cols-[minmax(0,180px)_minmax(0,1fr)] lg:items-center">
          <div>
            <p className="text-sm text-[color:var(--manager-ink-soft)]">键盘快捷键</p>
            <p className="mt-1 text-xs leading-5 text-[color:var(--manager-ink-subtle)]">系统级快捷键。若同一组合也被其他软件（如 QQ 截图）监听，两个程序可能同时响应；请重新录入一个不同组合并保存，或在另一软件中解绑。</p>
          </div>
          <div>
            <button
              type="button"
              disabled={launcher.settings.activation === 'middleMouse'}
              onClick={launcher.toggleRecording}
              onKeyDown={launcher.handleRecordKeyDown}
              className={`w-full rounded-[16px] border px-4 py-3 text-left text-sm outline-none transition disabled:cursor-not-allowed disabled:opacity-50 ${
                launcher.recording
                  ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-panel-strong)] text-[color:var(--manager-ink-strong)]'
                  : 'border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] hover:border-[color:var(--manager-accent)]'
              }`}
            >
              <span className="block font-medium">{launcher.recording ? '按下快捷键组合' : launcher.settings.keyboardShortcut}</span>
              <span className="mt-1 block text-xs text-[color:var(--manager-ink-subtle)]">
                {launcher.recording ? '按 Esc 取消录制' : '点击后直接按键录入'}
              </span>
            </button>
            {launcherShortcutConflict ? (
              <p className="mt-2 text-xs text-[color:var(--manager-warning)]" role="alert">
                与“{launcherShortcutConflict}”重复，请更换组合。
              </p>
            ) : null}
          </div>
        </div>

        <div className="mt-4 border-t border-[color:var(--manager-border)]/70 pt-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">排除软件</p>
              <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">填写 Windows 进程名，例如 `Code.exe`、`idea64.exe`。专注软件在前台时不会唤起 Launcher。</p>
            </div>
            <span className="text-xs text-[color:var(--manager-ink-subtle)]">{launcher.settings.excludedApps.length} 个</span>
          </div>
          <div className="mt-3 flex gap-2">
            <input
              value={launcher.excludedAppInput}
              onChange={(event) => launcher.setExcludedAppInput(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  launcher.addExcludedApp();
                }
              }}
              aria-label="添加排除软件"
              placeholder="例如 Code.exe"
              className="min-w-0 flex-1 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
            />
            <button
              type="button"
              onClick={launcher.addExcludedApp}
              className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
            >
              添加
            </button>
          </div>
          {launcher.settings.excludedApps.length ? (
            <ul className="mt-3 flex flex-wrap gap-2" aria-label="已排除软件">
              {launcher.settings.excludedApps.map((app) => (
                <li key={app.id} className="inline-flex items-center gap-2 rounded-full border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-1.5 text-xs text-[color:var(--manager-ink-soft)]">
                  <span>{app.label || app.executable}</span>
                  <button
                    type="button"
                    onClick={() => launcher.removeExcludedApp(app.id)}
                    aria-label={`移除排除软件 ${app.executable}`}
                    className="text-[color:var(--manager-ink-subtle)] hover:text-[color:var(--manager-ink-strong)]"
                  >
                    ×
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mt-3 text-xs text-[color:var(--manager-ink-subtle)]">尚未设置排除软件。</p>
          )}
        </div>

        <div className="mt-4 border-t border-[color:var(--manager-border)]/70 pt-4">
          <div>
            <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">Snow Shot 可执行文件（可选）</p>
            <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">
              便携版或自定义安装位置可填写完整路径；留空时自动检查常见目录和 PATH。
            </p>
          </div>
          <div className="mt-3 flex gap-2">
            <input
              value={launcher.snowShotPath}
              onChange={(event) => launcher.updateSnowShotPath(event.target.value)}
              aria-label="Snow Shot 可执行文件路径"
              placeholder="例如 C:\\Tools\\Snow Shot.exe"
              className="min-w-0 flex-1 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
            />
            <button
              type="button"
              onClick={launcher.saveSnowShotPath}
              className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
            >
              {launcher.snowShotPathSaveState === 'saved' ? '已保存' : '保存路径'}
            </button>
          </div>
        </div>
      </div>

      <div className="space-y-3">
        {SHORTCUT_FIELDS.map(({ key, label, description }) => (
          <div
            key={key}
            className="grid gap-3 rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3 lg:grid-cols-[180px_minmax(0,1fr)] lg:items-center"
          >
            <div>
              <p className="text-sm tracking-[0.16em] text-[color:var(--manager-ink-strong)]">
                {label}
              </p>
              <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">
                {description}
              </p>
            </div>
            <button
              onClick={() => shortcuts.toggleRecording(key)}
              onKeyDown={(event) => shortcuts.handleRecordKeyDown(key, event)}
              className={`rounded-[16px] border px-4 py-3 text-left text-sm outline-none transition ${
                shortcuts.recordingKey === key
                  ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-[color:var(--manager-ink)] hover:border-[color:var(--manager-accent)]'
              }`}
            >
              <span className="block font-medium">{shortcuts.recordingKey === key ? '按下快捷键组合' : shortcuts.config[key]}</span>
              <span className="mt-1 block text-xs text-[color:var(--manager-ink-subtle)]">
                {shortcuts.recordingKey === key ? '按 Esc 取消录制' : '点击后直接按键录入'}
              </span>
            </button>
          </div>
        ))}
      </div>
    </section>
  );
}
