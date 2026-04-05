/**
 * TodoParser - 从 Markdown 内容中提取 checkbox 待办事项
 * 支持 - [ ], * [ ], 1. [ ], 1. [x] 语法
 * 完成项可带 @done:ISO-timestamp 标记
 */

import type { TodoItem } from '../types/todo';
import { deriveDisplayTitle } from './noteParser';

/** 匹配 markdown checkbox 行 (支持无序和有序列表符号) */
const TODO_REGEX = /^(\s*)((?:[-*])|(?:\d+\.))\s*\[([ xX])\]\s*(.+)$/;
/** 匹配行尾的 @done:ISO-timestamp */
const DONE_TIMESTAMP_REGEX = /\s*@done:(\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:?\d{2})?)\s*$/;

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
