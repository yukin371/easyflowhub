/**
 * Script workbench: view / edit source, run manually, delete to trash.
 */

import { useCallback, useEffect, useState } from 'react';
import { scriptsApi, tasksApi } from '../../../lib/api/scriptmgr';
import type { RunScriptResponse } from '../../../lib/api/scriptmgr';
import type { ScriptSummary } from '../../../types/scriptmgr';
import { Modal, dangerButton, primaryButton, secondaryButton } from './ScriptDialogShell';

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

  const succeeded =
    result !== null &&
    (result.exit_code === 0 || result.status === 'success' || result.status === 'completed');

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
              className="w-full rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)] outline-none transition placeholder:text-[color:var(--manager-ink-subtle)] focus:border-[color:var(--manager-accent)]"
            />
          </label>

          {result ? (
            <div className="space-y-2">
              <div className="flex flex-wrap items-center gap-3 text-sm">
                <span
                  className={
                    succeeded ? 'text-[color:var(--manager-success)]' : 'text-[color:var(--manager-danger)]'
                  }
                >
                  {succeeded ? '执行成功' : '执行失败'}
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
