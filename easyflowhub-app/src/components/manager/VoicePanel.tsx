import { useCallback, useEffect, useMemo, useState } from 'react';
import { getVoiceState, openVoice, openVoicePrivacySettings, pauseVoice, resumeVoice, startVoice, stopVoice } from '../../lib/tauri/voice';
import { convertInboxItemToNote } from '../../lib/tauri/inbox';
import { TranscribeControl } from './shared/TranscribeControl';
import type { VoiceSession, VoiceStatus } from '../../types/voice';

export const VOICE_OPEN_INBOX_EVENT = 'easyflowhub:voice-open-inbox';
export const VOICE_NAVIGATE_INBOX_EVENT = 'easyflowhub:voice-navigate-inbox';

const STATUS_LABELS: Record<VoiceStatus, string> = {
  idle: '待机', recording: '录音中', paused: '已暂停', stopping: '正在保存', saved: '已保存', failed: '失败',
};
const ACTIVE_STATUSES: VoiceStatus[] = ['recording', 'paused', 'stopping'];

function formatDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(durationMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const clock = `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  return hours > 0 ? `${String(hours).padStart(2, '0')}:${clock}` : clock;
}

function errorMessage(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught);
}

export function VoicePanel() {
  const [session, setSession] = useState<VoiceSession>();
  const [title, setTitle] = useState('');
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [busyAction, setBusyAction] = useState<'start' | 'pause' | 'resume' | 'stop' | 'open' | 'privacy' | 'convert' | null>(null);

  const refresh = useCallback(async () => {
    try { setSession(await getVoiceState()); } catch (caught) { setError(errorMessage(caught)); }
  }, []);
  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    if (!session || !ACTIVE_STATUSES.includes(session.status)) return undefined;
    const timer = window.setInterval(() => void refresh(), 500);
    return () => window.clearInterval(timer);
  }, [refresh, session?.status]);

  const status = session?.status ?? 'idle';
  const isActive = ACTIVE_STATUSES.includes(status);
  const isBusy = busyAction !== null;
  const displayError = error ?? session?.error;
  const statusDescription = useMemo(() => STATUS_LABELS[status], [status]);

  const runAction = useCallback(async (action: 'pause' | 'resume' | 'stop', operation: () => Promise<VoiceSession>) => {
    setError(undefined);
    setBusyAction(action);
    try { setSession(await operation()); }
    catch (caught) { setError(errorMessage(caught)); await refresh(); }
    finally { setBusyAction(null); }
  }, [refresh]);

  const begin = async () => {
    setError(undefined);
    setBusyAction('start');
    try { setSession(await startVoice(title)); }
    catch (caught) { setError(errorMessage(caught)); await refresh(); }
    finally { setBusyAction(null); }
  };

  const openRecording = async () => {
    if (!session?.id) return;
    setError(undefined); setBusyAction('open');
    try { await openVoice(session.id); }
    catch (caught) { setError(errorMessage(caught)); }
    finally { setBusyAction(null); }
  };

  const openInbox = () => window.dispatchEvent(new CustomEvent(VOICE_NAVIGATE_INBOX_EVENT, { detail: { itemId: session?.inboxItemId ?? null } }));

  const convertToNote = async () => {
    if (!session?.inboxItemId) return;
    setError(undefined);
    setBusyAction('convert');
    try {
      await convertInboxItemToNote(session.inboxItemId);
      setNotice('已整理为笔记，可在“笔记”页面查看');
      openInbox();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusyAction(null);
    }
  };

  const openPrivacySettings = async () => {
    setError(undefined); setBusyAction('privacy');
    try { await openVoicePrivacySettings(); }
    catch (caught) { setError(errorMessage(caught)); }
    finally { setBusyAction(null); }
  };

  return <section className="flex h-full min-h-0 flex-col p-4 text-[color:var(--manager-ink)]">
    <header className="border-b border-[color:var(--manager-border)] pb-3">
      <p className="manager-kicker">Capture</p><h2 className="mt-1 text-xl font-medium text-[color:var(--manager-ink-strong)]">录音</h2>
      <p className="mt-1 text-sm text-[color:var(--manager-ink-muted)]">录音会保存为 WAV 原件并自动加入收集箱。请在系统提示出现时允许麦克风权限。</p>
    </header>
    <div className="max-w-xl space-y-3 py-4">
      <label className="block text-sm">标题<input aria-label="录音标题" value={title} onChange={(event) => setTitle(event.target.value)} disabled={isActive || isBusy} className="mt-1 w-full rounded border border-[color:var(--manager-border)] bg-transparent px-3 py-2 disabled:opacity-60" placeholder="录音（可选）" /></label>
      <div className="rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
        <div className="flex items-center justify-between gap-3"><div><p className="text-xs text-[color:var(--manager-ink-muted)]">状态</p><p role="status" className="mt-1 text-sm font-medium text-[color:var(--manager-ink-strong)]">{statusDescription}</p></div><p aria-label="录音时长" className="font-mono text-xl tabular-nums text-[color:var(--manager-ink-strong)]">{formatDuration(session?.durationMs ?? 0)}</p></div>
        <div className="mt-3 grid grid-cols-2 gap-2 text-xs text-[color:var(--manager-ink-muted)] sm:grid-cols-3"><span>设备：{session?.deviceName ?? '尚未选择'}</span><span>采样率：{session?.sampleRate ? `${session.sampleRate / 1000} kHz` : '—'}</span><span>声道：{session?.channels ? `${session.channels} 声道` : '—'}</span></div>
      </div>
      {status === 'idle' || status === 'failed' ? <button type="button" onClick={() => void begin()} disabled={isBusy} className="rounded bg-[color:var(--manager-accent)] px-3 py-2 text-sm text-white disabled:opacity-60">{status === 'failed' ? '重新录音' : '开始录音'}</button> : null}
      {status === 'recording' ? <div className="flex flex-wrap gap-2"><button type="button" onClick={() => void runAction('pause', pauseVoice)} disabled={isBusy} className="rounded border border-[color:var(--manager-border)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)] disabled:opacity-60">{busyAction === 'pause' ? '暂停中…' : '暂停'}</button><button type="button" onClick={() => void runAction('stop', stopVoice)} disabled={isBusy} className="rounded bg-[color:var(--manager-danger)] px-3 py-2 text-sm text-white disabled:opacity-60">{busyAction === 'stop' ? '保存中…' : '停止并保存'}</button></div> : null}
      {status === 'paused' ? <div className="flex flex-wrap gap-2"><button type="button" onClick={() => void runAction('resume', resumeVoice)} disabled={isBusy} className="rounded bg-[color:var(--manager-accent)] px-3 py-2 text-sm text-white disabled:opacity-60">{busyAction === 'resume' ? '继续中…' : '继续录音'}</button><button type="button" onClick={() => void runAction('stop', stopVoice)} disabled={isBusy} className="rounded border border-[color:var(--manager-danger)] px-3 py-2 text-sm text-[color:var(--manager-danger)] disabled:opacity-60">{busyAction === 'stop' ? '保存中…' : '停止并保存'}</button></div> : null}
      {status === 'stopping' ? <p className="text-sm text-[color:var(--manager-ink-muted)]">正在完成 WAV 文件并写入收集箱，请稍候…</p> : null}
      {status === 'saved' ? <div className="space-y-3"><div className="flex flex-wrap gap-2"><button type="button" onClick={() => void openRecording()} disabled={isBusy} className="rounded bg-[color:var(--manager-accent)] px-3 py-2 text-sm text-white disabled:opacity-60">{busyAction === 'open' ? '打开中…' : '打开录音文件'}</button><button type="button" onClick={openInbox} className="rounded border border-[color:var(--manager-border)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)]">查看收集箱</button></div>{session?.inboxItemId ? <TranscribeControl itemId={session.inboxItemId} audioLabel={session.audioPath?.split(/[\\/]/).pop() ?? '录音.wav'} durationMs={session.durationMs || null} onConvertToNote={() => void convertToNote()} /> : null}</div> : null}
      {notice ? <p role="status" className="text-sm text-[color:var(--manager-ink-soft)]">{notice}</p> : null}
      {session?.audioPath ? <p className="break-all text-xs leading-5 text-[color:var(--manager-ink-subtle)]">文件：{session.audioPath}</p> : null}
      {displayError ? <div className="space-y-2"><p role="alert" className="text-sm text-[color:var(--manager-danger)]">{displayError}</p><button type="button" onClick={() => void openPrivacySettings()} disabled={isBusy} className="rounded border border-[color:var(--manager-border)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)] disabled:opacity-60">{busyAction === 'privacy' ? '打开中…' : '打开麦克风隐私设置'}</button></div> : null}
    </div>
  </section>;
}
