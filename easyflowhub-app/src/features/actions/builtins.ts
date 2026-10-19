import type { Action } from './types';

/**
 * Snow Shot remains an independently installed application. This action only
 * describes the Launcher entry; native discovery and process startup live in
 * the Tauri bridge.
 */
export const SNOW_SHOT_ACTION: Action = {
  id: 'external.snow-shot.capture',
  name: '截屏',
  kind: 'external',
  source: 'external',
  description: '打开 Snow Shot 截图界面，支持全屏、窗口、区域选择与标注',
  keywords: ['snow shot', 'snowshot', '截图', '截屏', 'capture', 'screen'],
  tags: ['external', 'capture'],
  category: 'Capture',
  icon: '📸',
  metadata: {
    provider: 'snow-shot',
    installUrl: 'https://snowshot.top/download.html?os=windows',
  },
};

export const CAPTURE_CLIPBOARD_ACTION: Action = {
  id: 'builtin.inbox.capture-clipboard',
  name: '收集剪贴板内容',
  kind: 'text',
  source: 'builtin',
  description: '把当前剪贴板图片或文字保存到收集箱',
  keywords: ['inbox', '收集箱', '剪贴板', '截图', '图片', '笔记', 'capture'],
  tags: ['capture', 'inbox'],
  category: 'Capture',
  icon: '收',
  inputs: [{ id: 'text', label: '剪贴板文字', kind: 'text', required: true, source: 'clipboard' }],
};

export const TRANSLATE_TEXT_ACTION: Action = {
  id: 'builtin.translation.translate-text',
  name: '翻译文本',
  kind: 'text',
  source: 'provider',
  description: '翻译当前剪贴板文字，结果可复制或保存到收集箱',
  keywords: ['translate', 'translation', '翻译', '译文', '语言'],
  tags: ['understand', 'translation'],
  category: 'Understand',
  icon: '译',
  metadata: { cancellable: true },
  inputs: [{ id: 'text', label: '待翻译文字', kind: 'text', required: true, source: 'clipboard' }],
};

