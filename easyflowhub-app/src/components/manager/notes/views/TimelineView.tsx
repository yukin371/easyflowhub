import type { Note } from '../../../../types/note';
import { deriveDisplayTitle, truncateTitle } from '../../../../lib/noteParser';
import { InlineTitleEditor } from './InlineTitleEditor';

interface TimelineViewProps {
  groups: Array<[string, Note[]]>;
  onOpenNote: (note: Note) => void;
  onRenameNote: (noteId: string, title: string) => void | Promise<void>;
}

export function TimelineView({ groups, onOpenNote, onRenameNote }: TimelineViewProps) {
  return (
    <div className="space-y-8 px-2 pt-2">
      {groups.map(([bucket, bucketNotes]) => (
        <section key={bucket}>
          <div className="mb-4 flex items-center gap-4">
            <span className="manager-kicker">{bucket}</span>
            <div className="h-px flex-1 bg-[color:var(--manager-border)]" />
          </div>
          <div className="space-y-3">
            {bucketNotes.map((note) => (
              <article
                key={note.id}
                onClick={() => onOpenNote(note)}
                className="relative cursor-pointer rounded-[22px] border border-[color:var(--manager-border)] bg-white/68 p-5 pl-8 transition hover:border-[color:var(--manager-accent)] hover:shadow-[0_24px_50px_rgba(58,48,33,0.08)]"
              >
                <span className="absolute left-[-8px] top-8 h-4 w-4 rounded-full border-4 border-[color:var(--manager-bg)] bg-[color:var(--manager-accent)]" />
                <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                  <div>
                    <InlineTitleEditor
                      title={truncateTitle(deriveDisplayTitle(note.title, note.content), 40) || ''}
                      placeholder="无标题"
                      onSave={(title) => onRenameNote(note.id, title)}
                      onOpen={() => onOpenNote(note)}
                      className="text-left"
                      titleClassName="block pt-1 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-2xl leading-[1.28] text-[color:var(--manager-ink-strong)]"
                      inputClassName="w-full rounded-[12px] border border-[color:var(--manager-accent)] bg-white px-3 py-2 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-xl text-[color:var(--manager-ink-strong)] outline-none"
                    />
                    <p className="mt-2 line-clamp-2 text-sm leading-7 text-[color:var(--manager-ink-soft)]">
                      {note.content || '无正文'}
                    </p>
                  </div>
                  <div className="text-sm text-[color:var(--manager-ink-subtle)]">
                    {new Date(note.updated_at).toLocaleString('zh-CN')}
                  </div>
                </div>
              </article>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
