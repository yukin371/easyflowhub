/**
 * ScriptsPanel - 脚本管理面板
 * 左侧分类树 + 右侧脚本列表 + 详情抽屉 + 源码/运行工作台 + 运行记录
 */

import { useCallback, useEffect, useState } from 'react';
import { ScriptStoreProvider, useScriptStore } from './scriptStore';
import { ScriptCategoryTree } from './ScriptCategoryTree';
import { ScriptListView } from './ScriptListView';
import { ScriptDrawer } from './ScriptDrawer';
import { ScriptCreateDialog, ScriptImportDialog, ScriptRootsDialog, ScriptWorkbenchDialog } from './ScriptFileDialogs';
import { TasksPanel } from '../tasks/TasksPanel';
import { checkServerHealth, rootsApi } from '../../../lib/api/scriptmgr';
import type { RootsDetail, ScriptDetail, ScriptSummary } from '../../../types/scriptmgr';

type ScriptsView = 'scripts' | 'tasks';
type DialogState = null | 'create' | 'import' | 'roots' | { workbench: ScriptSummary };

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

  const [view, setView] = useState<ScriptsView>('scripts');
  const [roots, setRoots] = useState<RootsDetail | null>(null);
  const [dialog, setDialog] = useState<DialogState>(null);

  const loadRoots = useCallback(async () => {
    try {
      setRoots(await rootsApi.list());
    } catch {
      setRoots(null);
    }
  }, []);

  // Initial load
  useEffect(() => {
    void fetchScripts();
    void loadRoots();
  }, [fetchScripts, loadRoots]);

  // Check server health on mount
  useEffect(() => {
    checkServerHealth().then(({ ok, message }) => {
      if (!ok) {
        console.warn('ScriptMgr server health check:', message);
      }
    });
  }, []);

  const refreshAll = useCallback(async () => {
    await Promise.all([fetchScripts(), loadRoots()]);
  }, [fetchScripts, loadRoots]);

  const openWorkbench = (script: ScriptDetail | ScriptSummary) => {
    setDialog({ workbench: script });
  };

  return (
    <section className="flex h-full gap-3 px-1 py-1 sm:px-2 sm:py-2">
      {/* Left: Category Tree */}
      <aside className="hidden w-52 shrink-0 flex-col gap-2 rounded-[20px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-3 py-3 lg:flex">
        <div className="space-y-1 px-1 py-1">
          <p className="manager-kicker">ScriptMgr</p>
          <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[22px] leading-[1.1] text-[color:var(--manager-ink-strong)]">
            脚本库
          </h2>
        </div>

        {view === 'scripts' ? (
          <ScriptCategoryTree
            categories={categories}
            selectedCategory={selectedCategory}
            onSelect={setSelectedCategory}
          />
        ) : (
          <p className="px-1 text-xs leading-5 text-[color:var(--manager-ink-subtle)]">
            运行记录汇总所有脚本的执行历史与输出。
          </p>
        )}

        <div className="mt-auto space-y-1 px-1 pb-1">
          <p className="text-[11px] leading-5 tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
            {scripts.length} 个脚本
          </p>
          <p className="text-[11px] leading-5 tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
            {roots?.roots.length ?? 0} 个存储位置
          </p>
        </div>
      </aside>

      {/* Right: Script List */}
      <main className="flex min-w-0 flex-1 flex-col gap-2">
        {/* Header */}
        <header className="space-y-2 rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2" role="tablist" aria-label="脚本视图">
              {(['scripts', 'tasks'] as const).map((value) => (
                <button
                  key={value}
                  type="button"
                  role="tab"
                  aria-selected={view === value}
                  onClick={() => setView(value)}
                  className={`rounded-[8px] border px-3 py-1.5 text-sm transition ${
                    view === value
                      ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                      : 'border-transparent text-[color:var(--manager-ink-muted)] hover:bg-[color:var(--manager-panel-strong)]'
                  }`}
                >
                  {value === 'scripts' ? '脚本' : '运行记录'}
                </button>
              ))}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                onClick={() => setDialog('create')}
                className="rounded-[10px] bg-[color:var(--manager-accent)] px-3 py-1.5 text-sm text-white transition hover:brightness-105"
              >
                新建
              </button>
              <button
                type="button"
                onClick={() => setDialog('import')}
                className="rounded-[10px] border border-[color:var(--manager-border)] px-3 py-1.5 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
              >
                导入
              </button>
              <button
                type="button"
                onClick={() => setDialog('roots')}
                className="rounded-[10px] border border-[color:var(--manager-border)] px-3 py-1.5 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
              >
                存储位置
              </button>
            </div>
          </div>

          {view === 'scripts' ? (
            <div className="flex items-center gap-3">
              <span className="flex h-9 w-9 items-center justify-center rounded-full border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-xs tracking-[0.24em] text-[color:var(--manager-ink-soft)]">
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
          ) : null}
        </header>

        {/* Error Banner */}
        {error && (
          <div className="manager-alert-error flex items-center justify-between rounded-[14px] border px-4 py-2">
            <span className="text-sm">{error}</span>
            <div className="flex gap-2">
              <button onClick={() => void refreshAll()} className="text-sm font-medium transition hover:brightness-125">
                重试
              </button>
              <button onClick={clearError} className="text-sm opacity-80 transition hover:opacity-100">
                关闭
              </button>
            </div>
          </div>
        )}

        {/* Body */}
        {view === 'tasks' ? (
          <div className="min-h-0 flex-1 overflow-hidden rounded-[16px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)]">
            <TasksPanel />
          </div>
        ) : (
          <div className="min-h-0 flex-1 overflow-auto rounded-[16px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-2 py-2">
            {loading ? (
              <div className="flex h-full items-center justify-center">
                <div className="h-7 w-7 rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)] animate-spin" />
              </div>
            ) : filteredScripts.length === 0 ? (
              <div className="flex h-full min-h-[320px] items-center justify-center rounded-[20px] border border-dashed border-[color:var(--manager-border)] bg-transparent text-center">
                <div>
                  <p className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-3xl text-[color:var(--manager-ink-strong)]">
                    暂无脚本
                  </p>
                  <p className="mt-3 text-sm text-[color:var(--manager-ink-soft)]">
                    {searchQuery
                      ? '调整搜索条件'
                      : (roots?.roots.length ?? 0) === 0
                        ? '还没有脚本目录，点击「存储位置」添加'
                        : '当前存储位置下没有脚本，可用「新建」或「导入」添加'}
                  </p>
                </div>
              </div>
            ) : (
              <ScriptListView scripts={filteredScripts} onSelect={openDrawer} />
            )}
          </div>
        )}
      </main>

      {/* Drawer */}
      {drawerOpen && view === 'scripts' && (
        <ScriptDrawer onClose={closeDrawer} onOpenWorkbench={openWorkbench} />
      )}

      {/* Dialogs */}
      {dialog === 'create' && (
        <ScriptCreateDialog
          roots={roots}
          onClose={() => setDialog(null)}
          onCreated={async (script) => {
            await refreshAll();
            setDialog({ workbench: script });
          }}
        />
      )}
      {dialog === 'import' && (
        <ScriptImportDialog roots={roots} onClose={() => setDialog(null)} onImported={() => void refreshAll()} />
      )}
      {dialog === 'roots' && (
        <ScriptRootsDialog
          onClose={() => setDialog(null)}
          onChanged={(detail) => {
            setRoots(detail);
            void fetchScripts();
          }}
        />
      )}
      {dialog && typeof dialog === 'object' && 'workbench' in dialog && (
        <ScriptWorkbenchDialog
          script={dialog.workbench}
          onClose={() => setDialog(null)}
          onChanged={() => void refreshAll()}
        />
      )}
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
