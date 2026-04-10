import { ExtensionsPanel } from '../../components/manager/extensions/ExtensionsPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const extensionsModule = defineBuiltinModule(
  {
    id: 'extensions',
    name: '扩展',
    icon: '拓',
    caption: 'Extensions',
    defaultEnabled: true,
  },
  ExtensionsPanel
);
