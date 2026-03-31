import { isEmptyContent, parseNoteContent } from './noteParser';

export interface NotePersistParams {
  noteId: string;
  title: string;
  content: string;
  tags: string;
}

export function buildPersistParams(
  noteId: string | null,
  rawContent: string
): NotePersistParams | null {
  if (!noteId || isEmptyContent(rawContent)) {
    return null;
  }

  const parsed = parseNoteContent(rawContent);
  return {
    noteId,
    title: parsed.title,
    content: parsed.cleanContent,
    tags: parsed.tags,
  };
}
