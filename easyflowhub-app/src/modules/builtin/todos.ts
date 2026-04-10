/**
 * 待办事项模块定义
 */
import { TodoPanel } from '../../components/manager/todos/TodoPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const todosModule = defineBuiltinModule(
  {
    id: 'todos',
    name: '待办',
    icon: '办',
    caption: 'Todos',
    defaultEnabled: true,
  },
  TodoPanel
);
