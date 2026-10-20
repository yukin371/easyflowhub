/**
 * NotesPanel - 祅模式笔记管理面板（收集箱 / 笔记归档两个工作区标签）
 * 归档工作区的组合与窗口交互见 notes/NotesArchivePanel（Phase C 拆分）。
 */

import { useEffect, useState } from 'react';
import { NotesArchivePanel } from './notes/NotesArchivePanel';
import { InboxPanel } from './notes/InboxPanel';

export function NotesPanel() {
  const [activeView, setActiveView] = useState<'inbox' | 'notes'>('inbox');

  useEffect(() => {
    const openInbox = () => setActiveView('inbox');
    window.addEventListener('easyflowhub:voice-open-inbox', openInbox);
    return () => window.removeEventListener('easyflowhub:voice-open-inbox', openInbox);
  }, []);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-1 border-b border-[color:var(--manager-border)] px-3 py-2 sm:px-4" role="tablist" aria-label="笔记工作区">
        <button type="button" role="tab" aria-selected={activeView === 'inbox'} onClick={() => setActiveView('inbox')} className={`rounded-[7px] px-3 py-1.5 text-sm transition ${activeView === 'inbox' ? 'bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]' : 'text-[color:var(--manager-ink-muted)] hover:bg-[color:var(--manager-panel)]'}`}>收集箱</button>
        <button type="button" role="tab" aria-selected={activeView === 'notes'} onClick={() => setActiveView('notes')} className={`rounded-[7px] px-3 py-1.5 text-sm transition ${activeView === 'notes' ? 'bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]' : 'text-[color:var(--manager-ink-muted)] hover:bg-[color:var(--manager-panel)]'}`}>笔记</button>
      </div>
      <div className="min-h-0 flex-1">
        {activeView === 'inbox' ? <InboxPanel /> : <NotesArchivePanel />}
      </div>
    </div>
  );
}
