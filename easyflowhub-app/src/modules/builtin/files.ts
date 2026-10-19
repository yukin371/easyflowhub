/**
 * 指定目录文件索引与管理模块定义
 */
import { FilesPanel } from '../../components/manager/files/FilesPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const filesModule = defineBuiltinModule(
  {
    id: 'files',
    name: '文件',
    icon: '档',
    caption: 'Files',
    defaultEnabled: true,
  },
  FilesPanel
);
