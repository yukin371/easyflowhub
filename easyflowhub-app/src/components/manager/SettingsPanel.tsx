/**
 * SettingsPanel - 设置面板
 * 使用侧边栏导航 + 页面内滚动
 */

import { useEffect, useMemo, useState, useRef } from 'react';
import {
  DEFAULT_SHORTCUT_CONFIG,
  SHORTCUT_STORAGE_KEY,
  formatShortcutFromKeyboardEvent,
  loadShortcutConfig,
  type ShortcutConfig,
} from '../../types/shortcut';
import { getSettings, updateSettings } from '../../lib/tauri/settings';
import type { AppSettings } from '../../types/settings';
import { moduleRegistry } from '../../modules';
import type { FeatureModule } from '../../modules';
import { isEnabled as isAutostartEnabled, enable as enableAutostart, disable as disableAutostart } from '@tauri-apps/plugin-autostart';

type SettingsSection = 'general' | 'modules' | 'shortcuts' | 'quickNote' | 'todo' | 'trash' | 'editor';

interface ShortcutField {
  key: keyof ShortcutConfig;
  label: string;
  description: string;
}

const SHORTCUT_FIELDS: ShortcutField[] = [
  { key: 'save', label: '立即保存', description: '在编辑态直接执行一次保存，不等待防抖。' },
  { key: 'close', label: '退出编辑', description: '离开当前编辑态并触发必要的收尾保存。' },
  { key: 'quickSwitcher', label: '唤起切换器', description: '在快速笔记里呼出最近笔记切换面板。' },
  { key: 'toggleAlwaysOnTop', label: '切换置顶', description: '快速笔记窗口在置顶和取消置顶之间切换。' },
  { key: 'cutLine', label: '剪切行', description: '编辑器命令，默认避开系统 Ctrl+X。' },
  { key: 'deleteLine', label: '删除行', description: '编辑器命令，和关闭当前快速笔记分离。' },
  { key: 'duplicateLine', label: '复制行', description: '复制当前行到下一行。' },
  { key: 'insertLineBelow', label: '下方插入', description: '在当前行下方插入空行。' },
  { key: 'insertLineAbove', label: '上方插入', description: '在当前行上方插入空行。' },
  { key: 'heading1', label: '一级标题', description: '将当前行或选区设为一级标题。' },
  { key: 'heading2', label: '二级标题', description: '将当前行或选区设为二级标题。' },
  { key: 'heading3', label: '三级标题', description: '将当前行或选区设为三级标题。' },
  { key: 'clearHeading', label: '清除标题', description: '移除当前行或选区的标题前缀。' },
];

const NAV_ITEMS: { key: SettingsSection; label: string; icon: string }[] = [
  { key: 'general', label: '通用', icon: '⚙' },
  { key: 'modules', label: '功能模块', icon: '🧩' },
  { key: 'shortcuts', label: '快捷键', icon: '⌨' },
  { key: 'quickNote', label: '快速笔记', icon: '📝' },
  { key: 'todo', label: '待办', icon: '☑' },
  { key: 'trash', label: '回收站', icon: '🗑' },
  { key: 'editor', label: '编辑器', icon: '✏' },
];

