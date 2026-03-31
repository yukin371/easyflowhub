/**
 * 待办事项模块定义
 */
import type { FeatureModule } from '../types';
import { TodoPanel } from '../../components/manager/todos/TodoPanel';

export const todosModule: FeatureModule = {
  id: 'todos',
  name: '待办',
  icon: '办',
  caption: 'Todos',
  defaultEnabled: true,
  component: TodoPanel,
};
