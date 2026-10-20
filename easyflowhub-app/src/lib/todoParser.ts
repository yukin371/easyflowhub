/**
 * TodoParser - 从 Markdown 内容中提取 checkbox 待办事项
 * 支持 - [ ], * [ ], 1. [ ], 1. [x] 语法
 * 完成项可带 @done:ISO-timestamp 标记
 */

import type { TodoGroup, TodoItem } from '../types/todo';
import { deriveDisplayTitle } from './noteParser';

/** 匹配 markdown checkbox 行 (支持无序和有序列表符号) */
const TODO_REGEX = /^(\s*)((?:[-*])|(?:\d+\.))\s*\[([ xX])\]\s*(.+)$/;
/** 匹配行尾的 @done:ISO-timestamp */
const DONE_TIMESTAMP_REGEX = /\s*@done:(\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:?\d{2})?)\s*$/;

export const TODO_INBOX_TITLE = '待办箱';

/**
 * 从单个笔记中提取所有 todo 项
 */
export function extractTodos(content: string, noteId: string, noteTitle: string): TodoItem[] {
  const lines = content.split('\n');
  const todos: TodoItem[] = [];

  for (let i = 0; i < lines.length; i++) {
    const match = TODO_REGEX.exec(lines[i]);
    if (match) {
      const checked = match[3].toLowerCase() === 'x';
      let text = match[4].trim();
      let checkedAt: string | null = null;

      // 从已完成项中提取 @done:timestamp
      if (checked) {
        const tsMatch = DONE_TIMESTAMP_REGEX.exec(text);
        if (tsMatch) {
          checkedAt = tsMatch[1];
          text = text.slice(0, tsMatch.index).trim();
        }
      }

      if (text) {
        todos.push({
          id: `${noteId}:${i}`,
          text,
          checked,
          checkedAt,
          noteId,
          lineIndex: i,
          noteTitle,
        });
      }
    }
  }

  return todos;
}

/**
 * 从多个笔记中提取所有 todo 项
 */
export function extractAllTodos(
  notes: Array<{ id: string; title: string; content: string }>
): TodoItem[] {
  return notes.flatMap((note) =>
    extractTodos(note.content, note.id, deriveDisplayTitle(note.title, note.content) || '无标题')
  );
}

/**
 * 切换笔记内容中某行的 checkbox 状态
 * - 打勾时自动附加 @done:timestamp
 * - 取消打勾时移除 @done:timestamp
 * 返回更新后的内容，如果行不存在或不是 checkbox 则返回 null
 */
export function toggleTodoInContent(content: string, lineIndex: number): string | null {
  const lines = content.split('\n');
  if (lineIndex < 0 || lineIndex >= lines.length) return null;

  const match = TODO_REGEX.exec(lines[lineIndex]);
  if (!match) return null;

  const indent = match[1];
  const marker = match[2];
  const currentChecked = match[3].toLowerCase() === 'x';
  let text = match[4];

  const newChecked = !currentChecked;

  if (newChecked) {
    // 打勾：附加 @done:timestamp
    // 先移除可能已有的旧 timestamp
    text = text.replace(DONE_TIMESTAMP_REGEX, '').trim();
    const timestamp = new Date().toISOString();
    lines[lineIndex] = `${indent}${marker} [x] ${text} @done:${timestamp}`;
  } else {
    // 取消打勾：移除 @done:timestamp
    text = text.replace(DONE_TIMESTAMP_REGEX, '').trim();
    lines[lineIndex] = `${indent}${marker} [ ] ${text}`;
  }

  return lines.join('\n');
}

/**
 * 更新指定 todo 行的文本内容
 * 保留原有的列表符号、完成状态和 @done 时间戳
 */
export function updateTodoTextInContent(
  content: string,
  lineIndex: number,
  nextText: string
): string | null {
  const lines = content.split('\n');
  if (lineIndex < 0 || lineIndex >= lines.length) return null;

  const match = TODO_REGEX.exec(lines[lineIndex]);
  if (!match) return null;

  const normalizedText = nextText.trim();
  if (!normalizedText) return null;

  const indent = match[1];
  const marker = match[2];
  const checked = match[3].toLowerCase() === 'x';
  const text = match[4];
  const checkedAt = checked ? DONE_TIMESTAMP_REGEX.exec(text)?.[1] ?? null : null;
  const doneSuffix = checkedAt ? ` @done:${checkedAt}` : '';

  lines[lineIndex] = `${indent}${marker} [${checked ? 'x' : ' '}] ${normalizedText}${doneSuffix}`;
  return lines.join('\n');
}

/** 待办在正文中的身份引用：行号只是提示，文本才是定位依据。 */
export interface TodoLineRef {
  /** 列表快照时的行号（0-based），可能已过期 */
  lineIndex: number;
  /** 待办文本（不含 @done 后缀） */
  text: string;
}

export type TodoLineLookup =
  /** 原行号仍然指向同名待办 */
  | { kind: 'exact'; lineIndex: number }
  /** 行号已失效，但全文唯一匹配到同名待办 */
  | { kind: 'relocated'; lineIndex: number }
  /** 同名待办出现多次，无法确定目标，调用方必须报冲突而不是猜 */
  | { kind: 'conflict' }
  /** 待办已被其他窗口删除 */
  | { kind: 'missing' };

