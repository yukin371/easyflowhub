import { DiffView } from './DiffView';
import { diffText } from '../../lib/utils/diff';

interface BackupRestoreDialogProps {
  currentTitle: string;
  currentContent: string;
  currentTags: string;
  backupTitle: string;
  backupContent: string;
  backupTags: string;
  backupTimestamp?: number;
  onKeepCurrent: () => void;
  onRestoreBackup: () => void;
  onMergeWithBackup: () => void;
}

export function BackupRestoreDialog(props: BackupRestoreDialogProps) {
  const {
    currentTitle,
    currentContent,
    currentTags,
    backupTitle,
    backupContent,
    backupTags,
    backupTimestamp,
    onKeepCurrent,
    onRestoreBackup,
    onMergeWithBackup,
  } = props;

  const currentText = [currentTitle, currentContent, currentTags ? `# ${currentTags}` : '']
    .filter(Boolean)
    .join('\n');
  const backupText = [backupTitle, backupContent, backupTags ? `# ${backupTags}` : '']
    .filter(Boolean)
    .join('\n');
  const diff = diffText(currentText, backupText);

  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-[rgba(45,36,24,0.28)] px-6 py-8 backdrop-blur-sm">
      <div className="flex max-h-full w-full max-w-5xl flex-col overflow-hidden rounded-[32px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] shadow-[0_28px_80px_rgba(45,36,24,0.16)]">
        <div className="border-b border-[color:var(--manager-border)] px-6 py-5">
          <p className="manager-kicker">Backup Restore</p>
          <div className="mt-3 flex flex-col gap-2 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <h3 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-3xl text-[color:var(--manager-ink-strong)]">
                检测到未恢复的本地备份
              </h3>
              <p className="mt-2 text-sm leading-7 text-[color:var(--manager-ink-soft)]">
                {backupTimestamp
                  ? `备份时间：${new Date(backupTimestamp).toLocaleString('zh-CN')}`
                  : '检测到本地备份，建议比较后再决定采用哪一份内容。'}
              </p>
            </div>
            <div className="text-sm text-[color:var(--manager-ink-soft)]">
              {diff.hasChanges ? `${diff.changes.length} 处差异` : '无差异'}
            </div>
          </div>
        </div>

        <div className="flex-1 overflow-auto px-6 py-5">
          <div className="grid gap-5 xl:grid-cols-2">
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/58 p-5">
              <p className="manager-kicker">Current</p>
              <h4 className="mt-3 text-xl text-[color:var(--manager-ink-strong)]">{currentTitle || '无标题'}</h4>
              <p className="mt-3 whitespace-pre-wrap text-sm leading-7 text-[color:var(--manager-ink-soft)]">
                {currentContent || '无正文'}
              </p>
              <div className="mt-4 flex flex-wrap gap-2">
                {currentTags ? currentTags.split(/\s+/).filter(Boolean).map((tag) => (
                  <span key={tag} className="rounded-full bg-[color:var(--manager-accent-soft)] px-3 py-1 text-xs tracking-[0.14em] text-[color:var(--manager-ink-soft)]">
                    #{tag}
                  </span>
                )) : <span className="text-sm text-[color:var(--manager-ink-subtle)]">暂无标签</span>}
              </div>
            </section>

            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/72 p-5">
              <p className="manager-kicker">Backup</p>
              <h4 className="mt-3 text-xl text-[color:var(--manager-ink-strong)]">{backupTitle || '无标题'}</h4>
              <p className="mt-3 whitespace-pre-wrap text-sm leading-7 text-[color:var(--manager-ink-soft)]">
                {backupContent || '无正文'}
              </p>
              <div className="mt-4 flex flex-wrap gap-2">
                {backupTags ? backupTags.split(/\s+/).filter(Boolean).map((tag) => (
                  <span key={tag} className="rounded-full bg-[rgba(203,163,92,0.12)] px-3 py-1 text-xs tracking-[0.14em] text-[color:var(--manager-ink-soft)]">
                    #{tag}
                  </span>
                )) : <span className="text-sm text-[color:var(--manager-ink-subtle)]">暂无标签</span>}
              </div>
            </section>
          </div>

          <div className="mt-5">
            <DiffView diff={diff} />
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-end gap-3 border-t border-[color:var(--manager-border)] px-6 py-5">
          <button
            onClick={onKeepCurrent}
            className="rounded-full border border-[color:var(--manager-border)] bg-white/70 px-5 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
          >
            使用当前版本
          </button>
          <button
            onClick={onMergeWithBackup}
            className="rounded-full border border-[color:var(--manager-border)] bg-[rgba(203,163,92,0.12)] px-5 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
          >
            合并到当前内容
          </button>
          <button
            onClick={onRestoreBackup}
            className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
          >
            恢复备份版本
          </button>
        </div>
      </div>
    </div>
  );
}
