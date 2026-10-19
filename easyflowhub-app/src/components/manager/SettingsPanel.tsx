/**
 * SettingsPanel - 设置面板
 * 使用侧边栏导航 + 页面内滚动
 */

import { useCallback, useEffect, useMemo, useState, useRef } from 'react';
import {
  DEFAULT_SHORTCUT_CONFIG,
  SHORTCUT_STORAGE_KEY,
  formatShortcutFromKeyboardEvent,
  loadShortcutConfig,
  type ShortcutConfig,
} from '../../types/shortcut';
import {
  createExcludedApp,
  DEFAULT_LAUNCHER_SETTINGS,
  loadLauncherSettings,
  saveLauncherSettings,
  type LauncherActivation,
  type LauncherSettings,
} from '../../types/launcherSettings';
import { getSettings, updateSettings } from '../../lib/tauri/settings';
import {
  applyLauncherTriggerSettings,
  getConfiguredSnowShotPath,
  saveConfiguredSnowShotPath,
} from '../../lib/tauri/launcher';
import { DEFAULT_SETTINGS, type AppSettings, type LauncherTriggerSettings } from '../../types/settings';
import { moduleRegistry, useToggleableModules } from '../../modules/runtime';
import { isEnabled as isAutostartEnabled, enable as enableAutostart, disable as disableAutostart } from '@tauri-apps/plugin-autostart';
import { SettingsSectionCard } from './shared/SettingsSectionCard';
import { useAppearance } from '../../hooks/useAppearance';
import { THEME_OPTIONS, type ThemeMode } from '../../types/appearance';
import { clearDebugLogs, getDebugLogs, openDebugLogDir, setDebugMode } from '../../lib/tauri/logging';

