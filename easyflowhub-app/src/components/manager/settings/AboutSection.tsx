import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import { SettingsSectionCard } from '../shared/SettingsSectionCard';
import {
  describeUpdate,
  fetchLatestRelease,
  RELEASES_REPO,
  type UpdateStatus,
} from '../../../lib/updateCheck';

type CheckState =
  | { phase: 'idle' }
  | { phase: 'checking' }
  | { phase: 'done'; status: UpdateStatus }
  | { phase: 'error'; message: string };

/**
 * 关于与更新：显示当前版本，并与 GitHub 最新正式发布版比对。
 * 比对独立于 Tauri updater 的安装流程，只回答「有没有更新的已发布版本」。
 */
export function AboutSection() {
  const [version, setVersion] = useState<string>('—');
  const [check, setCheck] = useState<CheckState>({ phase: 'idle' });

  useEffect(() => {
    let mounted = true;
    void getVersion()
      .then((v) => {
        if (mounted) setVersion(v);
      })
      .catch(() => {
        // 浏览器预览没有 Tauri IPC，保持占位即可。
      });
    return () => {
      mounted = false;
    };
  }, []);

  const handleCheck = async () => {
    setCheck({ phase: 'checking' });
    try {
      const latest = await fetchLatestRelease();
      setCheck({ phase: 'done', status: describeUpdate(version, latest) });
    } catch (error) {
      setCheck({
        phase: 'error',
        message: error instanceof Error ? error.message : '检查更新失败',
      });
    }
  };

  return (
    <SettingsSectionCard
      kicker="About"
      title="关于与更新"
      description="查看当前版本，并与 GitHub 上的最新正式发布版比对。"
    >
      <div className="space-y-3">
        <div className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
              ℹ
            </span>
            <div>
              <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">当前版本</p>
              <p className="text-xs text-[color:var(--manager-ink-subtle)]">EasyFlowHub</p>
            </div>
          </div>
          <span className="font-mono text-sm text-[color:var(--manager-ink-strong)]">{version}</span>
        </div>

        <div className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
              ↻
            </span>
            <div>
              <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">检查更新</p>
              <p className="text-xs text-[color:var(--manager-ink-subtle)]">对比 GitHub 最新发布版</p>
            </div>
          </div>
          <button
            onClick={() => void handleCheck()}
            disabled={check.phase === 'checking'}
            className="rounded-full border border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] px-4 py-1.5 text-sm text-[color:var(--manager-ink-strong)] transition hover:bg-[color:var(--manager-accent)]/20 disabled:opacity-50"
          >
            {check.phase === 'checking' ? '检查中…' : '检查更新'}
          </button>
        </div>

        <UpdateResult
          check={check}
          onOpenReleases={() => void openUrl(`https://github.com/${RELEASES_REPO}/releases`)}
        />
      </div>
    </SettingsSectionCard>
  );
}

function UpdateResult({
  check,
  onOpenReleases,
}: {
  check: CheckState;
  onOpenReleases: () => void;
}) {
  if (check.phase === 'idle') return null;

  if (check.phase === 'checking') {
    return (
      <p className="rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-3 text-sm text-[color:var(--manager-ink-soft)]">
        正在获取最新版本…
      </p>
    );
  }

  if (check.phase === 'error') {
    return (
      <div
        role="alert"
        className="rounded-[14px] border border-[color:var(--manager-danger)]/40 bg-[color:var(--manager-danger)]/10 px-4 py-3"
      >
        <p className="text-sm text-[color:var(--manager-danger)]">检查更新失败：{check.message}</p>
        <button
          onClick={onOpenReleases}
          className="mt-2 text-xs text-[color:var(--manager-ink-soft)] underline transition hover:text-[color:var(--manager-ink-strong)]"
        >
          打开发布页面
        </button>
      </div>
    );
  }

  const { status } = check;
  if (status.kind === 'up-to-date') {
    return (
      <p className="rounded-[14px] border border-[color:var(--manager-success)]/40 bg-[color:var(--manager-success)]/10 px-4 py-3 text-sm text-[color:var(--manager-ink-soft)]">
        已是最新版本（{status.current}）。
      </p>
    );
  }

  if (status.kind === 'ahead') {
    return (
      <p className="rounded-[14px] border border-[color:var(--manager-warning)]/40 bg-[color:var(--manager-warning)]/10 px-4 py-3 text-sm text-[color:var(--manager-ink-soft)]">
        当前版本（{status.current}）高于最新正式发布版（{status.latest}），可能是内测或本地构建。
      </p>
    );
  }

  return (
    <div className="rounded-[14px] border border-[color:var(--manager-accent)]/40 bg-[color:var(--manager-accent-soft)] px-4 py-3">
      <p className="text-sm text-[color:var(--manager-ink-strong)]">
        发现新版本：{status.latest}
        {status.name ? <span className="text-[color:var(--manager-ink-soft)]">（{status.name}）</span> : null}
      </p>
      <button
        onClick={() => void openUrl(status.url)}
        className="mt-2 rounded-full border border-[color:var(--manager-accent)] px-4 py-1.5 text-xs text-[color:var(--manager-ink-strong)] transition hover:bg-[color:var(--manager-accent)]/20"
      >
        前往下载
      </button>
    </div>
  );
}
