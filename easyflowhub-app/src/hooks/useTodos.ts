// useTodos - 待办用例的唯一编排 owner（F05）。
//
// Manager 面板与悬浮卡片曾各自复制“读笔记 -> 按旧行号改写 -> 整篇保存”的
// 流程，多窗口在上方插入/删除行后旧行号会命中错误条目。现在两个入口都
// 消费本 hook：定位用文本身份（locateTodoLine），写入带 revision CAS
// （expected_updated_at），冲突显式上抛并刷新，而不是静默改错任务。
//
// parser（lib/todoParser）保持纯函数；SQLite 持久化仍归 native notes。

import { useCallback, useEffect, useState } from 'react';
import { createNote, getNote, listNotes, saveNote } from '../lib/tauri/notes';
import { getSettings } from '../lib/tauri/settings';
import {
  appendTodoLine,
  extractAllTodos,
  isWithinRetention,
  locateTodoLine,
  toggleTodoInContent,
  updateTodoTextInContent,
  TODO_INBOX_TITLE,
} from '../lib/todoParser';
import type { Note } from '../types/note';
import type { TodoItem } from '../types/todo';

export type TodoWriteFailure =
  /** 同名待办多于一条，拒绝猜测目标 */
  | 'conflict'
  /** 待办已被其他窗口删除 */
  | 'missing'
  /** CAS 失败：读取后被其他窗口改写 */
  | 'stale'
  | 'unknown';

export interface TodoWriteResult {
  ok: boolean;
  failure?: TodoWriteFailure;
  message?: string;
}

const CONFLICT_MESSAGE = '同名待办不止一条，无法确定目标，请先在笔记中区分它们。';
const MISSING_MESSAGE = '该待办已在其他窗口被删除，列表已刷新。';
const STALE_MESSAGE = '笔记刚被其他窗口修改，本次修改未保存，列表已刷新。';

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'string') {
    return error;
  }
  if (error && typeof error === 'object') {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string') {
      return message;
    }
  }
  return '';
}

function isNoteConflictError(error: unknown): boolean {
  return errorMessage(error).startsWith('note_conflict:');
}

// `createNote` only allocates an id; the title is persisted by a subsequent
// save. Serializing that pair prevents duplicate inbox notes when two todo
// controls in the same WebView initialize at the same time. Cross-window
// uniqueness still belongs in the native store, but this closes the common
// same-context race without changing the public IPC surface.
let todoInboxCreationTail: Promise<void> = Promise.resolve();

async function withTodoInboxCreationLock<T>(task: () => Promise<T>): Promise<T> {
  const previous = todoInboxCreationTail;
  let release!: () => void;
  todoInboxCreationTail = new Promise<void>((resolve) => {
    release = resolve;
  });

  await previous;
  try {
    return await task();
  } finally {
    release();
  }
}

