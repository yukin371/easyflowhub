/**
 * 回收站模块定义
 */
import { TrashPanel } from '../../components/manager/TrashPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const trashModule = defineBuiltinModule(
  {
    id: 'trash',
    name: '回收站',
    icon: '藏',
    caption: 'Archive',
    defaultEnabled: false,
  },
  TrashPanel
);
