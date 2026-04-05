import { describe, expect, it } from 'vitest';
import { getMarkdownCompletionUpdate } from './markdownEditing';

describe('markdownEditing', () => {
  it('continues unordered todo lists', () => {
    const update = getMarkdownCompletionUpdate({
      key: 'Enter',
      value: '- [ ] first task',
      selectionStart: 16,
      selectionEnd: 16,
    });

    expect(update).toEqual({
      value: '- [ ] first task\n- [ ] ',
      selectionStart: 23,
      selectionEnd: 23,
    });
  });

  it('continues ordered todo lists', () => {
    const update = getMarkdownCompletionUpdate({
      key: 'Enter',
      value: '1. [ ] first task',
      selectionStart: 17,
      selectionEnd: 17,
    });

    expect(update).toEqual({
      value: '1. [ ] first task\n2. [ ] ',
      selectionStart: 25,
      selectionEnd: 25,
    });
  });

  it('stops an empty list item instead of nesting forever', () => {
    const update = getMarkdownCompletionUpdate({
      key: 'Enter',
      value: 'first line\n- [ ] ',
      selectionStart: 16,
      selectionEnd: 16,
    });

    expect(update).toEqual({
      value: 'first line\n',
      selectionStart: 11,
      selectionEnd: 11,
    });
  });

  it('auto completes todo checkbox marker for ordered lists', () => {
    const update = getMarkdownCompletionUpdate({
      key: '[',
      value: '1. ',
      selectionStart: 3,
      selectionEnd: 3,
    });

    expect(update).toEqual({
      value: '1. [ ] ',
      selectionStart: 7,
      selectionEnd: 7,
    });
  });

  it('skips an existing closing bracket', () => {
    const update = getMarkdownCompletionUpdate({
      key: ']',
      value: '[]',
      selectionStart: 1,
      selectionEnd: 1,
    });

    expect(update).toEqual({
      value: '[]',
      selectionStart: 2,
      selectionEnd: 2,
    });
  });
});
