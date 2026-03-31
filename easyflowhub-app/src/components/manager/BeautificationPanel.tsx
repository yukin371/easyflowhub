/**
 * BeautificationPanel - 桌面美化设置面板
 * 包装 BeautificationPage 以适配 ManagerPage 布局
 */

import { useAppearance } from '../../hooks/useAppearance';
import { ThemeSection } from '../beautification/ThemeSection';
import { WindowSection } from '../beautification/WindowSection';
import { FontSection } from '../beautification/FontSection';
import { PreviewPanel } from '../beautification/PreviewPanel';

export function BeautificationPanel() {
  const {
    config,
    isLoaded,
    resolvedTheme,
    setTheme,
    setWindowAppearance,
    setFontSettings,
    reset,
  } = useAppearance();

  if (!isLoaded || !config) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-5 w-5 animate-spin rounded-full border-2 border-gray-600 border-t-blue-500" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="border-b border-[color:var(--manager-border)] px-6 py-4">
        <h1 className="text-lg font-semibold text-[color:var(--manager-ink-strong)]">桌面美化</h1>
        <p className="mt-1 text-sm text-[color:var(--manager-ink-soft)]">
          自定义主题、窗口外观和字体设置
        </p>
      </header>

      {/* Main content */}
      <main className="flex-1 overflow-auto p-6">
        <div className="mx-auto max-w-2xl space-y-8">
          <ThemeSection theme={config.theme} onThemeChange={setTheme} />
          <WindowSection window={config.window} onChange={setWindowAppearance} />
          <FontSection font={config.font} onChange={setFontSettings} />
          <PreviewPanel config={config} resolvedTheme={resolvedTheme} />
        </div>
      </main>

      {/* Action bar */}
      <footer className="flex items-center justify-end gap-3 border-t border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-6 py-4">
        <button
          onClick={reset}
          className="rounded-lg border border-[color:var(--manager-border)] bg-white/50 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition-colors hover:bg-white/70 hover:text-[color:var(--manager-ink)]"
        >
          恢复默认
        </button>
      </footer>
    </div>
  );
}
