/**
 * 笔记模块定义
 */
import { NotesPanel } from '../../components/manager/NotesPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const notesModule = defineBuiltinModule(
  {
    id: 'notes',
    name: '笔记',
    icon: '墨',
    caption: 'Notes',
    defaultEnabled: true,
    isCore: true,
  },
  NotesPanel
);
