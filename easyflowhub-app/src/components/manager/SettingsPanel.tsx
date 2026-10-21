/**
 * SettingsPanel - 设置面板
 * 只负责导航、分区组合与页面级反馈；
 * 各设置域的加载 / 草稿 / 提交逻辑在 settings/ 下的域 hook，
 * 视图在 settings/*Section.tsx。
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { useAppSettings } from './settings/useAppSettings';
import { useShortcutsSettings } from './settings/useShortcutsSettings';
import { GeneralSection } from './settings/GeneralSection';
import { ModulesSection } from './settings/ModulesSection';
import { ShortcutsSection } from './settings/ShortcutsSection';
import { TranslationSection } from './settings/TranslationSection';
import { TranscriptionSection } from './settings/TranscriptionSection';
import { DeveloperSection } from './settings/DeveloperSection';
import { QuickNoteSection } from './settings/QuickNoteSection';
import { TodoSection } from './settings/TodoSection';
import { TrashSection } from './settings/TrashSection';
import { EditorSection } from './settings/EditorSection';
import {
  MANAGER_OPEN_SETTINGS_SECTION_EVENT,
  type ManagerSettingsNavigationDetail,
} from '../../lib/tauri/manager';

type SettingsSection = 'general' | 'modules' | 'shortcuts' | 'translation' | 'transcription' | 'developer' | 'quickNote' | 'todo' | 'trash' | 'editor';

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

  const app = useAppSettings();
  // Launcher 提交成功后刷新普通设置聚合的 launcher_trigger 镜像（F08）。
  const shortcuts = useShortcutsSettings({ onLauncherCommitted: app.syncLauncherTrigger });

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

  // 重新加载普通设置，并把持久化的 launcher_trigger 同步进 Launcher 草稿。
  const reloadAll = useCallback(() => {
    app.reload((settings) => shortcuts.launcher.syncFromTrigger(settings.launcher_trigger));
  }, [app.reload, shortcuts.launcher.syncFromTrigger]);

  useEffect(() => {
    reloadAll();
  }, [reloadAll]);

  // 滚动到指定区块
  const scrollToSection = useCallback((section: SettingsSection) => {
    const el = sectionRefs.current[section];
    if (el && scrollContainerRef.current) {
      const container = scrollContainerRef.current;
      const offsetTop = el.offsetTop - container.offsetTop - 16;
      container.scrollTo?.({ top: offsetTop, behavior: 'smooth' });
    }
    setActiveSection(section);
  }, []);

  useEffect(() => {
    const handleSettingsNavigation = (event: Event) => {
      const detail = (event as CustomEvent<ManagerSettingsNavigationDetail>).detail;
      if (!detail?.section || !sectionRefs.current[detail.section]) return;
      scrollToSection(detail.section);
    };

    window.addEventListener(MANAGER_OPEN_SETTINGS_SECTION_EVENT, handleSettingsNavigation as EventListener);
    return () => window.removeEventListener(MANAGER_OPEN_SETTINGS_SECTION_EVENT, handleSettingsNavigation as EventListener);
  }, [scrollToSection]);

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

  return (
    <section className="flex h-full">
      {/* 侧边栏导航 */}
      <nav className="w-48 shrink-0 border-r border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4">
        <div className="space-y-1">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.key}
              type="button"
              onClick={() => scrollToSection(item.key)}
              aria-current={activeSection === item.key ? 'true' : undefined}
              className={`flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left text-sm transition ${
                activeSection === item.key
                  ? 'bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                  : 'text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-panel-strong)] hover:text-[color:var(--manager-ink-strong)]'
              }`}
            >
              <span className="text-base" aria-hidden="true">{item.icon}</span>
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
          {app.loadState === 'failed' ? (
            <div
              role="alert"
              className="mb-6 flex flex-wrap items-center justify-between gap-3 rounded-[14px] border border-[color:var(--manager-danger)]/40 bg-[color:var(--manager-danger)]/8 px-4 py-3"
            >
              <span className="text-sm text-[color:var(--manager-danger)]">
                设置加载失败，为避免覆盖现有配置，保存已暂停。
              </span>
              <button
                type="button"
                onClick={reloadAll}
                className="rounded-full border border-[color:var(--manager-danger)]/40 px-4 py-1.5 text-sm text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/12"
              >
                重试加载
              </button>
            </div>
          ) : null}

          {/* 通用设置 */}
          <div
            ref={(el) => { sectionRefs.current.general = el; }}
            className="mb-8"
          >
            <GeneralSection />
          </div>

          {/* 功能模块设置 */}
          <div
            ref={(el) => { sectionRefs.current.modules = el; }}
            className="mb-8"
          >
            <ModulesSection />
          </div>

          {/* 快捷键设置（含 Launcher 触发与 Snow Shot 路径） */}
          <div
            ref={(el) => { sectionRefs.current.shortcuts = el; }}
            className="mb-8"
          >
            <ShortcutsSection domain={shortcuts} />
          </div>

          {/* 翻译设置 */}
          <div
            ref={(el) => { sectionRefs.current.translation = el; }}
            className="mb-8"
          >
            <TranslationSection app={app} />
          </div>

          {/* 转写设置 */}
          <div
            ref={(el) => { sectionRefs.current.transcription = el; }}
            className="mb-8"
          >
            <TranscriptionSection app={app} />
          </div>

          {/* 开发者设置 */}
          <div
            ref={(el) => { sectionRefs.current.developer = el; }}
            className="mb-8"
          >
            <DeveloperSection app={app} />
          </div>

          {/* 快速笔记设置 */}
          <div
            ref={(el) => { sectionRefs.current.quickNote = el; }}
            className="mb-8"
          >
            <QuickNoteSection app={app} />
          </div>

          {/* 待办设置 */}
          <div
            ref={(el) => { sectionRefs.current.todo = el; }}
            className="mb-8"
          >
            <TodoSection app={app} />
          </div>

          {/* 回收站设置 */}
          <div
            ref={(el) => { sectionRefs.current.trash = el; }}
            className="mb-8"
          >
            <TrashSection app={app} />
          </div>

          {/* 编辑器设置 */}
          <div
            ref={(el) => { sectionRefs.current.editor = el; }}
            className="mb-8"
          >
            <EditorSection app={app} />
          </div>
        </div>
      </div>
    </section>
  );
}
