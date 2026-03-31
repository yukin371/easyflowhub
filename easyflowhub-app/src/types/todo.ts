/**
 * Todo 类型定义
 * 从 Markdown 笔记内容中提取的待办事项
 */

export interface TodoItem {
  /** 唯一标识 (noteId:lineIndex) */
  id: string;
  /** 待办文本 */
  text: string;
  /** 是否已完成 */
  checked: boolean;
  /** 完成时间 (ISO 8601)，未完成为 null */
  checkedAt: string | null;
  /** 所属笔记 ID */
  noteId: string;
  /** 在笔记内容中的行号 (0-based) */
  lineIndex: number;
  /** 笔记标题 (用于聚合视图显示来源) */
  noteTitle: string;
}

export type TodoFilter = 'all' | 'pending' | 'done';
