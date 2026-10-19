import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { copyTextToClipboard } from '../../../lib/tauri/clipboard';
import './FilesPanel.css';
import {
  clearFileIndex,
  copyFileSearchPath,
  copyIndexedFile,
  deleteToRecycleBin,
  getFileSearchStatus,
  listRecentFiles,
  moveIndexedFile,
  openFile,
  previewFile,
  refreshFileIndex,
  renameIndexedFile,
  revealFileInExplorer,
  searchFiles,
  setFileSearchPaused,
  setFileSearchRoot,
} from '../../../lib/tauri/fileSearch';
import type { FileEntry, FilePreview, FileSearchResponse, FileSearchStatus } from '../../../types/fileSearch';

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  if (error && typeof error === 'object' && 'message' in error) return String(error.message);
  return '文件操作失败，请检查目录权限和文件状态后重试。';
}

function formatSize(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  if (size < 1024 * 1024 * 1024) return `${(size / (1024 * 1024)).toFixed(1)} MB`;
  return `${(size / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatTime(value: string | null): string {
  if (!value) return '未知时间';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'short', timeStyle: 'short' }).format(date);
}

function fileType(file: FileEntry): string {
  if (file.isDir) return '文件夹';
  if (!file.extension) return '文件';
  if (/^(png|jpe?g|gif|webp|bmp|ico|svg)$/i.test(file.extension)) return '图片';
  if (/^(txt|md|markdown|log|json|csv|xml|ya?ml|toml|ini|ts|tsx|js|jsx|css|html|rs|go|py)$/i.test(file.extension)) return '文本';
  return file.extension.toLocaleUpperCase();
}

function statusLabel(status: FileSearchStatus | null): string {
  if (!status) return '正在读取索引';
  if (!status.root) return '未选择目录';
  if (status.paused) return `索引已暂停 · ${status.count.toLocaleString()} 项`;
  return `${status.count.toLocaleString()} 个文件已索引`;
}

function operationGeneration(status: FileSearchStatus | null, response: FileSearchResponse | null): number {
  return response?.generation ?? status?.generation ?? 0;
}

export function FilesPanel() {
  const [status, setStatus] = useState<FileSearchStatus | null>(null);
  const [query, setQuery] = useState('');
  const [searchResponse, setSearchResponse] = useState<FileSearchResponse | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [previewError, setPreviewError] = useState<string>();
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [isSearching, setIsSearching] = useState(false);
  const [globalBusy, setGlobalBusy] = useState(false);
  const [busyPaths, setBusyPaths] = useState<Set<string>>(() => new Set());
  const [renamingPath, setRenamingPath] = useState<string | null>(null);
  const [newName, setNewName] = useState('');
  const busyPathLock = useRef(new Set<string>());
  const globalBusyLock = useRef(false);

  const results = searchResponse?.items ?? [];
  const selectedFile = useMemo(
    () => results.find((file) => file.path === selectedPath) ?? null,
    [results, selectedPath]
  );
  const generation = operationGeneration(status, searchResponse);

  const loadStatus = useCallback(async () => {
    const next = await getFileSearchStatus();
    setStatus(next);
    return next;
  }, []);

  const runSearch = useCallback(async (value: string) => {
    if (!status?.root) {
      setSearchResponse(null);
      setSelectedPath(null);
      setPreview(null);
      return;
    }

    setIsSearching(true);
    try {
      const response = value.trim()
        ? await searchFiles({ query: value.trim(), sort: 'relevance', limit: 100 })
        : await listRecentFiles(50);
      setSearchResponse(response);
      setSelectedPath((current) => response.items.some((file) => file.path === current)
        ? current
        : response.items[0]?.path ?? null);
      setStatus((current) => current ? { ...current, generation: response.generation } : current);
      setError(undefined);
    } catch (caught) {
      setError(errorMessage(caught));
      setSearchResponse(null);
      setSelectedPath(null);
    } finally {
      setIsSearching(false);
    }
  }, [status?.root]);

  const refreshStatus = useCallback(async () => {
    setIsLoading(true);
    try {
      await loadStatus();
      setError(undefined);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setIsLoading(false);
    }
  }, [loadStatus]);

  useEffect(() => { void refreshStatus(); }, [refreshStatus]);

  useEffect(() => {
    if (!status?.root) {
      setSearchResponse(null);
      setSelectedPath(null);
      setPreview(null);
      return;
    }
    const timer = window.setTimeout(() => { void runSearch(query); }, query.trim() ? 220 : 0);
    return () => window.clearTimeout(timer);
  }, [query, status?.root, runSearch]);

  useEffect(() => {
    let cancelled = false;
    setPreview(null);
    setPreviewError(undefined);
    if (!selectedPath || !status?.root) return;
    void previewFile(selectedPath, generation).then((next) => {
      if (!cancelled) setPreview(next);
    }).catch((caught: unknown) => {
      if (!cancelled) setPreviewError(errorMessage(caught));
    });
    return () => { cancelled = true; };
  }, [generation, selectedPath, status?.root]);

  const withBusyPath = useCallback(async (path: string, action: () => Promise<void>) => {
    if (busyPathLock.current.has(path)) return;
    busyPathLock.current.add(path);
    setBusyPaths((current) => new Set(current).add(path));
    setError(undefined);
    setNotice(undefined);
    try {
      await action();
    } catch (caught) {
      const message = errorMessage(caught);
      setError(message);
      // A native file mutation can succeed while its post-operation scan
      // fails, or the result can become stale while the operation is in
      // flight. In both cases Rust advances generation and clears stale
      // entries; reload the status/results before allowing another action.
      if (message.includes('索引')) {
        try {
          const next = await loadStatus();
          setStatus(next);
          if (next.root) {
            await runSearch(query);
          }
          setError(message);
        } catch {
          // Keep the original, more actionable operation error visible.
        }
      }
    } finally {
      busyPathLock.current.delete(path);
      setBusyPaths((current) => {
        const next = new Set(current);
        next.delete(path);
        return next;
      });
    }
  }, [busyPaths, loadStatus, query, runSearch]);

  const withGlobalBusy = useCallback(async (action: () => Promise<void>) => {
    if (globalBusyLock.current) return;
    globalBusyLock.current = true;
    setGlobalBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      await action();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      globalBusyLock.current = false;
      setGlobalBusy(false);
    }
  }, [globalBusy]);

  const selectRoot = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: '选择文件索引目录' });
      if (!selected || Array.isArray(selected)) return;
      await withGlobalBusy(async () => {
        const next = await setFileSearchRoot(selected);
        setStatus(next);
        setSearchResponse(null);
        setSelectedPath(null);
        setPreview(null);
        setNotice('目录已设置。点击“刷新”建立文件索引。');
      });
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const clearRoot = async () => {
    if (!window.confirm('移除当前索引目录并清除它的索引记录？磁盘文件不会被删除。')) return;
    await withGlobalBusy(async () => {
      const next = await setFileSearchRoot(null);
      setStatus(next);
      setSearchResponse(null);
      setSelectedPath(null);
      setPreview(null);
      setNotice('索引目录已移除，磁盘文件未受影响。');
    });
  };

  const searchNow = async () => {
    if (!status?.root) {
      setError('请先选择一个索引目录。');
      return;
    }
    await runSearch(query);
  };

  const refreshIndex = async () => {
    await withGlobalBusy(async () => {
      const next = await refreshFileIndex();
      setStatus(next);
      setNotice(`目录扫描完成，共索引 ${next.count.toLocaleString()} 项。`);
      await runSearch(query);
    });
  };

  const togglePaused = async () => {
    if (!status) return;
    await withGlobalBusy(async () => {
      const next = await setFileSearchPaused(!status.paused);
      setStatus(next);
      setNotice(next.paused ? '索引已暂停，现有结果仍可搜索。' : '索引已恢复。');
      await runSearch(query);
    });
  };

  const clearIndex = async () => {
    if (!window.confirm('清除当前目录的索引记录？目录设置保留，磁盘文件不会被删除。')) return;
    await withGlobalBusy(async () => {
      const next = await clearFileIndex();
      setStatus(next);
      setSearchResponse(null);
      setSelectedPath(null);
      setPreview(null);
      setNotice('索引记录已清除。');
    });
  };

  const copyPath = async (file: FileEntry) => {
    await withBusyPath(file.path, async () => {
      const copiedPath = await copyFileSearchPath(file.path, generation);
      await copyTextToClipboard(copiedPath);
      setNotice('路径已复制。');
    });
  };

  const startRename = (file: FileEntry) => {
    setRenamingPath(file.path);
    setNewName(file.name);
  };

  const commitRename = async (file: FileEntry) => {
    const trimmed = newName.trim();
    if (!trimmed || trimmed === file.name) {
      setRenamingPath(null);
      return;
    }
    await withBusyPath(file.path, async () => {
      const response = await renameIndexedFile(file.path, trimmed, generation);
      setRenamingPath(null);
      setStatus((current) => current ? { ...current, generation: response.generation } : current);
      setNotice('文件名已更新。');
      await runSearch(query);
      setSelectedPath(response.entry.path);
    });
  };

  const chooseDestination = async (file: FileEntry, kind: 'move' | 'copy') => {
    try {
      const selected = await open({ directory: true, multiple: false, title: kind === 'move' ? '选择目标目录' : '选择复制目标目录' });
      if (!selected || Array.isArray(selected)) return;
      await withBusyPath(file.path, async () => {
        const response = kind === 'move'
          ? await moveIndexedFile(file.path, selected, generation)
          : await copyIndexedFile(file.path, selected, generation);
        setStatus((current) => current ? { ...current, generation: response.generation } : current);
        setNotice(kind === 'move' ? '文件已移动。' : '文件副本已创建。');
        await runSearch(query);
        setSelectedPath(response.entry.path);
      });
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const recycle = async (file: FileEntry) => {
    if (!window.confirm(`将“${file.name}”移至回收站？`)) return;
    await withBusyPath(file.path, async () => {
      const response = await deleteToRecycleBin(file.path, generation);
      setStatus((current) => current ? { ...current, generation: response.generation } : current);
      setSearchResponse((current) => current ? {
        ...current,
        items: current.items.filter((candidate) => candidate.path !== file.path),
        total: Math.max(0, current.total - 1),
        generation: response.generation,
      } : current);
      setSelectedPath(null);
      setNotice('文件已移至回收站。');
    });
  };

  const root = status?.root ?? null;

  return (
    <section className="flex h-full min-h-0 flex-col text-[color:var(--manager-ink)]">
      <header className="shrink-0 border-b border-[color:var(--manager-border)] px-3 py-3 sm:px-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="min-w-0">
            <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              <h2 className="text-xl font-semibold text-[color:var(--manager-ink-strong)]">文件</h2>
              <span role="status" className="text-xs text-[color:var(--manager-ink-muted)]">{statusLabel(status)}</span>
            </div>
            <p className="mt-1 max-w-[min(64vw,720px)] truncate text-xs text-[color:var(--manager-ink-subtle)]" title={root ?? undefined}>{root ?? '搜索仅限于你选择的本地目录'}</p>
          </div>
          <div className="flex flex-wrap items-center gap-1">
            <button type="button" onClick={() => void selectRoot()} disabled={globalBusy} className="files-tool-button">{root ? '更换目录' : '选择目录'}</button>
            <button type="button" onClick={() => void refreshIndex()} disabled={globalBusy || !root || status?.paused} className="files-tool-button" title="扫描受控目录">刷新</button>
            <button type="button" onClick={() => void togglePaused()} disabled={globalBusy || !root} className="files-tool-button" title={status?.paused ? '恢复索引' : '暂停索引'}>{status?.paused ? '继续索引' : '暂停'}</button>
            <button type="button" onClick={() => void clearIndex()} disabled={globalBusy || !root || status?.count === 0} className="files-tool-button files-tool-danger">清除索引</button>
            {root ? <button type="button" onClick={() => void clearRoot()} disabled={globalBusy} className="files-tool-button files-tool-danger">移除目录</button> : null}
          </div>
        </div>

        <div className="mt-3 flex min-w-0 flex-wrap items-center gap-2">
          <label className="relative min-w-[180px] flex-1 sm:max-w-[560px]">
            <span className="sr-only">搜索文件</span>
            <span aria-hidden="true" className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-sm text-[color:var(--manager-ink-subtle)]">⌕</span>
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => { if (event.key === 'Enter') void searchNow(); }}
              disabled={!root}
              placeholder={root ? '输入名称或路径，自动搜索' : '请先选择索引目录'}
              className="w-full rounded-[8px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] py-2 pl-9 pr-3 text-sm text-[color:var(--manager-ink-strong)] outline-none placeholder:text-[color:var(--manager-ink-subtle)] focus:border-[color:var(--manager-accent)] disabled:opacity-60"
            />
          </label>
          <button type="button" onClick={() => void searchNow()} disabled={!root || isSearching} className="files-tool-button">搜索</button>
          {isSearching ? <span className="text-xs text-[color:var(--manager-ink-muted)]">搜索中…</span> : null}
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-3 sm:px-4">
        {error ? <p role="alert" className="manager-alert-error mb-3 rounded-[8px] border px-3 py-2 text-xs">{error}</p> : null}
        {notice ? <p role="status" className="mb-3 text-xs text-[color:var(--manager-success)]">{notice}</p> : null}
        {isLoading ? <p className="py-8 text-center text-sm text-[color:var(--manager-ink-muted)]">正在读取索引…</p> : null}

        {!isLoading && !root ? (
          <div className="py-12 text-center">
            <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">先选择一个索引目录</p>
            <p className="mt-1 text-xs text-[color:var(--manager-ink-muted)]">仅检索所选目录。添加目录不会上传或修改文件。</p>
            <button type="button" onClick={() => void selectRoot()} disabled={globalBusy} className="files-tool-button mt-3">选择目录</button>
          </div>
        ) : null}

        {!isLoading && root && status?.count === 0 ? (
          <div className="py-10 text-center text-sm text-[color:var(--manager-ink-muted)]">{status.paused ? '索引已暂停，请先恢复索引。' : '目录尚未建立索引，点击“刷新”开始扫描。'}</div>
        ) : null}

        {!isLoading && root && status && status.count > 0 && !query.trim() && results.length === 0 && !isSearching ? (
          <div className="py-10 text-center text-sm text-[color:var(--manager-ink-muted)]">输入名称或路径以搜索文件</div>
        ) : null}

        {!isLoading && query.trim() && searchResponse && searchResponse.items.length === 0 && !isSearching ? (
          <div className="py-10 text-center text-sm text-[color:var(--manager-ink-muted)]">没有匹配的文件</div>
        ) : null}

        {!isLoading && results.length > 0 ? (
          <div className="grid min-h-[240px] gap-4 lg:grid-cols-[minmax(0,1.55fr)_minmax(240px,0.85fr)]">
            <div className="min-w-0">
              <div className="mb-2 flex items-center justify-between gap-2 text-[11px] text-[color:var(--manager-ink-subtle)]">
                <span>{query.trim() ? `${searchResponse?.total.toLocaleString()} 个结果` : '最近使用'}</span>
                <span className="hidden sm:inline">名称 · 位置 · 大小 · 修改时间 · 类型</span>
              </div>
              <div className="divide-y divide-[color:var(--manager-border)] border-y border-[color:var(--manager-border)]">
                {results.map((file) => {
                  const selected = file.path === selectedPath;
                  const busy = busyPaths.has(file.path);
                  return (
                    <article key={file.id} className={`min-w-0 py-2 ${selected ? 'bg-[color:var(--manager-accent-soft)]' : ''}`}>
                      <button type="button" aria-pressed={selected} onClick={() => setSelectedPath(file.path)} className="flex w-full min-w-0 items-start gap-2 px-2 text-left">
                        <span aria-hidden="true" className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] text-[10px] text-[color:var(--manager-accent)]">{file.isDir ? 'DIR' : fileType(file).slice(0, 3)}</span>
                        <span className="min-w-0 flex-1">
                          {renamingPath === file.path ? (
                            <span className="flex min-w-0 gap-1" onClick={(event) => event.stopPropagation()}>
                              <input aria-label="新文件名" autoFocus value={newName} onChange={(event) => setNewName(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') { event.preventDefault(); void commitRename(file); } if (event.key === 'Escape') setRenamingPath(null); }} className="min-w-0 flex-1 rounded-[6px] border border-[color:var(--manager-accent)] bg-[color:var(--manager-panel-strong)] px-2 py-1 text-xs outline-none" />
                              <button type="button" disabled={busy} onClick={() => void commitRename(file)} className="rounded-[6px] px-2 text-xs text-[color:var(--manager-accent)] disabled:opacity-50">保存</button>
                              <button type="button" onClick={() => setRenamingPath(null)} className="rounded-[6px] px-1 text-xs text-[color:var(--manager-ink-muted)]">取消</button>
                            </span>
                          ) : <span className="block truncate text-sm font-medium text-[color:var(--manager-ink-strong)]">{file.name}</span>}
                          <span className="mt-0.5 block break-all text-[11px] leading-4 text-[color:var(--manager-ink-subtle)]">{file.path}</span>
                          <span className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-[10px] text-[color:var(--manager-ink-muted)] sm:hidden">{formatSize(file.size)}<span>{formatTime(file.modifiedAt)}</span><span>{fileType(file)}</span></span>
                        </span>
                        <span className="hidden shrink-0 text-right text-[11px] leading-5 text-[color:var(--manager-ink-muted)] sm:block">{formatSize(file.size)}<br />{formatTime(file.modifiedAt)}<br />{fileType(file)}</span>
                      </button>
                      <div className="mt-1 flex flex-wrap gap-x-1 gap-y-0.5 pl-11">
                        <button type="button" disabled={busy} onClick={() => void withBusyPath(file.path, async () => { await openFile(file.path, generation); })} className="files-row-action">打开</button>
                        <button type="button" disabled={busy} onClick={() => void withBusyPath(file.path, async () => { await revealFileInExplorer(file.path, generation); })} className="files-row-action">定位</button>
                        <button type="button" disabled={busy} onClick={() => void copyPath(file)} className="files-row-action">复制路径</button>
                        <button type="button" disabled={busy || file.isDir} onClick={() => startRename(file)} className="files-row-action">重命名</button>
                        <button type="button" disabled={busy || file.isDir} onClick={() => void chooseDestination(file, 'move')} className="files-row-action">移动</button>
                        <button type="button" disabled={busy || file.isDir} onClick={() => void chooseDestination(file, 'copy')} className="files-row-action">复制到</button>
                        <button type="button" disabled={busy || file.isDir} onClick={() => void recycle(file)} className="files-row-action files-row-action-danger">{busy ? '处理中…' : '回收站'}</button>
                      </div>
                    </article>
                  );
                })}
              </div>
            </div>

            <aside className="min-w-0 border-t border-[color:var(--manager-border)] pt-3 lg:border-l lg:border-t-0 lg:pl-4 lg:pt-0">
              <div className="flex min-w-0 items-start justify-between gap-2">
                <div className="min-w-0">
                  <p className="manager-kicker">Preview</p>
                  <h3 className="mt-1 break-words text-sm font-medium text-[color:var(--manager-ink-strong)]">{selectedFile?.name ?? '预览'}</h3>
                </div>
                {selectedFile ? <span className="shrink-0 text-[10px] text-[color:var(--manager-ink-muted)]">{formatSize(selectedFile.size)}</span> : null}
              </div>
              {previewError ? <p role="alert" className="manager-alert-error mt-3 rounded-[8px] border px-3 py-2 text-xs">无法预览：{previewError}</p> : null}
              {selectedFile && !preview && !previewError ? <p className="mt-4 text-xs text-[color:var(--manager-ink-muted)]">正在读取预览…</p> : null}
              {preview?.kind === 'text' ? (
                <div className="mt-3 min-w-0">
                  <p className="mb-2 text-[10px] text-[color:var(--manager-ink-subtle)]">{preview.mimeType ?? '文本'}{preview.truncated ? ' · 内容已截断' : ''}</p>
                  <pre className="max-h-[42vh] min-h-[100px] overflow-auto whitespace-pre-wrap break-words rounded-[8px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-3 text-xs leading-5 text-[color:var(--manager-ink-soft)]">{preview.content ?? ''}</pre>
                </div>
              ) : null}
              {preview?.kind === 'image' ? (
                <div className="mt-3 flex min-h-[120px] items-center justify-center overflow-hidden rounded-[8px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-2">
                  {preview.imageDataUrl ? <img src={preview.imageDataUrl} alt={selectedFile?.name ?? '文件预览'} className="max-h-[42vh] max-w-full object-contain" /> : <p className="text-xs text-[color:var(--manager-ink-muted)]">{preview.truncated ? '图片超过预览大小限制，可直接打开文件。' : '图片预览不可用'}</p>}
                </div>
              ) : null}
              {preview?.kind === 'unsupported' ? <p className="mt-4 text-xs text-[color:var(--manager-ink-muted)]">此文件类型暂不支持预览，可直接打开文件。</p> : null}
              {preview?.truncated && preview.kind === 'text' ? <p className="mt-2 text-[10px] text-[color:var(--manager-ink-subtle)]">文本内容已截断。</p> : null}
            </aside>
          </div>
        ) : null}
      </div>
    </section>
  );
}

