import type { Note } from '../types/note';
import { deriveDisplayTitle } from './noteParser';

export function splitTags(tagString: string): string[] {
  return tagString
    .split(/[\s,，]+/)
    .map((tag) => tag.replace(/^#+/, '').trim())
    .filter(Boolean);
}

export function normalizeTags(tags: string): string {
  return Array.from(new Set(splitTags(tags))).join(' ');
}

export function getBackupKey(noteId: string): string {
  return `easyflowhub_note_autosave_${noteId}`;
}

export function relativeTimeLabel(iso: string): string {
  const time = new Date(iso).getTime();
  const diff = Date.now() - time;
  const minute = 60 * 1000;
  const hour = 60 * minute;
  const day = 24 * hour;

  if (diff < minute) return '刚刚';
  if (diff < hour) return `${Math.floor(diff / minute)} 分钟前`;
  if (diff < day) return `${Math.floor(diff / hour)} 小时前`;
  if (diff < day * 7) return `${Math.floor(diff / day)} 天前`;
  return new Date(iso).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' });
}

/** 同一笔记可能在最近列表中重复出现，按 id 去重（保留先出现的条目）。 */
export function dedupeNotesById(notes: Note[]): Note[] {
  const map = new Map<string, Note>();
  for (const note of notes) {
    map.set(note.id, note);
  }
  return Array.from(map.values());
}

/** 有正文或标签的笔记才算可“继续编辑”。 */
export function isNoteContinueCandidate(note: Note): boolean {
  return Boolean(note.content.trim() || note.tags.trim());
}

/** 快速笔记切换器里的两行标签：标题 + 首个非标题片段。 */
export function buildNoteSwitchLabel(note: Note): string {
  const title = deriveDisplayTitle(note.title, note.content) || '无标题';
  const snippet = note.content
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .find((line) => line !== title);

  return snippet ? `${title} · ${snippet}`.slice(0, 28) : title.slice(0, 28);
}

/** 切换器的简单过滤：标题 / 正文 / 标签任一命中即可。 */
export function filterNotesByQuery(notes: Note[], rawQuery: string): Note[] {
  const query = rawQuery.trim().toLowerCase();
  const uniqueNotes = dedupeNotesById(notes);
  if (!query) {
    return uniqueNotes;
  }

  return uniqueNotes.filter((item) => {
    const title = deriveDisplayTitle(item.title, item.content).toLowerCase();
    const content = item.content.toLowerCase();
    const tags = item.tags.toLowerCase();
    return title.includes(query) || content.includes(query) || tags.includes(query);
  });
}

export function timelineBucket(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const target = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const diffDays = Math.round((today - target) / (24 * 60 * 60 * 1000));

  if (diffDays <= 0) return '今天';
  if (diffDays === 1) return '昨天';
  if (diffDays < 7) return '本周更早';
  return '更早';
}
