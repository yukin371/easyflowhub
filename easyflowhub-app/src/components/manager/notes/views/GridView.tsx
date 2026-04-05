import type { Note } from '../../../../types/note';
import { deriveDisplayTitle, truncateTitle } from '../../../../lib/noteParser';
import { summarizeContentWithoutImageMarkdown } from '../../../../lib/imageAssets';
import { InlineTitleEditor } from './InlineTitleEditor';

interface GridViewProps {
  notes: Note[];
  splitTags: (tagString: string) => string[];
  relativeTimeLabel: (iso: string) => string;
  onOpenNote: (note: Note) => void;
  onRenameNote: (noteId: string, title: string) => void | Promise<void>;
  onTogglePin: (noteId: string, event?: React.MouseEvent<HTMLButtonElement>) => void | Promise<void>;
  onDeleteNote?: (noteId: string) => void | Promise<void>;
}

export function GridView(props: GridViewProps) {
  const { notes, splitTags, relativeTimeLabel, onOpenNote, onRenameNote, onTogglePin, onDeleteNote } = props;

  return (
    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      {notes.map((note) => (
        <article
          key={note.id}
          onClick={() => onOpenNote(note)}
          className="group flex min-h-[228px] cursor-pointer flex-col rounded-[18px] border border-[color:var(--manager-border)] bg-[rgba(255,249,242,0.52)] p-2.5 transition hover:-translate-y-1 hover:border-[color:var(--manager-accent)] hover:shadow-[0_20px_48px_rgba(58,48,33,0.08)]"
        >
          <div className="flex h-full flex-col rounded-[14px] bg-[rgba(252,247,239,0.58)] px-3.5 py-3.5">
            <div className="flex items-start justify-between gap-3">
              <InlineTitleEditor
                title={truncateTitle(deriveDisplayTitle(note.title, note.content), 32) || ''}
                placeholder="无标题"
                onSave={(title) => onRenameNote(note.id, title)}
                onOpen={() => onOpenNote(note)}
                className="min-w-0 flex-1 text-left"
                titleClassName="block truncate pt-1 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-xl leading-[1.32] text-[color:var(--manager-ink-strong)]"
                inputClassName="w-full rounded-[12px] border border-[color:var(--manager-accent)] bg-white px-3 py-2 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-lg text-[color:var(--manager-ink-strong)] outline-none"
              />
              <div className="flex items-center gap-3 pl-4">
                {onDeleteNote && (
                  <button
                    onClick={(event) => {
                      event.stopPropagation();
                      void onDeleteNote(note.id);
                    }}
                    className="text-xs uppercase tracking-[0.22em] text-red-400/60 opacity-0 transition hover:text-red-500 group-hover:opacity-100"
                    title="移到回收站"
                  >
                    删除
                  </button>
                )}
                <button
                  onClick={(event) => {
                    event.stopPropagation();
                    void onTogglePin(note.id);
                  }}
                  className="text-xs uppercase tracking-[0.22em] text-[color:var(--manager-ink-subtle)] transition hover:text-[color:var(--manager-ink-strong)]"
                >
                  {note.is_pinned ? '取消' : '置顶'}
                </button>
              </div>
            </div>
            <div className="mt-3 rounded-[12px] bg-[rgba(248,243,234,0.46)] px-3 py-3">
              <p className="line-clamp-4 text-[14px] leading-7 text-[color:var(--manager-ink-soft)]">
                {summarizeContentWithoutImageMarkdown(note.content) || '留白也会被妥善保存。'}
              </p>
            </div>
            <div className="mt-auto flex items-end justify-between gap-3 pt-4">
              <div className="flex flex-wrap gap-2">
                {splitTags(note.tags)
                  .slice(0, 3)
                  .map((tag) => (
                    <span
                      key={tag}
                      className="rounded-full bg-[color:var(--manager-accent-soft)] px-3 py-1 text-[11px] tracking-[0.16em] text-[color:var(--manager-ink-soft)]"
                    >
                      #{tag}
                    </span>
                  ))}
              </div>
              <span className="shrink-0 text-xs uppercase tracking-[0.22em] text-[color:var(--manager-ink-subtle)]">
                {relativeTimeLabel(note.updated_at)}
              </span>
            </div>
          </div>
        </article>
      ))}
    </div>
  );
}
