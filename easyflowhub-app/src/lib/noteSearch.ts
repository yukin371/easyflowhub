import type { Note } from '../types/note';
import { splitTags } from './notesUtils';
import { deriveDisplayTitle } from './noteParser';

/**
 * 笔记归档搜索查询的 token 匹配（title: / content: / tag: 前缀 + 全文）。
 * 纯函数，无状态；面板只负责 query 字符串的状态。
 */
function matchesQueryToken(note: Note, token: string): boolean {
  const normalizedToken = token.trim().toLowerCase();
  if (!normalizedToken) {
    return true;
  }

  const noteTags = splitTags(note.tags).map((tag) => tag.toLowerCase());
  const title = deriveDisplayTitle(note.title, note.content).toLowerCase();
  const content = note.content.toLowerCase();

  if (normalizedToken.startsWith('tag:') || normalizedToken.startsWith('#')) {
    const value = normalizedToken.replace(/^tag:/, '').replace(/^#/, '');
    return value ? noteTags.some((tag) => tag.includes(value)) : true;
  }

  if (normalizedToken.startsWith('title:') || normalizedToken.startsWith('t:')) {
    const value = normalizedToken.replace(/^title:/, '').replace(/^t:/, '');
    return value ? title.includes(value) : true;
  }

  if (
    normalizedToken.startsWith('content:') ||
    normalizedToken.startsWith('c:') ||
    normalizedToken.startsWith('body:')
  ) {
    const value = normalizedToken
      .replace(/^content:/, '')
      .replace(/^c:/, '')
      .replace(/^body:/, '');
    return value ? content.includes(value) : true;
  }

  return (
    title.includes(normalizedToken) ||
    content.includes(normalizedToken) ||
    noteTags.some((tag) => tag.includes(normalizedToken))
  );
}

export function matchesSearchQuery(note: Note, query: string): boolean {
  const tokens = query
    .split(/\s+/)
    .map((token) => token.trim())
    .filter(Boolean);

  if (tokens.length === 0) {
    return true;
  }

  return tokens.every((token) => matchesQueryToken(note, token));
}
