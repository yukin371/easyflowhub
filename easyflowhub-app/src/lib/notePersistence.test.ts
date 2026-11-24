import { describe, expect, it } from 'vitest';
import { buildPersistParams } from './notePersistence';

describe('buildPersistParams', () => {
  it('returns null without a note id', () => {
    expect(buildPersistParams(null, 'text')).toBeNull();
  });

  it('skips an empty brand-new note so autosave leaves no shell', () => {
    expect(buildPersistParams('n1', '')).toBeNull();
    expect(buildPersistParams('n1', '   \n\t ')).toBeNull();
  });

  it('persists an empty body when the note already exists', () => {
    // Regression: clearing the editor used to be dropped here, which left the
    // stale row in SQLite and made the old text reappear on reopen.
    const params = buildPersistParams('n1', '', '', { allowEmpty: true });
    expect(params).toEqual({ noteId: 'n1', title: '', content: '', tags: '' });
  });

  it('keeps non-empty content untouched regardless of allowEmpty', () => {
    expect(buildPersistParams('n1', 'hello')?.content).toBe('hello');
    expect(buildPersistParams('n1', 'hello', '', { allowEmpty: true })?.content).toBe('hello');
  });

  it('still persists tags-only content', () => {
    const params = buildPersistParams('n1', '#work');
    expect(params).not.toBeNull();
    expect(params?.tags).toContain('work');
  });
});
