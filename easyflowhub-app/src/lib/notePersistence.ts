import { isEmptyContent, normalizeStoredTitle, parseNoteContent } from './noteParser';

export interface NotePersistParams {
  noteId: string;
  title: string;
  content: string;
  tags: string;
}

export interface BuildPersistParamsOptions {
  /**
   * Persist an empty body instead of skipping the save.
   *
   * A note that already exists in SQLite needs this: otherwise clearing the
   * editor only clears the textarea, the stale row survives, and the old text
   * comes back the next time the note is opened. A brand-new note leaves this
   * off so autosave cannot litter the list with empty shells before the first
   * keystroke.
   */
  allowEmpty?: boolean;
}

export function buildPersistParams(
  noteId: string | null,
  rawContent: string,
  existingTitle: string = '',
  options: BuildPersistParamsOptions = {}
): NotePersistParams | null {
  if (!noteId) {
    return null;
  }

  const parsed = parseNoteContent(rawContent);
  const title = normalizeStoredTitle(existingTitle);
  if (!options.allowEmpty && isEmptyContent(rawContent) && !title) {
    return null;
  }

  return {
    noteId,
    title,
    content: parsed.cleanContent,
    tags: parsed.tags,
  };
}
