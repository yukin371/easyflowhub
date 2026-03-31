/**
 * 回收站模块定义
 */
import type { FeatureModule } from '../types';
import { TrashPanel } from '../../components/manager/TrashPanel';

export const trashModule: FeatureModule = {
  id: 'trash',
  name: '回收站',
  icon: '藏',
  caption: 'Archive',
  defaultEnabled: false,
  component: TrashPanel,
};
