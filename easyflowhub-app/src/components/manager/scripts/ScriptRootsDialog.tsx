/**
 * Manage script storage roots: pin / unpin directories.
 */

import { useCallback, useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { rootsApi } from '../../../lib/api/scriptmgr';
import type { RootsDetail } from '../../../types/scriptmgr';
import { Modal, secondaryButton } from './ScriptDialogShell';

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
