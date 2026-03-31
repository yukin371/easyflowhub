/**
 * 设置模块定义
 */
import type { FeatureModule } from '../types';
import { SettingsPanel } from '../../components/manager/SettingsPanel';

export const settingsModule: FeatureModule = {
  id: 'settings',
  name: '设置',
  icon: '序',
  caption: 'Settings',
  defaultEnabled: true,
  isCore: true, // 核心模块，不可关闭
  component: SettingsPanel,
};