export function SettingsPanel() {
  const [activeSection, setActiveSection] = useState<SettingsSection>('general');
  const [shortcutConfig, setShortcutConfig] = useState<ShortcutConfig>(DEFAULT_SHORTCUT_CONFIG);
  const [shortcutSaveState, setShortcutSaveState] = useState<'saved' | 'dirty'>('saved');
  const [recordingKey, setRecordingKey] = useState<keyof ShortcutConfig | null>(null);

  const [appSettings, setAppSettings] = useState<AppSettings>({
    quick_note: { width: 400, height: 300 },
    trash: { retention_days: 30 },
    editor: {
      undo_steps: 100,
      cursor_style: 'accent',
      cursor_color: '#4f5a43',
      cursor_trail: true,
    },
    todo: { done_retention_hours: 24 },
  });
  const [settingsSaveState, setSettingsSaveState] = useState<'saved' | 'dirty'>('saved');

  // 模块状态
  const [toggleableModules, setToggleableModules] = useState<FeatureModule[]>([]);

  // 自启动状态
  const [autostartOn, setAutostartOn] = useState(false);

  // 临时编辑状态，用于允许输入框清空后再输入
  const [editingValues, setEditingValues] = useState<Record<string, string>>({});

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const sectionRefs = useRef<Record<SettingsSection, HTMLDivElement | null>>({
    general: null,
    modules: null,
    shortcuts: null,
    quickNote: null,
    todo: null,
    trash: null,
    editor: null,
  });

  useEffect(() => {
    setShortcutConfig(loadShortcutConfig());
    getSettings().then(setAppSettings).catch(console.error);

    // 读取自启动状态
    isAutostartEnabled().then(setAutostartOn).catch(() => setAutostartOn(false));

    // 加载可切换的模块
    setToggleableModules(moduleRegistry.getToggleableModules());

    // 订阅模块配置变更
    const unsubscribe = moduleRegistry.subscribe(() => {
      setToggleableModules(moduleRegistry.getToggleableModules());
    });

    return unsubscribe;
  }, []);

  // 滚动到指定区块
  const scrollToSection = (section: SettingsSection) => {
    const el = sectionRefs.current[section];
    if (el && scrollContainerRef.current) {
      const container = scrollContainerRef.current;
      const offsetTop = el.offsetTop - container.offsetTop - 16;
      container.scrollTo({ top: offsetTop, behavior: 'smooth' });
    }
    setActiveSection(section);
  };

  // 监听滚动更新当前区块
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const handleScroll = () => {
      const scrollTop = container.scrollTop;
      let currentSection: SettingsSection = 'shortcuts';

      for (const key of NAV_ITEMS.map(item => item.key)) {
        const el = sectionRefs.current[key];
        if (el) {
          const offsetTop = el.offsetTop - container.offsetTop - 32;
          if (scrollTop >= offsetTop) {
            currentSection = key;
          }
        }
      }

      setActiveSection(currentSection);
    };

    container.addEventListener('scroll', handleScroll);
    return () => container.removeEventListener('scroll', handleScroll);
  }, []);

  const changedCount = useMemo(
    () =>
      SHORTCUT_FIELDS.filter(
        ({ key }) => shortcutConfig[key].trim() !== DEFAULT_SHORTCUT_CONFIG[key].trim()
      ).length,
    [shortcutConfig]
  );

  const handleShortcutChange = (key: keyof ShortcutConfig, value: string) => {
    setShortcutConfig((prev) => ({ ...prev, [key]: value }));
    setShortcutSaveState('dirty');
  };

  const handleRecordKeyDown = (key: keyof ShortcutConfig, event: React.KeyboardEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === 'Escape' && recordingKey === key) {
      setRecordingKey(null);
      return;
    }

    const shortcut = formatShortcutFromKeyboardEvent(event.nativeEvent);
    if (!shortcut) {
      return;
    }

    handleShortcutChange(key, shortcut);
    setRecordingKey(null);
  };

  const handleSaveShortcuts = () => {
    localStorage.setItem(SHORTCUT_STORAGE_KEY, JSON.stringify(shortcutConfig));
    setShortcutSaveState('saved');
  };

  const handleResetShortcuts = () => {
    setShortcutConfig(DEFAULT_SHORTCUT_CONFIG);
    localStorage.setItem(SHORTCUT_STORAGE_KEY, JSON.stringify(DEFAULT_SHORTCUT_CONFIG));
    setShortcutSaveState('saved');
  };

  // 切换模块启用状态
  const handleToggleModule = async (moduleId: string, enabled: boolean) => {
    await moduleRegistry.toggleModule(moduleId, enabled);
  };

  const handleSettingsChange = <K extends keyof AppSettings>(
    category: K,
    field: keyof AppSettings[K],
    value: number
  ) => {
    setAppSettings((prev) => ({
      ...prev,
      [category]: { ...prev[category], [field]: value },
    }));
    setSettingsSaveState('dirty');
  };

  const handleSaveSettings = async () => {
    try {
      await updateSettings(appSettings);
      setSettingsSaveState('saved');
    } catch (err) {
      console.error('Failed to save settings:', err);
    }
  };

  return (
    <section className="flex h-full">
      {/* 侧边栏导航 */}
      <nav className="w-48 shrink-0 border-r border-[color:var(--manager-border)] bg-white/40 p-4">
        <div className="space-y-1">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.key}
              onClick={() => scrollToSection(item.key)}
              className={`flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left text-sm transition ${
                activeSection === item.key
                  ? 'bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'text-[color:var(--manager-ink-soft)] hover:bg-white/60 hover:text-[color:var(--manager-ink-strong)]'
              }`}
            >
              <span className="text-base">{item.icon}</span>
              <span>{item.label}</span>
            </button>
          ))}
        </div>
      </nav>

      {/* 内容区域 */}
      <div className="flex-1 overflow-hidden">
        <header className="border-b border-[color:var(--manager-border)] px-6 py-5">
          <p className="manager-kicker">Settings</p>
          <h2 className="mt-2 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-4xl text-[color:var(--manager-ink-strong)]">
            设置
          </h2>
          <p className="mt-2 text-sm leading-7 text-[color:var(--manager-ink-soft)]">
            配置应用行为和编辑习惯。更改会在保存后立即生效。
          </p>
        </header>

        <div ref={scrollContainerRef} className="h-[calc(100%-120px)] overflow-y-auto px-6 py-5">
          {/* 通用设置 */}
          <div
            ref={(el) => { sectionRefs.current.general = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5">
              <div className="mb-4">
                <p className="manager-kicker">General</p>
                <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">通用</h3>
              </div>
              <p className="mb-4 text-sm text-[color:var(--manager-ink-soft)]">
                应用启动行为等基础配置。
              </p>

              <div className="space-y-2">
                <div className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 px-4 py-3">
                  <div className="flex items-center gap-3">
                    <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-white/70 text-sm text-[color:var(--manager-ink-soft)]">
                      🚀
                    </span>
                    <div>
                      <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
                        开机自启动
                      </p>
                      <p className="text-xs text-[color:var(--manager-ink-subtle)]">
                        系统启动时自动运行 EasyFlowHub
                      </p>
                    </div>
                  </div>
                  <button
                    onClick={async () => {
                      try {
                        if (autostartOn) {
                          await disableAutostart();
                          setAutostartOn(false);
                        } else {
                          await enableAutostart();
                          setAutostartOn(true);
                        }
                      } catch (err) {
                        console.error('Failed to toggle autostart:', err);
                      }
                    }}
                    className={`relative h-6 w-11 rounded-full transition-colors ${
                      autostartOn
                        ? 'bg-[color:var(--manager-accent)]'
                        : 'bg-[color:var(--manager-border)]'
                    }`}
                  >
                    <span
                      className={`absolute top-1 h-4 w-4 rounded-full bg-white transition-transform ${
                        autostartOn ? 'left-6' : 'left-1'
                      }`}
                    />
                  </button>
                </div>
              </div>
            </section>
          </div>

          {/* 功能模块设置 */}
          <div
            ref={(el) => { sectionRefs.current.modules = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5">
              <div className="mb-4">
                <p className="manager-kicker">Modules</p>
                <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">功能模块</h3>
              </div>
              <p className="mb-4 text-sm text-[color:var(--manager-ink-soft)]">
                启用或禁用应用功能模块。禁用的模块不会显示在侧边栏中。
              </p>

              <div className="space-y-2">
                {toggleableModules.map((module) => {
                  const isEnabled = moduleRegistry.isEnabled(module.id);
                  return (
                    <div
                      key={module.id}
                      className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 px-4 py-3"
                    >
                      <div className="flex items-center gap-3">
                        <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-white/70 text-sm text-[color:var(--manager-ink-soft)]">
                          {module.icon}
                        </span>
                        <div>
                          <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
                            {module.name}
                          </p>
                          <p className="text-xs text-[color:var(--manager-ink-subtle)]">
                            {module.caption}
                          </p>
                        </div>
                      </div>
                      <button
                        onClick={() => handleToggleModule(module.id, !isEnabled)}
                        className={`relative h-6 w-11 rounded-full transition-colors ${
                          isEnabled
                            ? 'bg-[color:var(--manager-accent)]'
                            : 'bg-[color:var(--manager-border)]'
                        }`}
                      >
                        <span
                          className={`absolute top-1 h-4 w-4 rounded-full bg-white transition-transform ${
                            isEnabled ? 'left-6' : 'left-1'
                          }`}
                        />
                      </button>
                    </div>
                  );
                })}
              </div>

              {toggleableModules.length === 0 && (
                <p className="text-sm text-[color:var(--manager-ink-subtle)]">
                  所有模块已启用。
                </p>
              )}
            </section>
          </div>

          {/* 快捷键设置 */}
          <div
            ref={(el) => { sectionRefs.current.shortcuts = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5">
              <div className="mb-4 flex items-center justify-between">
                <div>
                  <p className="manager-kicker">Shortcuts</p>
                  <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">快捷键配置</h3>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={handleSaveShortcuts}
                    className="rounded-full bg-[color:var(--manager-accent)] px-4 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                  >
                    保存配置
                  </button>
                  <button
                    onClick={handleResetShortcuts}
                    className="rounded-full border border-[color:var(--manager-border)] bg-white/75 px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
                  >
                    恢复默认
                  </button>
                </div>
              </div>
              <p className="mb-4 text-xs text-[color:var(--manager-ink-subtle)]">
                {shortcutSaveState === 'saved' ? '✓ 配置已保存' : '○ 存在未保存修改'} · {changedCount} 项已自定义
              </p>

              <div className="space-y-3">
                {SHORTCUT_FIELDS.map(({ key, label, description }) => (
                  <div
                    key={key}
                    className="grid gap-3 rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 px-4 py-3 lg:grid-cols-[180px_minmax(0,1fr)] lg:items-center"
                  >
                    <div>
                      <p className="text-sm tracking-[0.16em] text-[color:var(--manager-ink-strong)]">
                        {label}
                      </p>
                      <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">
                        {description}
                      </p>
                    </div>
                    <button
                      onClick={() => setRecordingKey((prev) => (prev === key ? null : key))}
                      onKeyDown={(event) => handleRecordKeyDown(key, event)}
                      className={`rounded-[16px] border px-4 py-3 text-left text-sm outline-none transition ${
                        recordingKey === key
                          ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                          : 'border-[color:var(--manager-border)] bg-white text-[color:var(--manager-ink)] hover:border-[color:var(--manager-accent)]'
                      }`}
                    >
                      <span className="block font-medium">{recordingKey === key ? '按下快捷键组合' : shortcutConfig[key]}</span>
                      <span className="mt-1 block text-xs text-[color:var(--manager-ink-subtle)]">
                        {recordingKey === key ? '按 Esc 取消录制' : '点击后直接按键录入'}
                      </span>
                    </button>
                  </div>
                ))}
              </div>
            </section>
          </div>

          {/* 快速笔记设置 */}
          <div
            ref={(el) => { sectionRefs.current.quickNote = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5">
              <p className="manager-kicker">Quick Note</p>
              <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">快速笔记窗口</h3>
              <p className="mt-2 text-sm text-[color:var(--manager-ink-soft)]">
                新建快速笔记窗口的默认尺寸。
              </p>

              <div className="mt-6 grid gap-4 sm:grid-cols-2">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    宽度 (px)
                  </label>
                  <input
                    type="number"
                    min={200}
                    max={800}
                    value={editingValues['quick_note.width'] ?? appSettings.quick_note.width}
                    onChange={(e) => {
                      const val = e.target.value;
                      setEditingValues(prev => ({ ...prev, 'quick_note.width': val }));
                    }}
                    onBlur={(e) => {
                      const num = parseInt(e.target.value);
                      const finalValue = isNaN(num) || num < 200 || num > 800 ? 400 : num;
                      handleSettingsChange('quick_note', 'width', finalValue);
                      setEditingValues(prev => {
                        const { 'quick_note.width': _, ...rest } = prev;
                        return rest;
                      });
                    }}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  />
                </div>
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    高度 (px)
                  </label>
                  <input
                    type="number"
                    min={150}
                    max={600}
                    value={editingValues['quick_note.height'] ?? appSettings.quick_note.height}
                    onChange={(e) => {
                      const val = e.target.value;
                      setEditingValues(prev => ({ ...prev, 'quick_note.height': val }));
                    }}
                    onBlur={(e) => {
                      const num = parseInt(e.target.value);
                      const finalValue = isNaN(num) || num < 150 || num > 600 ? 300 : num;
                      handleSettingsChange('quick_note', 'height', finalValue);
                      setEditingValues(prev => {
                        const { 'quick_note.height': _, ...rest } = prev;
                        return rest;
                      });
                    }}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  />
                </div>
              </div>

              <div className="mt-4 flex items-center gap-4">
                <button
                  onClick={handleSaveSettings}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  保存设置
                </button>
                <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                  {settingsSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                </span>
              </div>
            </section>
          </div>

          {/* 待办设置 */}
          <div
            ref={(el) => { sectionRefs.current.todo = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5">
              <p className="manager-kicker">Todos</p>
              <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">待办</h3>
              <p className="mt-2 text-sm text-[color:var(--manager-ink-soft)]">
                已完成的待办会在保留期内以删除线显示，方便确认和撤销误操作。
              </p>

              <div className="mt-6">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    完成项保留时间（小时）
                  </label>
                  <input
                    type="number"
                    min={1}
                    max={168}
                    value={editingValues['todo.done_retention_hours'] ?? appSettings.todo.done_retention_hours}
                    onChange={(e) => {
                      const val = e.target.value;
                      setEditingValues(prev => ({ ...prev, 'todo.done_retention_hours': val }));
                    }}
                    onBlur={(e) => {
                      const num = parseInt(e.target.value);
                      const finalValue = isNaN(num) || num < 1 || num > 168 ? 24 : num;
                      handleSettingsChange('todo', 'done_retention_hours', finalValue);
                      setEditingValues(prev => {
                        const { 'todo.done_retention_hours': _, ...rest } = prev;
                        return rest;
                      });
                    }}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  />
                  <p className="mt-2 text-xs text-[color:var(--manager-ink-subtle)]">
                    默认 24 小时。设为 0 则不保留已完成项。最长 168 小时（7天）。
                  </p>
                </div>
              </div>

              <div className="mt-4 flex items-center gap-4">
                <button
                  onClick={handleSaveSettings}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  保存设置
                </button>
                <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                  {settingsSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                </span>
              </div>
            </section>
          </div>

          {/* 回收站设置 */}
          <div
            ref={(el) => { sectionRefs.current.trash = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5">
              <p className="manager-kicker">Trash</p>
              <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">回收站</h3>
              <p className="mt-2 text-sm text-[color:var(--manager-ink-soft)]">
                回收站中的笔记在指定天数后将自动永久删除。
              </p>

              <div className="mt-6">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    保留天数
                  </label>
                  <input
                    type="number"
                    min={1}
                    max={90}
                    value={appSettings.trash.retention_days}
                    onChange={(e) => handleSettingsChange('trash', 'retention_days', parseInt(e.target.value) || 30)}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  />
                  <p className="mt-2 text-xs text-[color:var(--manager-ink-subtle)]">
                    建议设置 7-30 天。超过保留期的笔记将被自动清除。
                  </p>
                </div>
              </div>

              <div className="mt-4 flex items-center gap-4">
                <button
                  onClick={handleSaveSettings}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  保存设置
                </button>
                <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                  {settingsSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                </span>
              </div>
            </section>
          </div>

          {/* 编辑器设置 */}
          <div
            ref={(el) => { sectionRefs.current.editor = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-white/62 p-5">
              <p className="manager-kicker">Editor</p>
              <h3 className="mt-2 text-lg text-[color:var(--manager-ink-strong)]">编辑器</h3>
              <p className="mt-2 text-sm text-[color:var(--manager-ink-soft)]">
                编辑器保留的撤销步数与光标可见性。保持轻量，不引入复杂编辑器内核。
              </p>

              <div className="mt-6 grid gap-4 lg:grid-cols-2">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    撤销步数
                  </label>
                  <input
                    type="number"
                    min={10}
                    max={500}
                    value={appSettings.editor.undo_steps}
                    onChange={(e) => handleSettingsChange('editor', 'undo_steps', parseInt(e.target.value) || 100)}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  />
                  <p className="mt-2 text-xs text-[color:var(--manager-ink-subtle)]">
                    建议设置 50-200 步。使用 Ctrl+Z 撤销，Ctrl+Shift+Z 重做。
                  </p>
                </div>
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-white/68 p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    光标风格
                  </label>
                  <select
                    value={appSettings.editor.cursor_style}
                    onChange={(e) => {
                      setAppSettings((prev) => ({
                        ...prev,
                        editor: {
                          ...prev.editor,
                          cursor_style: e.target.value as AppSettings['editor']['cursor_style'],
                        },
                      }));
                      setSettingsSaveState('dirty');
                    }}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  >
                    <option value="default">默认</option>
                    <option value="accent">强调</option>
                    <option value="focus">聚焦</option>
                  </select>
                  <label className="mt-4 block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    光标颜色
                  </label>
                  <div className="mt-2 flex items-center gap-3">
                    <input
                      type="color"
                      value={appSettings.editor.cursor_color}
                      onChange={(e) => {
                        setAppSettings((prev) => ({
                          ...prev,
                          editor: { ...prev.editor, cursor_color: e.target.value },
                        }));
                        setSettingsSaveState('dirty');
                      }}
                      className="h-10 w-14 rounded-[12px] border border-[color:var(--manager-border)] bg-white p-1"
                    />
                    <input
                      type="text"
                      value={appSettings.editor.cursor_color}
                      onChange={(e) => {
                        setAppSettings((prev) => ({
                          ...prev,
                          editor: { ...prev.editor, cursor_color: e.target.value || '#4f5a43' },
                        }));
                        setSettingsSaveState('dirty');
                      }}
                      className="flex-1 rounded-[12px] border border-[color:var(--manager-border)] bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                    />
                  </div>
                  <label className="mt-4 flex items-center gap-3 text-sm text-[color:var(--manager-ink-soft)]">
                    <input
                      type="checkbox"
                      checked={appSettings.editor.cursor_trail}
                      onChange={(e) => {
                        setAppSettings((prev) => ({
                          ...prev,
                          editor: { ...prev.editor, cursor_trail: e.target.checked },
                        }));
                        setSettingsSaveState('dirty');
                      }}
                    />
                    <span>启用简易聚焦光标特效</span>
                  </label>
                </div>
              </div>

              <div className="mt-4 flex items-center gap-4">
                <button
                  onClick={handleSaveSettings}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  保存设置
                </button>
                <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                  {settingsSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                </span>
              </div>
            </section>
          </div>
        </div>
      </div>
    </section>
  );
}
