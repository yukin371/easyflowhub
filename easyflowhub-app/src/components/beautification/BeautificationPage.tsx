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
      <div className="h-full flex items-center justify-center bg-gray-900/80">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-blue-500 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col bg-gray-900/80">
      {/* Header */}
      <header className="p-4 border-b border-white/10">
        <h1 className="text-lg font-semibold text-gray-200">桌面美化</h1>
      </header>

      {/* Main content */}
      <main className="flex-1 overflow-auto p-6 space-y-8">
        <ThemeSection theme={config.theme} onThemeChange={setTheme} />
        <WindowSection window={config.window} onChange={setWindowAppearance} />
        <FontSection font={config.font} onChange={setFontSettings} />
        <PreviewPanel config={config} resolvedTheme={resolvedTheme} />
      </main>

      {/* Action bar */}
      <footer className="p-4 border-t border-white/10 bg-black/30 flex items-center justify-end gap-3">
        <button
          onClick={reset}
          className="px-4 py-2 bg-white/10 text-gray-300 text-sm rounded hover:bg-white/15 transition-colors"
        >
          恢复默认
        </button>
      </footer>
    </div>
  );
}
