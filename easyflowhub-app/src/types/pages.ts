// Page types for EasyFlowHub multi-page architecture

export type PageType = 'notes' | 'scripts' | 'manager' | 'beautification' | 'search' | 'settings';

export interface Page {
  id: string;
  type: PageType;
  title: string;
  icon: string;
  isPinned?: boolean;  // Can be pinned to desktop
  position?: { x: number; y: number };  // Position when pinned
}

export interface PageConfig {
  type: PageType;
  label: string;
  icon: string;
  description: string;
}

// Available page configurations
export const PAGE_CONFIGS: Record<PageType, PageConfig> = {
  notes: {
    type: 'notes',
    label: '笔记',
    icon: '📝',
    description: 'Markdown 笔记编辑器',
  },
  scripts: {
    type: 'scripts',
    label: '脚本',
    icon: '⚡',
    description: '脚本管理和快速启动',
  },
  manager: {
    type: 'manager',
    label: '管理',
    icon: '📋',
    description: '笔记管理和批量操作',
  },
  beautification: {
    type: 'beautification',
    label: '美化',
    icon: '🎨',
    description: '桌面美化和外观设置',
  },
  search: {
    type: 'search',
    label: '搜索',
    icon: '🔍',
    description: '桌面文件搜索',
  },
  settings: {
    type: 'settings',
    label: '设置',
    icon: '⚙️',
    description: '应用设置',
  },
};
