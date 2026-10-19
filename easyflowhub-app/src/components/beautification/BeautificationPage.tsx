// src/components/beautification/BeautificationPage.tsx
import { useAppearance } from '../../hooks/useAppearance';
import { ThemeSection } from './ThemeSection';
import { WindowSection } from './WindowSection';
import { FontSection } from './FontSection';
import { PreviewPanel } from './PreviewPanel';

export function BeautificationPage() {
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
      <div className="flex h-full items-center justify-center bg-[color:var(--manager-panel)]">
        <div className="h-5 w-5 animate-spin rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)]" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-[color:var(--manager-panel)] text-[color:var(--manager-ink)]">
      {/* Header */}
      <header className="border-b border-[color:var(--manager-border)] p-4">
        <h1 className="text-lg font-semibold text-[color:var(--manager-ink-strong)]">桌面美化</h1>
      </header>

      {/* Main content */}
      <main className="flex-1 overflow-auto p-6 space-y-8">
        <ThemeSection theme={config.theme} onThemeChange={setTheme} />
        <WindowSection window={config.window} onChange={setWindowAppearance} />
        <FontSection font={config.font} onChange={setFontSettings} />
        <PreviewPanel config={config} resolvedTheme={resolvedTheme} />
      </main>

      {/* Action bar */}
      <footer className="flex items-center justify-end gap-3 border-t border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-4">
        <button
          onClick={reset}
          className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition-colors hover:bg-[color:var(--manager-glass-highlight)] hover:text-[color:var(--manager-ink-strong)]"
        >
          恢复默认
        </button>
      </footer>
    </div>
  );
}