export function useTodos() {
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadTodos = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [notes, settings] = await Promise.all([listNotes(), getSettings()]);
      const hours = settings.todo?.done_retention_hours ?? 24;
      const allTodos = extractAllTodos(
        notes.map((note) => ({ id: note.id, title: note.title, content: note.content }))
      );
      // 只保留未完成 + 保留期内的已完成项
      setTodos(allTodos.filter((todo) => !todo.checked || isWithinRetention(todo.checkedAt, hours)));
    } catch (err) {
      const rawMessage = errorMessage(err);
      const unavailable = /invoke|tauri|undefined/i.test(rawMessage);
      setError(
        unavailable
          ? '待办数据暂不可用，请在桌面应用中启动后端服务。'
          : rawMessage || '加载待办失败',
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadTodos();
  }, [loadTodos]);

  /** 在最新正文中按文本身份定位快照里的待办；失败时刷新并给出可读原因。 */
  const resolveTodoLine = useCallback(
    async (
      todo: TodoItem,
      note: Note,
    ): Promise<{ lineIndex: number } | { failure: TodoWriteFailure; message: string }> => {
      const lookup = locateTodoLine(note.content, { lineIndex: todo.lineIndex, text: todo.text });
      if (lookup.kind === 'exact' || lookup.kind === 'relocated') {
        return { lineIndex: lookup.lineIndex };
      }
      await loadTodos();
      return lookup.kind === 'conflict'
        ? { failure: 'conflict', message: CONFLICT_MESSAGE }
        : { failure: 'missing', message: MISSING_MESSAGE };
    },
    [loadTodos],
  );

  /** CAS 保存：读取 -> 定位 -> 改写 -> 带 revision 写回，冲突显式上抛。 */
  const mutateTodo = useCallback(
    async (
      todo: TodoItem,
      rewrite: (content: string, lineIndex: number) => string | null,
    ): Promise<TodoWriteResult> => {
      try {
        const note = await getNote(todo.noteId);
        if (!note) {
          await loadTodos();
          return { ok: false, failure: 'missing', message: MISSING_MESSAGE };
        }

        const resolved = await resolveTodoLine(todo, note);
        if ('failure' in resolved) {
          return { ok: false, failure: resolved.failure, message: resolved.message };
        }

        const newContent = rewrite(note.content, resolved.lineIndex);
        if (newContent === null) {
          await loadTodos();
          return { ok: false, failure: 'unknown', message: '待办行格式无法识别，列表已刷新。' };
        }

        try {
          await saveNote({
            id: todo.noteId,
            content: newContent,
            expected_updated_at: note.updated_at,
          });
        } catch (saveError) {
          if (isNoteConflictError(saveError)) {
            await loadTodos();
            return { ok: false, failure: 'stale', message: STALE_MESSAGE };
          }
          throw saveError;
        }

        await loadTodos();
        return { ok: true };
      } catch (err) {
        console.error('Failed to update todo:', err);
        return {
          ok: false,
          failure: 'unknown',
          message: errorMessage(err) || '更新待办失败',
        };
      }
    },
    [loadTodos, resolveTodoLine],
  );

  const toggleTodo = useCallback(
    (todo: TodoItem) => mutateTodo(todo, (content, lineIndex) => toggleTodoInContent(content, lineIndex)),
    [mutateTodo],
  );

  const updateTodoText = useCallback(
    (todo: TodoItem, nextText: string) =>
      mutateTodo(todo, (content, lineIndex) => updateTodoTextInContent(content, lineIndex, nextText)),
    [mutateTodo],
  );

  /** 待办箱笔记：唯一归属标题，存在则复用。 */
  const ensureTodoInboxNote = useCallback(async (): Promise<Note> => {
    return withTodoInboxCreationLock(async () => {
      const notes = await listNotes();
      const existing = notes.find((note) => note.title.trim() === TODO_INBOX_TITLE);
      if (existing) {
        return existing;
      }

      const created = await createNote();
      return await saveNote({
        id: created.id,
        title: TODO_INBOX_TITLE,
        content: '',
        tags: created.tags,
      });
    });
  }, []);

  /** 新建待办：追加到待办箱，同样带 revision CAS。 */
  const createTodo = useCallback(
    async (text: string): Promise<TodoWriteResult> => {
      const normalizedText = text.trim();
      if (!normalizedText) {
        return { ok: true };
      }

      try {
        const inbox = await ensureTodoInboxNote();
        // 用最新正文追加，避免覆盖其他窗口刚写入的待办。
        const freshInbox = (await getNote(inbox.id)) ?? inbox;
        const updatedContent = appendTodoLine(freshInbox.content, normalizedText);
        try {
          await saveNote({
            id: freshInbox.id,
            title: freshInbox.title,
            content: updatedContent,
            tags: freshInbox.tags,
            expected_updated_at: freshInbox.updated_at,
          });
        } catch (saveError) {
          if (isNoteConflictError(saveError)) {
            await loadTodos();
            return { ok: false, failure: 'stale', message: STALE_MESSAGE };
          }
          throw saveError;
        }
        await loadTodos();
        return { ok: true };
      } catch (err) {
        console.error('Failed to create todo:', err);
        return {
          ok: false,
          failure: 'unknown',
          message: errorMessage(err) || '新建待办失败',
        };
      }
    },
    [ensureTodoInboxNote, loadTodos],
  );

  return {
    todos,
    loading,
    error,
    loadTodos,
    toggleTodo,
    updateTodoText,
    createTodo,
  };
}
