import { useNotes } from '../hooks/useNotes';
import { NoteEditor } from './NoteEditor';
import { TabBar } from './TabBar';

export function NotesPage() {
  const {
    tabs,
    activeTabId,
    activeNote,
    isLoading,
    handleNewTab,
    handleTabSelect,
    handleTabClose,
    updateNoteContent,
  } = useNotes();

  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="w-5 h-5 border-2 border-gray-500/30 border-t-gray-400 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Tab bar */}
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onTabSelect={handleTabSelect}
        onTabClose={handleTabClose}
        onNewTab={handleNewTab}
      />

      {/* Note editor */}
      <div className="flex-1 min-h-0 overflow-hidden">
        <NoteEditor
          content={activeNote?.content || ''}
          onChange={updateNoteContent}
          title={activeNote?.title || '新笔记'}
        />
      </div>
    </div>
  );
}