/** 解析单行 checkbox；不是待办行则返回 null。 */
function parseTodoLine(line: string): { checked: boolean; text: string } | null {
  const match = TODO_REGEX.exec(line);
  if (!match) {
    return null;
  }
  const checked = match[3].toLowerCase() === 'x';
  let text = match[4].trim();
  if (checked) {
    text = text.replace(DONE_TIMESTAMP_REGEX, '').trim();
  }
  return { checked, text };
}

/**
 * 在“最新正文”中定位列表快照里的待办（F05）。
 *
 * 快照里的 lineIndex 来自另一次读取：多窗口在上方插入/删除行后，直接用
 * 旧行号改写正文会命中错误的条目。这里先验证原行号是否仍指向同名待办，
 * 失败则按文本在全文中重新定位；同名待办多于一条时返回 conflict，宁可
 * 拒绝写入也不静默改错任务。
 */
export function locateTodoLine(content: string, ref: TodoLineRef): TodoLineLookup {
  const lines = content.split('\n');

  const candidates: number[] = [];
  for (let i = 0; i < lines.length; i++) {
    const parsed = parseTodoLine(lines[i]);
    if (parsed && parsed.text === ref.text) {
      candidates.push(i);
    }
  }

  if (candidates.length === 0) {
    return { kind: 'missing' };
  }
  if (candidates.length > 1) {
    // 同名待办多于一条：即使快照行号仍落在其中一条上，也无法证明它没
    // 被其他窗口的插入/删除顶替成另一条同名待办，宁可拒绝也不静默改错。
    return { kind: 'conflict' };
  }

  const lineIndex = candidates[0];
  return lineIndex === ref.lineIndex
    ? { kind: 'exact', lineIndex }
    : { kind: 'relocated', lineIndex };
}

/** 在正文末尾追加一条未完成待办；空正文直接生成首行。 */
export function appendTodoLine(content: string, text: string): string {
  const normalizedText = text.trim();
  if (!normalizedText) {
    return content;
  }

  if (!content.trim()) {
    return `- [ ] ${normalizedText}`;
  }

  return `${content.replace(/\s*$/, '')}\n- [ ] ${normalizedText}`;
}

/**
 * 检查已完成的 todo 是否在保留期内
 * @param checkedAt ISO 时间戳字符串
 * @param retentionHours 保留小时数
 * @returns true = 还在保留期内，应显示删除线
 */
export function isWithinRetention(checkedAt: string | null, retentionHours: number): boolean {
  if (!checkedAt) return true; // 无时间戳的完成项默认显示
  const doneTime = new Date(checkedAt).getTime();
  const now = Date.now();
  return now - doneTime < retentionHours * 3600_000;
}

/**
 * 统计 todo 完成情况
 */
export function getTodoStats(todos: TodoItem[]): { total: number; done: number; pending: number } {
  const done = todos.filter((t) => t.checked).length;
  return {
    total: todos.length,
    done,
    pending: todos.length - done,
  };
}

/** 按笔记聚合待办用于分组展示：组内未完成优先 + 行号排序，组间待办箱优先。 */
export function groupTodosForDisplay(todos: TodoItem[]): TodoGroup[] {
  const groups = new Map<string, TodoGroup>();
  for (const todo of todos) {
    const existing = groups.get(todo.noteId);
    if (existing) {
      existing.todos.push(todo);
      continue;
    }

    groups.set(todo.noteId, {
      noteId: todo.noteId,
      noteTitle: todo.noteTitle,
      todos: [todo],
    });
  }

  return Array.from(groups.values())
    .map((group) => ({
      ...group,
      todos: sortTodosForDisplay(group.todos),
    }))
    .sort(compareTodoGroupsForDisplay);
}

export function sortTodosForDisplay(todos: TodoItem[]): TodoItem[] {
  return [...todos].sort((a, b) => {
    if (a.checked !== b.checked) {
      return a.checked ? 1 : -1;
    }

    return a.lineIndex - b.lineIndex;
  });
}

export function compareTodoGroupsForDisplay<
  T extends { noteId: string; noteTitle: string; todos: TodoItem[] }
>(a: T, b: T): number {
  const aIsInbox = a.noteTitle.trim() === TODO_INBOX_TITLE;
  const bIsInbox = b.noteTitle.trim() === TODO_INBOX_TITLE;
  if (aIsInbox !== bIsInbox) {
    return aIsInbox ? -1 : 1;
  }

  const aHasPending = a.todos.some((todo) => !todo.checked);
  const bHasPending = b.todos.some((todo) => !todo.checked);
  if (aHasPending !== bHasPending) {
    return aHasPending ? -1 : 1;
  }

  const titleCompare = a.noteTitle.localeCompare(b.noteTitle, 'zh-CN');
  if (titleCompare !== 0) {
    return titleCompare;
  }

  return a.noteId.localeCompare(b.noteId);
}
