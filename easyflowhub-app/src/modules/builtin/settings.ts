/**
 * 设置模块定义
 */
import { SettingsPanel } from '../../components/manager/SettingsPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const settingsModule = defineBuiltinModule(
  {
    id: 'settings',
    name: '设置',
    icon: '序',
    caption: 'Settings',
    defaultEnabled: true,
    isCore: true,
  },
  SettingsPanel
);
