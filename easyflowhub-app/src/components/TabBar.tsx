import type { Tab } from '../types';

interface TabBarProps {
  tabs: Tab[];
  activeTabId: string | null;
  onTabSelect: (tabId: string) => void;
  onTabClose: (tabId: string) => void;
  onNewTab: () => void;
}

export function TabBar({
  tabs,
  activeTabId,
  onTabSelect,
  onTabClose,
  onNewTab,
}: TabBarProps) {
  return (
    <div
      className="flex items-center border-b border-white/10 shrink-0"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.15)' }}
      data-tauri-drag-region
    >
      {/* Tabs */}
      <div className="flex-1 flex items-center overflow-x-auto">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            className={`group flex items-center gap-2 px-3 py-2 cursor-pointer border-r border-white/10 min-w-0 max-w-48 ${
              activeTabId === tab.id
                ? 'bg-white/5'
                : 'opacity-60 hover:opacity-100 hover:bg-white/5'
            }`}
            onClick={() => onTabSelect(tab.id)}
          >
            {/* Dirty indicator */}
            {tab.isDirty && (
              <span className="w-2 h-2 rounded-full bg-blue-500 flex-shrink-0" />
            )}
            {/* Tab title */}
            <span className="truncate text-sm">{tab.title || '新笔记'}</span>
            {/* Close button */}
            <button
              onClick={(e) => {
                e.stopPropagation();
                onTabClose(tab.id);
              }}
              className="opacity-0 group-hover:opacity-100 flex-shrink-0 hover:text-red-400"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        ))}
      </div>

      {/* New tab button */}
      <button
        onClick={onNewTab}
        className="opacity-60 hover:opacity-100 hover:bg-white/5 px-3 py-2 flex-shrink-0"
        title="新建标签页 (Ctrl+N)"
      >
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
        </svg>
      </button>
    </div>
  );
}
