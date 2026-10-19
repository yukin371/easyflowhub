/**
 * Script file management dialogs: source workbench, create, import, roots.
 *
 * The manager used to be read-only: scripts were discovered and could only be
 * run by the Launcher or an AI agent. These dialogs give the user direct
 * control over the files (view / edit / run / import / pin storage locations).
 */

import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { rootsApi, scriptsApi, tasksApi } from '../../../lib/api/scriptmgr';
import type { RunScriptResponse } from '../../../lib/api/scriptmgr';
import type { RootsDetail, ScriptSummary } from '../../../types/scriptmgr';

const SCRIPT_EXTENSIONS = ['ps1', 'py', 'bat', 'cmd'];

function Modal({
  title,
  subtitle,
  onClose,
  children,
  footer,
  wide = false,
}: {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 p-4"
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <div
        className={`flex max-h-[90vh] w-full ${
          wide ? 'max-w-[880px]' : 'max-w-[560px]'
        } flex-col overflow-hidden rounded-[20px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] shadow-[0_30px_80px_rgba(0,0,0,0.35)]`}
      >
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-[color:var(--manager-border)] px-5 py-4">
          <div className="min-w-0">
            <h3 className="truncate text-base font-medium text-[color:var(--manager-ink-strong)]">{title}</h3>
            {subtitle ? (
              <p className="mt-1 truncate text-xs text-[color:var(--manager-ink-subtle)]">{subtitle}</p>
            ) : null}
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="关闭"
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-[color:var(--manager-ink-muted)] transition hover:bg-[color:var(--manager-accent-soft)] hover:text-[color:var(--manager-ink-strong)]"
          >
            ✕
          </button>
        </header>
        <div className="min-h-0 flex-1 overflow-auto px-5 py-4">{children}</div>
        {footer ? (
          <footer className="shrink-0 border-t border-[color:var(--manager-border)] px-5 py-3">{footer}</footer>
        ) : null}
      </div>
    </div>
  );
}

const primaryButton =
  'rounded-[10px] bg-[color:var(--manager-accent)] px-4 py-2 text-sm text-white transition disabled:opacity-50';
const secondaryButton =
  'rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)] disabled:opacity-50';
const dangerButton =
  'rounded-[10px] border border-[color:var(--manager-danger)] px-4 py-2 text-sm text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/10 disabled:opacity-50';
const inputClass =
  'w-full rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)] outline-none transition placeholder:text-[color:var(--manager-ink-subtle)] focus:border-[color:var(--manager-accent)]';

// ============================================================================
// Source workbench: view / edit / run / delete
// ============================================================================

