// useTodoEditSession - 待办勾选 / 文本编辑会话的共享用例（Phase C 拆分）。
//
// TodoPanel 与 TodoCardPage 曾各自复制“勾选 -> 写入 -> 失败提示 / 进入编辑 ->
// 保存或取消”的会话状态。现在两个入口消费本 hook。
//
// 不变量（架构审查第 5 节）：
// - 写入编排（文本身份定位 + revision CAS + 冲突上抛）仍归 useTodos（F05），
//   本 hook 只持有编辑会话状态与错误展示，不触碰正文改写；
// - 两个入口的差异通过 keepEditOpenOnFailure 表达：面板保存后总是退出编辑，
//   悬浮卡片写入失败时保留草稿让用户就地重试。

import { useCallback, useState } from 'react';
import type { TodoItem } from '../types/todo';
import type { TodoWriteResult } from './useTodos';

interface UseTodoEditSessionOptions {
  toggleTodo: (todo: TodoItem) => Promise<TodoWriteResult>;
  updateTodoText: (todo: TodoItem, nextText: string) => Promise<TodoWriteResult>;
  /** 写入失败时是否保持编辑态与草稿（默认 false：总是退出编辑）。 */
  keepEditOpenOnFailure?: boolean;
}

export function useTodoEditSession(options: UseTodoEditSessionOptions) {
  const { toggleTodo, updateTodoText, keepEditOpenOnFailure = false } = options;

  const [editingTodoId, setEditingTodoId] = useState<string | null>(null);
  const [todoDraft, setTodoDraft] = useState('');
  const [savingTodoId, setSavingTodoId] = useState<string | null>(null);
  const [writeError, setWriteError] = useState<string | null>(null);

  const clearWriteError = useCallback(() => setWriteError(null), []);

  const cancelEdit = useCallback(() => {
    setEditingTodoId(null);
    setTodoDraft('');
    setSavingTodoId(null);
  }, []);

  const startEdit = useCallback((todo: TodoItem) => {
    setEditingTodoId(todo.id);
    setTodoDraft(todo.text);
    setWriteError(null);
  }, []);

  const isEditing = useCallback((todoId: string) => editingTodoId === todoId, [editingTodoId]);
  const isSaving = useCallback((todoId: string) => savingTodoId === todoId, [savingTodoId]);

  /** 勾选 / 取消勾选；冲突或失败时把可读原因展示给用户。 */
  const toggle = useCallback(
    async (todo: TodoItem): Promise<TodoWriteResult> => {
      setWriteError(null);
      const result = await toggleTodo(todo);
      if (!result.ok && result.message) {
        setWriteError(result.message);
      }
      return result;
    },
    [toggleTodo]
  );

  /** 保存草稿文本；空文本或未改动视为取消。 */
  const saveEdit = useCallback(
    async (todo: TodoItem): Promise<TodoWriteResult | null> => {
      const normalizedText = todoDraft.trim();
      if (!normalizedText || normalizedText === todo.text) {
        cancelEdit();
        return null;
      }

      setSavingTodoId(todo.id);
      setWriteError(null);
      try {
        const result = await updateTodoText(todo, normalizedText);
        if (!result.ok && result.message) {
          setWriteError(result.message);
        }
        if (result.ok || !keepEditOpenOnFailure) {
          cancelEdit();
        }
        return result;
      } finally {
        // cancelEdit 已清零；失败保留编辑态时也要解除输入禁用。
        setSavingTodoId(null);
      }
    },
    [cancelEdit, keepEditOpenOnFailure, todoDraft, updateTodoText]
  );

  return {
    editingTodoId,
    todoDraft,
    setTodoDraft,
    savingTodoId,
    writeError,
    setWriteError,
    clearWriteError,
    startEdit,
    cancelEdit,
    isEditing,
    isSaving,
    toggle,
    saveEdit,
  };
}
