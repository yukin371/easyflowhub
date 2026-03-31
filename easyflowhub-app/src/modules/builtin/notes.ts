/**
 * 笔记模块定义
 */
import type { FeatureModule } from '../types';
import { NotesPanel } from '../../components/manager/NotesPanel';

export const notesModule: FeatureModule = {
  id: 'notes',
  name: '笔记',
  icon: '墨',
  caption: 'Notes',
  defaultEnabled: true,
  isCore: true, // 核心模块，不可关闭
  component: NotesPanel,
};