export function ScriptWorkbenchDialog({
  script,
  onClose,
  onChanged,
}: {
  script: ScriptSummary;
  onClose: () => void;
  onChanged: () => void;
}) {
  const [tab, setTab] = useState<'source' | 'run'>('source');
  const [source, setSource] = useState<string | null>(null);
  const [draft, setDraft] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [args, setArgs] = useState('');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<RunScriptResponse | null>(null);

  const loadSource = useCallback(async () => {
    setLoading(true);
    try {
      const loaded = await scriptsApi.readSource(script.id);
      setSource(loaded.content);
      setDraft(loaded.content);
      setError(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setLoading(false);
    }
  }, [script.id]);

  useEffect(() => {
    void loadSource();
  }, [loadSource]);

  const dirty = source !== null && draft !== source;

  const handleSave = async () => {
    setSaving(true);
    setError(undefined);
    try {
      const saved = await scriptsApi.saveSource(script.id, draft);
      setSource(saved.content);
      setDraft(saved.content);
      setNotice('已保存');
      onChanged();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setSaving(false);
    }
  };

  const handleRun = async () => {
    setRunning(true);
    setError(undefined);
    setResult(null);
    try {
      const parsedArgs = args.trim() ? args.trim().split(/\s+/) : [];
      const response = await tasksApi.run({ script_id: script.id, args: parsedArgs, async: false });
      setResult(response);
      onChanged();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setRunning(false);
    }
  };

  const handleDelete = async () => {
    if (!window.confirm(`将“${script.name}”移入回收站？可以在回收站中恢复。`)) return;
    try {
      await scriptsApi.remove(script.id);
      onChanged();
      onClose();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  return (
    <Modal
      wide
      title={script.name}
      subtitle={`${script.script_type} · ${script.path}`}
      onClose={onClose}
      footer={
        <div className="flex flex-wrap items-center justify-between gap-2">
          <button type="button" className={dangerButton} onClick={() => void handleDelete()}>
            删除到回收站
          </button>
          <div className="flex items-center gap-2">
            {tab === 'source' ? (
              <>
                <button type="button" className={secondaryButton} onClick={() => void loadSource()} disabled={loading}>
                  重新加载
                </button>
                <button type="button" className={primaryButton} onClick={() => void handleSave()} disabled={saving || !dirty}>
                  {saving ? '保存中…' : dirty ? '保存修改' : '已保存'}
                </button>
              </>
            ) : (
              <button type="button" className={primaryButton} onClick={() => void handleRun()} disabled={running}>
                {running ? '执行中…' : '执行脚本'}
              </button>
            )}
          </div>
        </div>
      }
    >
      <div className="mb-3 flex items-center gap-2" role="tablist" aria-label="脚本工作台">
        {(['source', 'run'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={tab === value}
            onClick={() => setTab(value)}
            className={`rounded-[8px] border px-3 py-1.5 text-sm transition ${
              tab === value
                ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                : 'border-transparent text-[color:var(--manager-ink-muted)] hover:bg-[color:var(--manager-panel-strong)]'
            }`}
          >
            {value === 'source' ? '源码' : '运行'}
          </button>
        ))}
        {dirty ? <span className="text-xs text-[color:var(--manager-warning)]">有未保存的修改</span> : null}
        {notice ? <span role="status" className="text-xs text-[color:var(--manager-ink-soft)]">{notice}</span> : null}
      </div>

      {error ? (
        <p role="alert" className="mb-3 rounded-[10px] border border-[color:var(--manager-danger)]/40 px-3 py-2 text-xs text-[color:var(--manager-danger)]">
          {error}
        </p>
      ) : null}

      {tab === 'source' ? (
        loading ? (
          <p className="py-10 text-center text-sm text-[color:var(--manager-ink-muted)]">加载源码…</p>
        ) : (
          <textarea
            aria-label="脚本源码"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            spellCheck={false}
            className="h-[46vh] w-full resize-none rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-3 font-mono text-xs leading-6 text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          />
        )
      ) : (
        <div className="space-y-3">
          <label className="block">
            <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">
              参数（空格分隔，可留空）
            </span>
            <input
              aria-label="脚本参数"
              value={args}
              onChange={(event) => setArgs(event.target.value)}
              placeholder="--dry-run input.txt"
              className={inputClass}
            />
          </label>

          {result ? (
            <div className="space-y-2">
              <div className="flex flex-wrap items-center gap-3 text-sm">
                <span
                  className={
                    result.exit_code === 0 || result.status === 'success' || result.status === 'completed'
                      ? 'text-[color:var(--manager-success)]'
                      : 'text-[color:var(--manager-danger)]'
                  }
                >
                  {result.exit_code === 0 || result.status === 'success' || result.status === 'completed'
                    ? '执行成功'
                    : '执行失败'}
                </span>
                <span className="text-[color:var(--manager-ink-muted)]">退出码 {result.exit_code}</span>
                {typeof result.duration_ms === 'number' ? (
                  <span className="text-[color:var(--manager-ink-muted)]">{result.duration_ms} ms</span>
                ) : null}
                {result.output_meta?.truncated ? (
                  <span className="text-[color:var(--manager-warning)]">输出已截断</span>
                ) : null}
              </div>
              <pre
                aria-label="执行结果"
                className="max-h-[38vh] overflow-auto whitespace-pre-wrap break-words rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-3 font-mono text-xs leading-6 text-[color:var(--manager-ink-soft)]"
              >
                {result.output?.trim() ? result.output : '（无输出）'}
              </pre>
            </div>
          ) : (
            <p className="py-8 text-center text-sm text-[color:var(--manager-ink-muted)]">
              执行后可在此查看退出码与输出
            </p>
          )}
        </div>
      )}
    </Modal>
  );
}

// ============================================================================
// Create script
// ============================================================================

const TEMPLATES: Record<string, string> = {
  ps1: '# 新脚本\nparam()\n\nWrite-Host "Hello from EasyFlowHub"\n',
  py: '#!/usr/bin/env python3\n\n\ndef main() -> None:\n    print("Hello from EasyFlowHub")\n\n\nif __name__ == "__main__":\n    main()\n',
  bat: '@echo off\n\necho Hello from EasyFlowHub\n',
  cmd: '@echo off\n\necho Hello from EasyFlowHub\n',
};

export function ScriptCreateDialog({
  roots,
  onClose,
  onCreated,
}: {
  roots: RootsDetail | null;
  onClose: () => void;
  onCreated: (script: ScriptSummary) => void;
}) {
  const availableRoots = useMemo(() => roots?.roots ?? [], [roots]);
  const [root, setRoot] = useState(availableRoots[0] ?? '');
  const [filename, setFilename] = useState('new-script.ps1');
  const [name, setName] = useState('');
  const [category, setCategory] = useState('');
  const [description, setDescription] = useState('');
  const [content, setContent] = useState(TEMPLATES.ps1);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!root && availableRoots.length > 0) setRoot(availableRoots[0]);
  }, [availableRoots, root]);

  const handleFilenameChange = (value: string) => {
    setFilename(value);
    const extension = value.split('.').pop()?.toLowerCase() ?? '';
    if (TEMPLATES[extension] && !name) {
      setContent(TEMPLATES[extension]);
    }
  };

  const handleCreate = async () => {
    setSaving(true);
    setError(undefined);
    try {
      const script = await scriptsApi.create({ root, filename, content, name, category, description });
      onCreated(script);
      onClose();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title="新建脚本"
      subtitle="写入已配置的存储位置，保存后立即出现在脚本库中"
      onClose={onClose}
      footer={
        <div className="flex items-center justify-end gap-2">
          <button type="button" className={secondaryButton} onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className={primaryButton}
            onClick={() => void handleCreate()}
            disabled={saving || !root || !filename.trim()}
          >
            {saving ? '创建中…' : '创建脚本'}
          </button>
        </div>
      }
    >
      <div className="space-y-3">
        {error ? (
          <p role="alert" className="rounded-[10px] border border-[color:var(--manager-danger)]/40 px-3 py-2 text-xs text-[color:var(--manager-danger)]">
            {error}
          </p>
        ) : null}
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">存储位置</span>
          <select aria-label="存储位置" value={root} onChange={(event) => setRoot(event.target.value)} className={inputClass}>
            {availableRoots.length === 0 ? <option value="">请先添加存储位置</option> : null}
            {availableRoots.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">文件名</span>
          <input
            aria-label="文件名"
            value={filename}
            onChange={(event) => handleFilenameChange(event.target.value)}
            placeholder="deploy.ps1"
            className={inputClass}
          />
          <span className="mt-1 block text-[11px] text-[color:var(--manager-ink-subtle)]">
            支持 {SCRIPT_EXTENSIONS.map((ext) => `.${ext}`).join(' / ')}
          </span>
        </label>
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="block">
            <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">显示名称</span>
            <input aria-label="显示名称" value={name} onChange={(event) => setName(event.target.value)} placeholder="可选" className={inputClass} />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">分类</span>
            <input aria-label="分类" value={category} onChange={(event) => setCategory(event.target.value)} placeholder="可选" className={inputClass} />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">描述</span>
          <input aria-label="描述" value={description} onChange={(event) => setDescription(event.target.value)} placeholder="可选" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">初始内容</span>
          <textarea
            aria-label="初始内容"
            value={content}
            onChange={(event) => setContent(event.target.value)}
            spellCheck={false}
            className="h-40 w-full resize-y rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-3 font-mono text-xs leading-6 text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          />
        </label>
      </div>
    </Modal>
  );
}

// ============================================================================
// Import scripts
// ============================================================================

export function ScriptImportDialog({
  roots,
  onClose,
  onImported,
}: {
  roots: RootsDetail | null;
  onClose: () => void;
  onImported: () => void;
}) {
  const availableRoots = useMemo(() => roots?.roots ?? [], [roots]);
  const [root, setRoot] = useState(availableRoots[0] ?? '');
  const [paths, setPaths] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string>();
  const [summary, setSummary] = useState<string>();

  useEffect(() => {
    if (!root && availableRoots.length > 0) setRoot(availableRoots[0]);
  }, [availableRoots, root]);

  const pickFiles = async () => {
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [{ name: '脚本', extensions: SCRIPT_EXTENSIONS }],
    });
    if (!selected) return;
    const list = Array.isArray(selected) ? selected : [selected];
    setPaths((current) => Array.from(new Set([...current, ...list])));
  };

  const handleImport = async () => {
    setImporting(true);
    setError(undefined);
    setSummary(undefined);
    try {
      const response = await scriptsApi.importFiles(root, paths);
      const imported = response.imported?.length ?? 0;
      const failed = response.failed ?? [];
      setSummary(`已导入 ${imported} 个脚本${failed.length > 0 ? `，${failed.length} 个失败` : ''}`);
      if (failed.length > 0) {
        setError(failed.map((item) => `${item.path}: ${item.error}`).join('\n'));
      }
      setPaths([]);
      onImported();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setImporting(false);
    }
  };

  return (
    <Modal
      title="导入脚本"
      subtitle="从磁盘复制脚本文件到已配置的存储位置（不覆盖同名文件）"
      onClose={onClose}
      footer={
        <div className="flex items-center justify-end gap-2">
          <button type="button" className={secondaryButton} onClick={onClose}>
            关闭
          </button>
          <button
            type="button"
            className={primaryButton}
            onClick={() => void handleImport()}
            disabled={importing || !root || paths.length === 0}
          >
            {importing ? '导入中…' : `导入 ${paths.length} 个文件`}
          </button>
        </div>
      }
    >
      <div className="space-y-3">
        {summary ? (
          <p role="status" className="rounded-[10px] border border-[color:var(--manager-success)]/40 px-3 py-2 text-xs text-[color:var(--manager-success)]">
            {summary}
          </p>
        ) : null}
        {error ? (
          <p role="alert" className="whitespace-pre-wrap rounded-[10px] border border-[color:var(--manager-danger)]/40 px-3 py-2 text-xs text-[color:var(--manager-danger)]">
            {error}
          </p>
        ) : null}
        <label className="block">
          <span className="mb-1 block text-xs uppercase tracking-[0.14em] text-[color:var(--manager-ink-muted)]">导入到</span>
          <select aria-label="导入到" value={root} onChange={(event) => setRoot(event.target.value)} className={inputClass}>
            {availableRoots.length === 0 ? <option value="">请先添加存储位置</option> : null}
            {availableRoots.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </label>
        <button type="button" className={secondaryButton} onClick={() => void pickFiles()}>
          选择脚本文件…
        </button>
        {paths.length > 0 ? (
          <ul className="space-y-1">
            {paths.map((path) => (
              <li key={path} className="flex items-center justify-between gap-2 rounded-[8px] bg-[color:var(--manager-panel-strong)] px-3 py-1.5 text-xs text-[color:var(--manager-ink-soft)]">
                <span className="truncate font-mono">{path}</span>
                <button
                  type="button"
                  onClick={() => setPaths((current) => current.filter((item) => item !== path))}
                  className="shrink-0 text-[color:var(--manager-ink-subtle)] hover:text-[color:var(--manager-danger)]"
                >
                  移除
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-xs text-[color:var(--manager-ink-subtle)]">尚未选择文件</p>
        )}
      </div>
    </Modal>
  );
}

// ============================================================================
// Roots (storage locations)
// ============================================================================

export function ScriptRootsDialog({
  onClose,
  onChanged,
}: {
  onClose: () => void;
  onChanged: (roots: RootsDetail) => void;
}) {
  const [roots, setRoots] = useState<RootsDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string>();
  const [error, setError] = useState<string>();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const detail = await rootsApi.list();
      setRoots(detail);
      onChanged(detail);
      setError(undefined);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setLoading(false);
    }
  }, [onChanged]);

  useEffect(() => {
    void load();
  }, [load]);

  const handleAdd = async () => {
    const selected = await open({ multiple: false, directory: true });
    if (!selected || Array.isArray(selected)) return;
    setBusy('add');
    try {
      const detail = await rootsApi.add(selected);
      setRoots(detail);
      onChanged(detail);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setBusy(undefined);
    }
  };

  const handleRemove = async (path: string) => {
    setBusy(path);
    try {
      const detail = await rootsApi.remove(path);
      setRoots(detail);
      onChanged(detail);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setBusy(undefined);
    }
  };

  const handlePin = async (path: string) => {
    setBusy(path);
    try {
      const detail = await rootsApi.add(path);
      setRoots(detail);
      onChanged(detail);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setBusy(undefined);
    }
  };

  const custom = new Set(roots?.custom ?? []);

  return (
    <Modal
      title="存储位置"
      subtitle="固定后的目录会在每次启动时加载；自动发现的目录取决于启动位置"
      onClose={onClose}
      footer={
        <div className="flex items-center justify-between gap-2">
          <button type="button" className={secondaryButton} onClick={() => void handleAdd()} disabled={busy === 'add'}>
            添加目录…
          </button>
          <button type="button" className={secondaryButton} onClick={onClose}>
            关闭
          </button>
        </div>
      }
    >
      {error ? (
        <p role="alert" className="mb-3 rounded-[10px] border border-[color:var(--manager-danger)]/40 px-3 py-2 text-xs text-[color:var(--manager-danger)]">
          {error}
        </p>
      ) : null}
      {loading ? (
        <p className="py-8 text-center text-sm text-[color:var(--manager-ink-muted)]">加载中…</p>
      ) : (roots?.roots.length ?? 0) === 0 ? (
        <p className="py-8 text-center text-sm text-[color:var(--manager-ink-muted)]">
          还没有存储位置，点击“添加目录”选择脚本所在文件夹。
        </p>
      ) : (
        <ul className="space-y-2">
          {(roots?.roots ?? []).map((path) => (
            <li
              key={path}
              className="flex items-start justify-between gap-3 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2.5"
            >
              <div className="min-w-0">
                <p className="break-all font-mono text-xs text-[color:var(--manager-ink-soft)]">{path}</p>
                <p className="mt-1 text-[11px] text-[color:var(--manager-ink-subtle)]">
                  {custom.has(path) ? '已固定' : '自动发现'}
                </p>
              </div>
              {custom.has(path) ? (
                <button
                  type="button"
                  onClick={() => void handleRemove(path)}
                  disabled={busy === path}
                  className="shrink-0 text-xs text-[color:var(--manager-ink-muted)] hover:text-[color:var(--manager-danger)]"
                >
                  取消固定
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => void handlePin(path)}
                  disabled={busy === path}
                  className="shrink-0 text-xs text-[color:var(--manager-accent)] hover:underline"
                >
                  固定此目录
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}

// ============================================================================
// Shared delete confirmation for a script row
// ============================================================================

export async function moveScriptToTrash(script: ScriptSummary): Promise<void> {
  if (!window.confirm(`将“${script.name}”移入回收站？可以在回收站中恢复。`)) return;
  await scriptsApi.remove(script.id);
}
