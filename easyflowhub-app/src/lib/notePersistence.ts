import { isEmptyContent, normalizeStoredTitle, parseNoteContent } from './noteParser';

export interface NotePersistParams {
  noteId: string;
  title: string;
  content: string;
  tags: string;
}

export function buildPersistParams(
  noteId: string | null,
  rawContent: string,
  existingTitle: string = ''
): NotePersistParams | null {
  if (!noteId) {
    return null;
  }

  const parsed = parseNoteContent(rawContent);
  const title = normalizeStoredTitle(existingTitle);
  if (isEmptyContent(rawContent) && !title) {
    return null;
  }

  return {
    noteId,
    title,
    content: parsed.cleanContent,
    tags: parsed.tags,
  };
}
