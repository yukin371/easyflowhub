/**
 * 执行记录模块定义
 */
import { TasksPanel } from '../../components/manager/tasks/TasksPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const tasksModule = defineBuiltinModule(
  {
    id: 'tasks',
    name: '执行',
    icon: '行',
    caption: 'Tasks',
    defaultEnabled: false,
  },
  TasksPanel
);
