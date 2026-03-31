/**
 * ScriptsPanel - 脚本管理面板
 * 左侧分类树 + 右侧脚本列表 + 详情抽屉
 */

import { useEffect } from 'react';
import { ScriptStoreProvider, useScriptStore } from './scriptStore';
import { ScriptCategoryTree } from './ScriptCategoryTree';
import { ScriptListView } from './ScriptListView';
import { ScriptDrawer } from './ScriptDrawer';
import { checkServerHealth } from '../../../lib/api/scriptmgr';

function ScriptsPanelContent() {
  const {
    scripts,
    categories,
    selectedCategory,
    searchQuery,
    loading,
    error,
    drawerOpen,
    filteredScripts,
    fetchScripts,
    setSelectedCategory,
    openDrawer,
    closeDrawer,
    setSearchQuery,
    clearError,
  } = useScriptStore();

  // Initial load
  useEffect(() => {
    fetchScripts();
  }, [fetchScripts]);

  // Check server health on mount
  useEffect(() => {
    checkServerHealth().then(({ ok, message }) => {
      if (!ok) {
        console.warn('ScriptMgr server health check:', message);
      }
    });
  }, []);

  return (
    <section className="flex h-full gap-3 px-1 py-1 sm:px-2 sm:py-2">
      {/* Left: Category Tree */}
      <aside className="hidden w-52 shrink-0 flex-col gap-2 rounded-[20px] bg-[rgba(255,251,245,0.44)] px-3 py-3 lg:flex">
        <div className="space-y-1 px-1 py-1">
          <p className="manager-kicker">ScriptMgr</p>
          <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[22px] leading-[1.1] text-[color:var(--manager-ink-strong)]">
            脚本库
          </h2>
        </div>

        <ScriptCategoryTree
          categories={categories}
          selectedCategory={selectedCategory}
          onSelect={setSelectedCategory}
        />

        <div className="mt-auto px-1 pb-1">
          <p className="text-[11px] leading-5 tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
            {scripts.length} 个脚本
          </p>
        </div>
      </aside>

      {/* Right: Script List */}
      <main className="flex min-w-0 flex-1 flex-col gap-2">
        {/* Header */}
        <header className="rounded-[18px] bg-[rgba(255,251,245,0.52)] px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-full border border-[color:var(--manager-border)] bg-[rgba(255,251,245,0.56)] text-xs tracking-[0.24em] text-[color:var(--manager-ink-soft)]">
              检
            </span>
            <input
              type="text"
              placeholder="搜索脚本名称、描述或标签..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full border-none bg-transparent text-sm text-[color:var(--manager-ink)] outline-none placeholder:text-[color:var(--manager-ink-subtle)]"
            />
            <span className="shrink-0 text-xs uppercase tracking-[0.22em] text-[color:var(--manager-ink-subtle)]">
              {filteredScripts.length} / {scripts.length}
            </span>
          </div>
        </header>

        {/* Error Banner */}
        {error && (
          <div className="flex items-center justify-between rounded-[14px] border border-red-200 bg-red-50 px-4 py-2">
            <span className="text-sm text-red-700">{error}</span>
            <div className="flex gap-2">
              <button
                onClick={fetchScripts}
                className="text-sm font-medium text-red-700 hover:text-red-900"
              >
                重试
              </button>
              <button onClick={clearError} className="text-sm text-red-500 hover:text-red-700">
                关闭
              </button>
            </div>
          </div>
        )}

        {/* Script List */}
        <div className="min-h-0 flex-1 overflow-auto rounded-[16px] bg-[rgba(255,249,242,0.24)] px-2 py-2">
          {loading ? (
            <div className="flex h-full items-center justify-center">
              <div className="h-7 w-7 rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)] animate-spin" />
            </div>
          ) : filteredScripts.length === 0 ? (
            <div className="flex h-full min-h-[320px] items-center justify-center rounded-[20px] border border-dashed border-[color:var(--manager-border)] bg-white/35 text-center">
              <div>
                <p className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-3xl text-[color:var(--manager-ink-strong)]">
                  暂无脚本
                </p>
                <p className="mt-3 text-sm text-[color:var(--manager-ink-soft)]">
                  {searchQuery ? '调整搜索条件' : 'ScriptMgr 服务可能未启动'}
                </p>
              </div>
            </div>
          ) : (
            <ScriptListView scripts={filteredScripts} onSelect={openDrawer} />
          )}
        </div>
      </main>

      {/* Drawer */}
      {drawerOpen && <ScriptDrawer onClose={closeDrawer} />}
    </section>
  );
}

export function ScriptsPanel() {
  return (
    <ScriptStoreProvider>
      <ScriptsPanelContent />
    </ScriptStoreProvider>
  );
}
