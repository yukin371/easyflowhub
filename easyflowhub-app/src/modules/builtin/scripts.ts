/**
 * 脚本模块定义
 */
import type { FeatureModule } from '../types';
import { ScriptsPanel } from '../../components/manager/scripts/ScriptsPanel';

export const scriptsModule: FeatureModule = {
  id: 'scripts',
  name: '脚本',
  icon: '策',
  caption: 'Scripts',
  defaultEnabled: false,
  component: ScriptsPanel,
};
