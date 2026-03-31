import type { Note } from '../../../../types/note';
import { deriveDisplayTitle, truncateTitle } from '../../../../lib/noteParser';
import { InlineTitleEditor } from './InlineTitleEditor';

interface ListViewProps {
  notes: Note[];
  splitTags: (tagString: string) => string[];
  relativeTimeLabel: (iso: string) => string;
  onOpenNote: (note: Note) => void;
  onRenameNote: (noteId: string, title: string) => void | Promise<void>;
  onTogglePin: (noteId: string, event?: React.MouseEvent<HTMLButtonElement>) => void | Promise<void>;
  onDeleteNote: (noteId: string) => void | Promise<void>;
}

export function ListView(props: ListViewProps) {
  const { notes, splitTags, relativeTimeLabel, onOpenNote, onRenameNote, onTogglePin, onDeleteNote } = props;

  return (
    <div className="space-y-4 pt-1">
      {notes.map((note) => (
        <article
          key={note.id}
          onClick={() => onOpenNote(note)}
          className="group rounded-[22px] border border-[color:var(--manager-border)] bg-white/68 p-5 transition hover:-translate-y-0.5 hover:border-[color:var(--manager-accent)] hover:shadow-[0_24px_50px_rgba(58,48,33,0.08)]"
        >
          <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-3">
                <button
                  onClick={(event) => void onTogglePin(note.id, event)}
                  className={`rounded-full border px-3 py-1 text-xs tracking-[0.2em] transition ${
                    note.is_pinned
                      ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                      : 'border-[color:var(--manager-border)] text-[color:var(--manager-ink-subtle)] hover:border-[color:var(--manager-accent)]'
                  }`}
                >
                  {note.is_pinned ? '置顶' : '收纳'}
                </button>
                <InlineTitleEditor
                  title={truncateTitle(deriveDisplayTitle(note.title, note.content), 40) || ''}
                  placeholder="无标题"
                  onSave={(title) => onRenameNote(note.id, title)}
                  onOpen={() => onOpenNote(note)}
                  className="min-w-0 flex-1 text-left"
                  titleClassName="block truncate pt-1 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-2xl leading-[1.28] text-[color:var(--manager-ink-strong)]"
                  inputClassName="w-full rounded-[12px] border border-[color:var(--manager-accent)] bg-white px-3 py-2 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-xl text-[color:var(--manager-ink-strong)] outline-none"
                />
              </div>
              <p className="mt-4 line-clamp-2 text-sm leading-7 text-[color:var(--manager-ink-soft)]">
                {note.content || '这是一条空白笔记，点击后进入全屏编辑。'}
              </p>
              <div className="mt-4 flex flex-wrap items-center gap-2">
                {splitTags(note.tags).map((tag) => (
                  <span
                    key={tag}
                    className="rounded-full bg-[color:var(--manager-accent-soft)] px-3 py-1 text-xs tracking-[0.14em] text-[color:var(--manager-ink-soft)]"
                  >
                    #{tag}
                  </span>
                ))}
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-3 text-sm text-[color:var(--manager-ink-subtle)]">
              <span>{relativeTimeLabel(note.updated_at)}</span>
              <button
                onClick={(event) => {
                  event.stopPropagation();
                  void onDeleteNote(note.id);
                }}
                className="rounded-full border border-transparent px-3 py-1 transition hover:border-[color:var(--manager-danger)] hover:text-[color:var(--manager-danger)]"
              >
                删除
              </button>
            </div>
          </div>
        </article>
      ))}
    </div>
  );
}
