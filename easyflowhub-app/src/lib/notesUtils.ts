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
