/**
 * 组件预览模块定义
 */
import { ComponentPreviewPanel } from '../../components/manager/ComponentPreviewPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const componentsModule = defineBuiltinModule(
  {
    id: 'components',
    name: '组件',
    icon: '件',
    caption: 'Components',
    defaultEnabled: false,
  },
  ComponentPreviewPanel
);
