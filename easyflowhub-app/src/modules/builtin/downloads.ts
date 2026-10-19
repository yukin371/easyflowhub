import { DownloadsPanel } from '../../components/manager/downloads/DownloadsPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const downloadsModule = defineBuiltinModule(
  {
    id: 'downloads',
    name: '下载',
    icon: '下',
    caption: 'Downloads',
    defaultEnabled: true,
  },
  DownloadsPanel
);
