/**
 * 脚本模块定义
 */
import { ScriptsPanel } from '../../components/manager/scripts/ScriptsPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const scriptsModule = defineBuiltinModule(
  {
    id: 'scripts',
    name: '脚本',
    icon: '策',
    caption: 'Scripts',
    defaultEnabled: false,
  },
  ScriptsPanel
);
