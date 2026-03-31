/**
 * 组件预览模块定义
 */
import type { FeatureModule } from '../types';
import { ComponentPreviewPanel } from '../../components/manager/ComponentPreviewPanel';

export const componentsModule: FeatureModule = {
  id: 'components',
  name: '组件',
  icon: '件',
  caption: 'Components',
  defaultEnabled: false,
  component: ComponentPreviewPanel,
};
