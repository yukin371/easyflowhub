import { useCallback, useEffect, useState } from 'react';
import { getTranscript, transcribeAudio, type InboxTranscript, type TranscriptionResult } from '../../../lib/tauri/transcription';
import { copyTextToClipboard } from '../../../lib/tauri/clipboard';
import { getSettings } from '../../../lib/tauri/settings';
import { DEFAULT_SETTINGS } from '../../../types/settings';

export interface TranscribeControlProps {
  itemId: string;
  audioLabel?: string;
  audioSizeBytes?: number | null;
  durationMs?: number | null;
  onTranscribed?: (result: TranscriptionResult) => void;
  onConvertToNote?: () => void;
}

type Phase = 'idle' | 'confirming' | 'running' | 'done' | 'error';

const RELAY_LABEL = '本机 Relay';

function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

function formatDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}

function errorMessage(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught);
}

/**
 * Shared post-recording transcription control.
 *
 * The provider request and privacy confirmation rules live in
 * `docs/decisions/ADR-0008-voice-transcription-and-relay.md`: nothing is sent
 * until the user explicitly confirms the file, provider and privacy notice.
 */
export function TranscribeControl({
  itemId,
  audioLabel,
  audioSizeBytes,
  durationMs,
  onTranscribed,
  onConvertToNote,
}: TranscribeControlProps) {
  const [model, setModel] = useState('');
  const [phase, setPhase] = useState<Phase>('idle');
  const [transcript, setTranscript] = useState<InboxTranscript | null>(null);
  const [result, setResult] = useState<TranscriptionResult | null>(null);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();

  useEffect(() => {
    let active = true;
    void getSettings()
      .then((settings) => {
        if (active) setModel(settings.transcription?.model ?? DEFAULT_SETTINGS.transcription.model);
      })
      .catch(() => {
        // Without settings the confirmation view stays in guidance mode.
      });    void getTranscript(itemId)
      .then((existing) => {
        if (!active) return;
        setTranscript(existing);
        if (existing) setPhase('done');
      })
      .catch(() => {
        // A read failure must not block recording controls; treat as no transcript.
      });
    return () => { active = false; };
  }, [itemId]);

  const start = useCallback(() => {
    setError(undefined);
    setNotice(undefined);
    setPhase('confirming');
  }, []);

  const confirm = useCallback(async () => {
    setPhase('running');
    setError(undefined);
    try {
      const value = await transcribeAudio(itemId);
      setResult(value);
      setTranscript({
        attachmentId: value.attachmentId,
        text: value.text,
        model: value.model,
        createdAt: new Date().toISOString(),
      });
      setPhase('done');
      setNotice('转写完成');
      onTranscribed?.(value);
    } catch (caught) {
      setError(errorMessage(caught));
      setPhase(transcript ? 'done' : 'error');
    }
  }, [itemId, onTranscribed, transcript]);

  const copy = useCallback(async () => {
    const text = result?.text ?? transcript?.text;
    if (!text) return;
    try {
      await copyTextToClipboard(text);
      setNotice('已复制转写文本');
      setError(undefined);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }, [result, transcript]);

  const modelReady = model.trim().length > 0;
  const displayText = transcript?.text ?? result?.text ?? '';
  const canConvert = Boolean(onConvertToNote) && phase === 'done';

  return (
    <div className="rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <p className="text-xs text-[color:var(--manager-ink-muted)]">语音转文字</p>
          <p className="mt-1 text-sm text-[color:var(--manager-ink-strong)]">
            {phase === 'done' ? '已转写，可复制或整理为笔记' : '通过本机 Relay 转写，音频不会自动上传'}
          </p>
        </div>
        {phase !== 'running' && phase !== 'confirming' ? (
          <button
            type="button"
            onClick={phase === 'done' ? () => { setResult(null); setPhase('idle'); } : start}
            disabled={!modelReady}
            className="rounded border border-[color:var(--manager-border)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)] disabled:opacity-60"
          >
            {phase === 'done' ? '重新转写' : '转写录音'}
          </button>
        ) : null}
      </div>

      {!modelReady ? (
        <p className="mt-2 text-xs leading-6 text-[color:var(--manager-danger)]">
          请先在「设置 → 转写」中选择转写模型（Relay 可用模型），再回来转写。
        </p>
      ) : null}

      {phase === 'confirming' ? (
        <div className="mt-3 space-y-2 rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-3 text-xs leading-6 text-[color:var(--manager-ink-soft)]">
          <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">确认转写</p>
          <p>文件：{audioLabel ?? '录音'}{audioSizeBytes ? ` · ${formatSize(audioSizeBytes)}` : ''}{durationMs ? ` · 约 ${formatDuration(durationMs)}` : ''}</p>
          <p>目标 Provider：{RELAY_LABEL} · 模型：{model}</p>
          <p className="text-[color:var(--manager-danger)]">
            隐私提示：音频将通过 Relay 发送到其配置的上游 Provider；未确认前不会发送任何数据。
          </p>
          <div className="flex flex-wrap gap-2 pt-1">
            <button type="button" onClick={() => void confirm()} className="rounded bg-[color:var(--manager-accent)] px-3 py-2 text-sm text-white">确认并发送</button>
            <button type="button" onClick={() => setPhase(transcript ? 'done' : 'idle')} className="rounded border border-[color:var(--manager-border)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)]">取消</button>
          </div>
        </div>
      ) : null}

      {phase === 'running' ? <p className="mt-2 text-sm text-[color:var(--manager-ink-muted)]">转写中，请稍候（长音频可能需要数分钟）…</p> : null}

      {phase === 'done' && displayText ? (
        <div className="mt-3 space-y-2">
          <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-words rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 font-sans text-sm leading-6 text-[color:var(--manager-ink-soft)]">{displayText}</pre>
          <div className="flex flex-wrap gap-2">
            <button type="button" onClick={() => void copy()} className="rounded border border-[color:var(--manager-border)] px-3 py-1.5 text-xs text-[color:var(--manager-ink-strong)]">复制文本</button>
            {canConvert ? <button type="button" onClick={() => onConvertToNote?.()} className="rounded border border-[color:var(--manager-accent)] px-3 py-1.5 text-xs text-[color:var(--manager-accent)]">整理为笔记</button> : null}
          </div>
        </div>
      ) : null}

      {error ? <p role="alert" className="mt-2 text-xs text-[color:var(--manager-danger)]">{error}</p> : null}
      {notice ? <p role="status" className="mt-2 text-xs text-[color:var(--manager-ink-soft)]">{notice}</p> : null}
    </div>
  );
}