type SettingsSection = 'general' | 'modules' | 'shortcuts' | 'translation' | 'transcription' | 'developer' | 'quickNote' | 'todo' | 'trash' | 'editor';

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
  { key: 'translation', label: '翻译', icon: '文' },
  { key: 'transcription', label: '转写', icon: '🎙' },
  { key: 'developer', label: '开发者', icon: '🛠' },
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
  const [launcherSettings, setLauncherSettings] = useState<LauncherSettings>(DEFAULT_LAUNCHER_SETTINGS);
  const [launcherSaveState, setLauncherSaveState] = useState<'saved' | 'dirty'>('saved');
  const [launcherError, setLauncherError] = useState<string>();
  const [recordingLauncherKey, setRecordingLauncherKey] = useState(false);
  const [excludedAppInput, setExcludedAppInput] = useState('');
  const [snowShotPath, setSnowShotPath] = useState('');
  const [snowShotPathSaveState, setSnowShotPathSaveState] = useState<'saved' | 'dirty'>('saved');

  const [appSettings, setAppSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [settingsSaveState, setSettingsSaveState] = useState<'saved' | 'dirty'>('saved');
  const [settingsError, setSettingsError] = useState<string>();

  const toggleableModules = useToggleableModules();
  const { config: appearanceConfig, setTheme } = useAppearance();

  // 自启动状态
  const [autostartOn, setAutostartOn] = useState(false);

  // 临时编辑状态，用于允许输入框清空后再输入
  const [editingValues, setEditingValues] = useState<Record<string, string>>({});
  const [debugLogLines, setDebugLogLines] = useState<string[]>([]);
  const [logLoading, setLogLoading] = useState(false);
  const [logError, setLogError] = useState<string>();

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const sectionRefs = useRef<Record<SettingsSection, HTMLDivElement | null>>({
    general: null,
    modules: null,
    shortcuts: null,
    translation: null,
    transcription: null,
    developer: null,
    quickNote: null,
    todo: null,
    trash: null,
    editor: null,
  });

  useEffect(() => {
    setShortcutConfig(loadShortcutConfig());
    setLauncherSettings(loadLauncherSettings());
    setSnowShotPath(getConfiguredSnowShotPath() ?? '');
    void refreshDebugLogs();
    getSettings().then((settings) => {
      setAppSettings({
        ...DEFAULT_SETTINGS,
        ...settings,
        translation: { ...DEFAULT_SETTINGS.translation, ...settings.translation },
        transcription: { ...DEFAULT_SETTINGS.transcription, ...settings.transcription },
        developer: { ...DEFAULT_SETTINGS.developer, ...settings.developer },
      });
      const trigger = settings.launcher_trigger;
      if (trigger) {
        const activation: LauncherActivation = trigger.trigger === 'keyboard'
          ? 'keyboard'
          : trigger.trigger === 'mouse_middle'
            ? 'middleMouse'
            : 'keyboardOrMiddleMouse';
        setLauncherSettings({
          activation,
          keyboardShortcut: trigger.keyboard_shortcut,
          excludedApps: trigger.excluded_apps
            .map((executable) => createExcludedApp(executable))
            .filter((app): app is NonNullable<ReturnType<typeof createExcludedApp>> => Boolean(app)),
        });
      }
    }).catch(console.error);

    // 读取自启动状态
    isAutostartEnabled().then(setAutostartOn).catch(() => setAutostartOn(false));
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

  const launcherShortcutConflict = useMemo(() => {
    const shortcut = launcherSettings.keyboardShortcut.trim().toLowerCase();
    if (!shortcut) return undefined;
    const conflict = SHORTCUT_FIELDS.find(({ key }) => shortcutConfig[key].trim().toLowerCase() === shortcut);
    return conflict?.label;
  }, [launcherSettings.keyboardShortcut, shortcutConfig]);

  const handleShortcutChange = (key: keyof ShortcutConfig, value: string) => {
    setShortcutConfig((prev) => ({ ...prev, [key]: value }));
    setShortcutSaveState('dirty');
  };

  const handleLauncherSettingChange = <K extends keyof LauncherSettings>(
    key: K,
    value: LauncherSettings[K],
  ) => {
    setLauncherSettings((previous) => ({ ...previous, [key]: value }));
    setLauncherSaveState('dirty');
    setLauncherError(undefined);
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

  const handleLauncherRecordKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === 'Escape') {
      setRecordingLauncherKey(false);
      return;
    }

    const shortcut = formatShortcutFromKeyboardEvent(event.nativeEvent);
    if (!shortcut) return;

    handleLauncherSettingChange('keyboardShortcut', shortcut);
    setRecordingLauncherKey(false);
  };

  const handleSaveShortcuts = async () => {
    try {
      await applyLauncherTriggerSettings(launcherSettings);
      localStorage.setItem(SHORTCUT_STORAGE_KEY, JSON.stringify(shortcutConfig));
      saveLauncherSettings(launcherSettings);
      setShortcutSaveState('saved');
      setLauncherSaveState('saved');
    } catch (error) {
      console.error('Failed to apply Launcher trigger:', error);
      setLauncherSaveState('dirty');
      setLauncherError(error instanceof Error ? error.message : 'Launcher 触发方式无法应用，请更换快捷键。');
    }
  };

  const handleResetShortcuts = async () => {
    setShortcutConfig(DEFAULT_SHORTCUT_CONFIG);
    setLauncherSettings(DEFAULT_LAUNCHER_SETTINGS);
    try {
      await applyLauncherTriggerSettings(DEFAULT_LAUNCHER_SETTINGS);
      localStorage.setItem(SHORTCUT_STORAGE_KEY, JSON.stringify(DEFAULT_SHORTCUT_CONFIG));
      saveLauncherSettings(DEFAULT_LAUNCHER_SETTINGS);
      setShortcutSaveState('saved');
      setLauncherSaveState('saved');
    } catch (error) {
      console.error('Failed to reset Launcher trigger:', error);
      setLauncherSaveState('dirty');
      setLauncherError(error instanceof Error ? error.message : 'Launcher 默认触发方式无法应用。');
    }
  };

  const handleSaveSnowShotPath = () => {
    saveConfiguredSnowShotPath(snowShotPath);
    setSnowShotPath((current) => current.trim());
    setSnowShotPathSaveState('saved');
  };

  const addExcludedApp = () => {
    const app = createExcludedApp(excludedAppInput);
    if (!app) return;
    setLauncherSettings((previous) => {
      if (previous.excludedApps.some((existing) => existing.id === app.id)) return previous;
      return { ...previous, excludedApps: [...previous.excludedApps, app] };
    });
    setLauncherSaveState('dirty');
    setExcludedAppInput('');
  };

  const removeExcludedApp = (id: string) => {
    setLauncherSettings((previous) => ({
      ...previous,
      excludedApps: previous.excludedApps.filter((app) => app.id !== id),
    }));
    setLauncherSaveState('dirty');
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
    setSettingsError(undefined);
    try {
      const launcher_trigger: LauncherTriggerSettings = {
        enabled: true,
        trigger: launcherSettings.activation === 'keyboard'
          ? 'keyboard'
          : launcherSettings.activation === 'middleMouse'
            ? 'mouse_middle'
            : 'keyboard_or_mouse_middle',
        keyboard_shortcut: launcherSettings.keyboardShortcut,
        excluded_apps: launcherSettings.excludedApps.map((app) => app.executable),
      };
      const settings = {
        ...appSettings,
        launcher_trigger,
        translation: { ...appSettings.translation, model: appSettings.translation.model.trim() },
        transcription: { ...appSettings.transcription, model: appSettings.transcription.model.trim() },
      };
      await updateSettings(settings);
      // Apply the debug flag immediately so logging starts before the next restart.
      await setDebugMode(settings.developer.debug_mode).catch(() => undefined);
      setAppSettings(settings);
      setSettingsSaveState('saved');
    } catch (err) {
      console.error('Failed to save settings:', err);
      setSettingsError('设置保存失败，请重试。');
    }
  };

  const refreshDebugLogs = useCallback(async () => {
    setLogLoading(true);
    try {
      setDebugLogLines(await getDebugLogs(200));
      setLogError(undefined);
    } catch (err) {
      // Browser preview / non-Tauri shells cannot read native logs; keep this
      // out of the settings error banner.
      setLogError(err instanceof Error ? err.message : String(err));
    } finally {
      setLogLoading(false);
    }
  }, []);

  return (
    <section className="flex h-full">
      {/* 侧边栏导航 */}
      <nav className="w-48 shrink-0 border-r border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
        <div className="space-y-1">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.key}
              onClick={() => scrollToSection(item.key)}
              className={`flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left text-sm transition ${
                activeSection === item.key
                  ? 'bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel-strong)] hover:text-[color:var(--manager-ink-strong)]'
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
            <SettingsSectionCard
              kicker="General"
              title="通用"
              description="应用启动行为等基础配置。"
            >
              <div className="space-y-2">
                <div className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
                  <div className="flex items-center gap-3">
                    <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
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
                      className={`absolute top-1 h-4 w-4 rounded-full bg-[color:var(--manager-ink-strong)] transition-transform ${
                        autostartOn ? 'left-6' : 'left-1'
                      }`}
                    />
                  </button>
                </div>
                <div className="flex flex-wrap items-center justify-between gap-4 rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3">
                  <div className="flex items-center gap-3">
                    <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
                      ◐
                    </span>
                    <div>
                      <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">界面主题</p>
                      <p className="text-xs text-[color:var(--manager-ink-subtle)]">切换深色、浅色或跟随 Windows 设置。</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-1 rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-1" role="group" aria-label="界面主题">
                    {THEME_OPTIONS.map((option) => {
                      const selected = appearanceConfig?.theme === option.id;
                      return (
                        <button
                          key={option.id}
                          type="button"
                          aria-pressed={selected}
                          onClick={() => setTheme(option.id as ThemeMode)}
                          className={`rounded-[10px] px-2.5 py-1.5 text-xs transition ${
                            selected
                              ? 'bg-[color:var(--manager-accent)] text-white shadow-sm'
                              : 'text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel-strong)]'
                          }`}
                        >
                          <span aria-hidden="true">{option.icon}</span>
                          <span className="ml-1">{option.name}</span>
                        </button>
                      );
                    })}
                  </div>
                </div>
              </div>
            </SettingsSectionCard>
          </div>

          {/* 功能模块设置 */}
          <div
            ref={(el) => { sectionRefs.current.modules = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Modules"
              title="功能模块"
              description="启用或禁用应用功能模块。禁用的模块不会显示在侧边栏中。"
            >
              <div className="space-y-2">
                {toggleableModules.map((module) => {
                  const isEnabled = moduleRegistry.isEnabled(module.id);
                  return (
                    <div
                      key={module.id}
                      className="flex items-center justify-between rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3"
                    >
                      <div className="flex items-center gap-3">
                        <span className="flex h-9 w-9 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-sm text-[color:var(--manager-ink-soft)]">
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
                          className={`absolute top-1 h-4 w-4 rounded-full bg-[color:var(--manager-ink-strong)] transition-transform ${
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
            </SettingsSectionCard>
          </div>

          {/* 快捷键设置 */}
          <div
            ref={(el) => { sectionRefs.current.shortcuts = el; }}
            className="mb-8"
          >
            <section className="rounded-[24px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-5">
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
                    className="rounded-full border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
                  >
                    恢复默认
                  </button>
                </div>
              </div>
              <p className="mb-4 text-xs text-[color:var(--manager-ink-subtle)]">
                {shortcutSaveState === 'saved' ? '✓ 配置已保存' : '○ 存在未保存修改'} · {changedCount} 项已自定义
              </p>

              <div className="mb-5 rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-accent-soft)]/45 p-4">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div>
                    <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">Launcher 唤起方式</p>
                    <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">
                      选择键盘、鼠标中键，或同时启用两种入口。排除软件在前台运行时不会响应全局唤起。
                    </p>
                  </div>
                  <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                    {launcherSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                  </span>
                </div>
                {launcherError ? (
                  <p className="mt-3 rounded-[12px] border border-[color:var(--manager-danger)]/40 bg-[color:var(--manager-danger)]/12 px-3 py-2 text-xs leading-5 text-[color:var(--manager-danger)]" role="alert">
                    {launcherError}
                  </p>
                ) : null}

                <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(0,180px)_minmax(0,1fr)] lg:items-center">
                  <label className="text-sm text-[color:var(--manager-ink-soft)]" htmlFor="launcher-activation">
                    触发入口
                  </label>
                  <select
                    id="launcher-activation"
                    aria-label="Launcher 唤起方式"
                    value={launcherSettings.activation}
                    onChange={(event) => handleLauncherSettingChange('activation', event.target.value as LauncherActivation)}
                    className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  >
                    <option value="keyboardOrMiddleMouse">键盘快捷键 + 鼠标中键</option>
                    <option value="keyboard">仅键盘快捷键</option>
                    <option value="middleMouse">仅鼠标中键</option>
                  </select>
                </div>

                <div className="mt-3 grid gap-3 lg:grid-cols-[minmax(0,180px)_minmax(0,1fr)] lg:items-center">
                  <div>
                    <p className="text-sm text-[color:var(--manager-ink-soft)]">键盘快捷键</p>
                    <p className="mt-1 text-xs text-[color:var(--manager-ink-subtle)]">系统级快捷键，建议使用不常被其他软件占用的组合。</p>
                  </div>
                  <div>
                    <button
                      type="button"
                      disabled={launcherSettings.activation === 'middleMouse'}
                      onClick={() => setRecordingLauncherKey((previous) => !previous)}
                      onKeyDown={handleLauncherRecordKeyDown}
                      className={`w-full rounded-[16px] border px-4 py-3 text-left text-sm outline-none transition disabled:cursor-not-allowed disabled:opacity-50 ${
                        recordingLauncherKey
                          ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-panel-strong)] text-[color:var(--manager-ink-strong)]'
                          : 'border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] hover:border-[color:var(--manager-accent)]'
                      }`}
                    >
                      <span className="block font-medium">{recordingLauncherKey ? '按下快捷键组合' : launcherSettings.keyboardShortcut}</span>
                      <span className="mt-1 block text-xs text-[color:var(--manager-ink-subtle)]">
                        {recordingLauncherKey ? '按 Esc 取消录制' : '点击后直接按键录入'}
                      </span>
                    </button>
                    {launcherShortcutConflict ? (
                      <p className="mt-2 text-xs text-[color:var(--manager-warning)]" role="alert">
                        与“{launcherShortcutConflict}”重复，请更换组合。
                      </p>
                    ) : null}
                  </div>
                </div>

                <div className="mt-4 border-t border-[color:var(--manager-border)]/70 pt-4">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">排除软件</p>
                      <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">填写 Windows 进程名，例如 `Code.exe`、`idea64.exe`。专注软件在前台时不会唤起 Launcher。</p>
                    </div>
                    <span className="text-xs text-[color:var(--manager-ink-subtle)]">{launcherSettings.excludedApps.length} 个</span>
                  </div>
                  <div className="mt-3 flex gap-2">
                    <input
                      value={excludedAppInput}
                      onChange={(event) => setExcludedAppInput(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          event.preventDefault();
                          addExcludedApp();
                        }
                      }}
                      aria-label="添加排除软件"
                      placeholder="例如 Code.exe"
                      className="min-w-0 flex-1 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                    />
                    <button
                      type="button"
                      onClick={addExcludedApp}
                      className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
                    >
                      添加
                    </button>
                  </div>
                  {launcherSettings.excludedApps.length ? (
                    <ul className="mt-3 flex flex-wrap gap-2" aria-label="已排除软件">
                      {launcherSettings.excludedApps.map((app) => (
                        <li key={app.id} className="inline-flex items-center gap-2 rounded-full border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-1.5 text-xs text-[color:var(--manager-ink-soft)]">
                          <span>{app.label || app.executable}</span>
                          <button
                            type="button"
                            onClick={() => removeExcludedApp(app.id)}
                            aria-label={`移除排除软件 ${app.executable}`}
                            className="text-[color:var(--manager-ink-subtle)] hover:text-[color:var(--manager-ink-strong)]"
                          >
                            ×
                          </button>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="mt-3 text-xs text-[color:var(--manager-ink-subtle)]">尚未设置排除软件。</p>
                  )}
                </div>

                <div className="mt-4 border-t border-[color:var(--manager-border)]/70 pt-4">
                  <div>
                    <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">Snow Shot 可执行文件（可选）</p>
                    <p className="mt-1 text-xs leading-6 text-[color:var(--manager-ink-subtle)]">
                      便携版或自定义安装位置可填写完整路径；留空时自动检查常见目录和 PATH。
                    </p>
                  </div>
                  <div className="mt-3 flex gap-2">
                    <input
                      value={snowShotPath}
                      onChange={(event) => {
                        setSnowShotPath(event.target.value);
                        setSnowShotPathSaveState('dirty');
                      }}
                      aria-label="Snow Shot 可执行文件路径"
                      placeholder="例如 C:\\Tools\\Snow Shot.exe"
                      className="min-w-0 flex-1 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                    />
                    <button
                      type="button"
                      onClick={handleSaveSnowShotPath}
                      className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
                    >
                      {snowShotPathSaveState === 'saved' ? '已保存' : '保存路径'}
                    </button>
                  </div>
                </div>
              </div>

              <div className="space-y-3">
                {SHORTCUT_FIELDS.map(({ key, label, description }) => (
                  <div
                    key={key}
                    className="grid gap-3 rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-4 py-3 lg:grid-cols-[180px_minmax(0,1fr)] lg:items-center"
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
                          : 'border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-[color:var(--manager-ink)] hover:border-[color:var(--manager-accent)]'
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

          <div
            ref={(el) => { sectionRefs.current.translation = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Translation"
              title="翻译"
              description="复用本机 Relay 的服务和密钥，从快捷窗口翻译剪贴板文本。"
            >
              <div className="grid gap-4 sm:grid-cols-2">
                <div>
                  <label htmlFor="translation-target-language" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    目标语言
                  </label>
                  <select
                    id="translation-target-language"
                    value={appSettings.translation.target_language}
                    onChange={(event) => {
                      setAppSettings((previous) => ({
                        ...previous,
                        translation: {
                          ...previous.translation,
                          target_language: event.target.value as AppSettings['translation']['target_language'],
                        },
                      }));
                      setSettingsSaveState('dirty');
                    }}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
                  >
                    <option value="zh-CN">简体中文</option>
                    <option value="en">英语</option>
                    <option value="ja">日语</option>
                    <option value="ko">韩语</option>
                  </select>
                </div>
                <div>
                  <label htmlFor="translation-model" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    翻译模型
                  </label>
                  <input
                    id="translation-model"
                    value={appSettings.translation.model}
                    onChange={(event) => {
                      setAppSettings((previous) => ({
                        ...previous,
                        translation: { ...previous.translation, model: event.target.value },
                      }));
                      setSettingsSaveState('dirty');
                    }}
                    placeholder="填写 Relay 中可用的模型名称"
                    autoComplete="off"
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
                  />
                </div>
              </div>
              <p className="mt-3 text-xs leading-6 text-[color:var(--manager-ink-muted)]">
                使用前请启动 Relay，并填写已配置的模型名称。
              </p>
              <div className="mt-4 flex items-center gap-4">
                <button
                  type="button"
                  onClick={handleSaveSettings}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-[color:var(--manager-bg)] transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  保存翻译设置
                </button>
                <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                  {settingsSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                </span>
              </div>
              {settingsError && <p role="alert" className="mt-3 text-sm text-[color:var(--manager-danger)]">{settingsError}</p>}
            </SettingsSectionCard>
          </div>

          {/* 转写设置 */}
          <div
            ref={(el) => { sectionRefs.current.transcription = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Transcription"
              title="转写"
              description="录音结束后将音频发送到 Relay 配置的上游 Provider 转写为文字。音频不会自动上传，每次转写都需要在录音或收集箱中确认。"
            >
              <div className="grid gap-4 sm:grid-cols-2">
                <div>
                  <label htmlFor="transcription-language" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    音频语言
                  </label>
                  <select
                    id="transcription-language"
                    value={appSettings.transcription.language}
                    onChange={(event) => {
                      setAppSettings((previous) => ({
                        ...previous,
                        transcription: {
                          ...previous.transcription,
                          language: event.target.value,
                        },
                      }));
                      setSettingsSaveState('dirty');
                    }}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
                  >
                    <option value="">自动识别</option>
                    <option value="zh">中文</option>
                    <option value="en">英语</option>
                    <option value="ja">日语</option>
                    <option value="ko">韩语</option>
                  </select>
                </div>
                <div>
                  <label htmlFor="transcription-model" className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    转写模型
                  </label>
                  <input
                    id="transcription-model"
                    value={appSettings.transcription.model}
                    onChange={(event) => {
                      setAppSettings((previous) => ({
                        ...previous,
                        transcription: { ...previous.transcription, model: event.target.value },
                      }));
                      setSettingsSaveState('dirty');
                    }}
                    placeholder="填写 Relay 中可用的语音转文字模型名称"
                    autoComplete="off"
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
                  />
                </div>
              </div>
              <p className="mt-3 text-xs leading-6 text-[color:var(--manager-ink-muted)]">
                使用前请启动 Relay，并填写已配置的模型名称。单个录音上限 25 MB（约 4.5 分钟），超出时请分段录制。密钥由 Relay 管理，此处不保存任何凭据。
              </p>
              <div className="mt-4 flex items-center gap-4">
                <button
                  type="button"
                  onClick={handleSaveSettings}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-[color:var(--manager-bg)] transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  保存转写设置
                </button>
                <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                  {settingsSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                </span>
              </div>
              {settingsError && <p role="alert" className="mt-3 text-sm text-[color:var(--manager-danger)]">{settingsError}</p>}
            </SettingsSectionCard>
          </div>

          {/* 开发者设置 */}
          <div
            ref={(el) => { sectionRefs.current.developer = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Developer"
              title="开发者模式"
              description="用于排查启动、服务连接与脚本执行问题。日志只保存在本机，不会上传；调试模式只影响日志详细程度与落盘，不改变任何安全边界。"
            >
              <label className="flex items-start gap-3 rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-3">
                <input
                  type="checkbox"
                  aria-label="开启调试日志"
                  checked={appSettings.developer.debug_mode}
                  onChange={(event) => {
                    const enabled = event.target.checked;
                    setAppSettings((previous) => ({
                      ...previous,
                      developer: { ...previous.developer, debug_mode: enabled },
                    }));
                    setSettingsSaveState('dirty');
                  }}
                  className="mt-1 h-4 w-4"
                />
                <span className="min-w-0">
                  <span className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    开启调试日志
                  </span>
                  <span className="mt-1 block text-xs leading-6 text-[color:var(--manager-ink-muted)]">
                    开启后日志会追加写入应用数据目录下的 logs/app.log（超过 2 MB 自动轮转）。
                  </span>
                </span>
              </label>

              <div className="mt-4 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={handleSaveSettings}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-2 text-sm text-[color:var(--manager-bg)] transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  保存开发者设置
                </button>
                <span className="text-xs text-[color:var(--manager-ink-subtle)]">
                  {settingsSaveState === 'saved' ? '✓ 已保存' : '○ 未保存'}
                </span>
                <button
                  type="button"
                  onClick={() => void refreshDebugLogs()}
                  disabled={logLoading}
                  className="rounded-full border border-[color:var(--manager-border)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] disabled:opacity-60"
                >
                  {logLoading ? '读取中…' : '刷新日志'}
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    await clearDebugLogs().catch(() => undefined);
                    await refreshDebugLogs();
                  }}
                  className="rounded-full border border-[color:var(--manager-border)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-danger)] hover:text-[color:var(--manager-danger)]"
                >
                  清空
                </button>
                <button
                  type="button"
                  onClick={() => void openDebugLogDir().catch((err) => setSettingsError(String(err)))}
                  className="rounded-full border border-[color:var(--manager-border)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)]"
                >
                  打开日志目录
                </button>
              </div>

              <pre
                aria-label="调试日志"
                className="mt-4 max-h-[280px] overflow-auto whitespace-pre-wrap break-words rounded-[14px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-3 font-mono text-[11px] leading-6 text-[color:var(--manager-ink-soft)]"
              >
                {debugLogLines.length > 0 ? debugLogLines.join('\n') : logError ? `无法读取日志：${logError}` : '暂无日志'}
              </pre>
              {settingsError && <p role="alert" className="mt-3 text-sm text-[color:var(--manager-danger)]">{settingsError}</p>}
            </SettingsSectionCard>
          </div>

          {/* 快速笔记设置 */}
          <div
            ref={(el) => { sectionRefs.current.quickNote = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Quick Note"
              title="快速笔记窗口"
              description="新建快速笔记窗口的默认尺寸。"
            >
              <div className="mt-6 grid gap-4 sm:grid-cols-2">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
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
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  />
                </div>
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
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
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
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
            </SettingsSectionCard>
          </div>

          {/* 待办设置 */}
          <div
            ref={(el) => { sectionRefs.current.todo = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Todos"
              title="待办"
              description="已完成的待办会在保留期内以删除线显示，方便确认和撤销误操作。"
            >
              <div className="mt-6">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
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
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
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
            </SettingsSectionCard>
          </div>

          {/* 回收站设置 */}
          <div
            ref={(el) => { sectionRefs.current.trash = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Trash"
              title="回收站"
              description="回收站中的笔记在指定天数后将自动永久删除。"
            >
              <div className="mt-6">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    保留天数
                  </label>
                  <input
                    type="number"
                    min={1}
                    max={90}
                    value={appSettings.trash.retention_days}
                    onChange={(e) => handleSettingsChange('trash', 'retention_days', parseInt(e.target.value) || 30)}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
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
            </SettingsSectionCard>
          </div>

          {/* 编辑器设置 */}
          <div
            ref={(el) => { sectionRefs.current.editor = el; }}
            className="mb-8"
          >
            <SettingsSectionCard
              kicker="Editor"
              title="编辑器"
              description="编辑器保留的撤销步数与光标可见性。保持轻量，不引入复杂编辑器内核。"
            >
              <div className="mt-6 grid gap-4 lg:grid-cols-2">
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
                  <label className="block text-sm font-medium text-[color:var(--manager-ink-strong)]">
                    撤销步数
                  </label>
                  <input
                    type="number"
                    min={10}
                    max={500}
                    value={appSettings.editor.undo_steps}
                    onChange={(e) => handleSettingsChange('editor', 'undo_steps', parseInt(e.target.value) || 100)}
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
                  />
                  <p className="mt-2 text-xs text-[color:var(--manager-ink-subtle)]">
                    建议设置 50-200 步。使用 Ctrl+Z 撤销，Ctrl+Shift+Z 重做。
                  </p>
                </div>
                <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
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
                    className="mt-2 w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
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
                      className="h-10 w-14 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] p-1"
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
                      className="flex-1 rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm outline-none focus:border-[color:var(--manager-accent)]"
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
            </SettingsSectionCard>
          </div>
        </div>
      </div>
    </section>
  );
}
