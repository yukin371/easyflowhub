import { describe, expect, it } from 'vitest';
import { matchesSearchQuery } from './noteSearch';
import type { Note } from '../types/note';

function createNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'note-1',
    title: 'Release checklist',
    content: 'ship the docs\n#urgent',
    tags: 'work docs',
    created_at: '2026-10-19T00:00:00.000Z',
    updated_at: '2026-10-19T00:00:00.000Z',
    is_pinned: false,
    deleted_at: null,
    ...overrides,
  };
}

describe('matchesSearchQuery', () => {
  it('matches free text across title, content and tags', () => {
    const note = createNote();
    expect(matchesSearchQuery(note, 'release')).toBe(true);
    expect(matchesSearchQuery(note, 'ship')).toBe(true);
    expect(matchesSearchQuery(note, 'work')).toBe(true);
    expect(matchesSearchQuery(note, 'missing')).toBe(false);
  });

  it('supports title: / content: / tag: prefixes', () => {
    const note = createNote();
    expect(matchesSearchQuery(note, 'title:checklist')).toBe(true);
    expect(matchesSearchQuery(note, 't:checklist')).toBe(true);
    expect(matchesSearchQuery(note, 'title:docs')).toBe(false);
    expect(matchesSearchQuery(note, 'content:ship')).toBe(true);
    expect(matchesSearchQuery(note, 'c:ship')).toBe(true);
    expect(matchesSearchQuery(note, 'body:ship')).toBe(true);
    expect(matchesSearchQuery(note, 'tag:work')).toBe(true);
    expect(matchesSearchQuery(note, '#work')).toBe(true);
    expect(matchesSearchQuery(note, 'tag:other')).toBe(false);
  });

  it('requires every whitespace-separated token to match', () => {
    const note = createNote();
    expect(matchesSearchQuery(note, 'release ship')).toBe(true);
    expect(matchesSearchQuery(note, 'release missing')).toBe(false);
    expect(matchesSearchQuery(note, '  ')).toBe(true);
  });
});
