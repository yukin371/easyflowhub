// useTodoGroups - 待办分组展示的共享用例（Phase C 拆分）。
//
// TodoPanel（Manager 面板）与 TodoCardPage（悬浮卡片）曾各自复制“按笔记聚合 +
// 组内排序 + 组间排序 + 待处理/已完成拆分”的 memo 链。现在两个入口消费本
// hook；纯聚合逻辑在 lib/todoParser.groupTodosForDisplay，解析 owner 不变。
//
// 本 hook 只做展示编排：加载 / toggle / edit 的写入编排仍归 useTodos（F05）。

import { useMemo } from 'react';
import { groupTodosForDisplay } from '../lib/todoParser';
import type { TodoGroup, TodoItem } from '../types/todo';

export interface TodoGroupsView {
  /** 未经待处理/已完成拆分的全部分组（组内已排序） */
  groupedTodos: TodoGroup[];
  pendingGroups: TodoGroup[];
  completedGroups: TodoGroup[];
}

export function useTodoGroups(todos: TodoItem[]): TodoGroupsView {
  const groupedTodos = useMemo(() => groupTodosForDisplay(todos), [todos]);

  const pendingGroups = useMemo(
    () =>
      groupedTodos
        .map((group) => ({
          ...group,
          todos: group.todos.filter((todo) => !todo.checked),
        }))
        .filter((group) => group.todos.length > 0),
    [groupedTodos]
  );

  const completedGroups = useMemo(
    () =>
      groupedTodos
        .map((group) => ({
          ...group,
          todos: group.todos.filter((todo) => todo.checked),
        }))
        .filter((group) => group.todos.length > 0),
    [groupedTodos]
  );

  return { groupedTodos, pendingGroups, completedGroups };
}
