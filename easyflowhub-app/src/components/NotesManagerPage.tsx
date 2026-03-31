import { useState } from 'react';
import type { Note } from '../types/note';

export function NotesManagerPage() {
  const [selectedNotes, setSelectedNotes] = useState<string[]>([]);
  const [viewMode, setViewMode] = useState<'list' | 'grid' | 'timeline'>('list');
  const [searchQuery, setSearchQuery] = useState('');

  // Mock data for demo
  const notes: Note[] = [
    {
      id: '1',
      title: '工作笔记',
      content: '这是工作相关的笔记内容...',
      tags: '',
      created_at: '2026-03-17T10:00:00Z',
      updated_at: '2026-03-17T14:30:00Z',
      is_pinned: true,
      deleted_at: null,
    },
    {
      id: '2',
      title: '学习笔记',
      content: '这是学习相关的笔记内容...',
      tags: '',
      created_at: '2026-03-16T09:00:00Z',
      updated_at: '2026-03-16T15:00:00Z',
      is_pinned: false,
      deleted_at: null,
    },
    {
      id: '3',
      title: '生活笔记',
      content: '这是生活相关的笔记内容...',
      tags: '',
      created_at: '2026-03-15T18:00:00Z',
      updated_at: '2026-03-15T19:00:00Z',
      is_pinned: false,
      deleted_at: null,
    },
  ];

  const handleNoteSelect = (noteId: string) => {
    setSelectedNotes((prev) => {
      if (prev.includes(noteId)) {
        return prev.filter((id) => id !== noteId);
      } else {
        return [...prev, noteId];
      }
    });
  };

  const handleSelectAll = () => {
    if (selectedNotes.length === notes.length) {
      setSelectedNotes([]);
    } else {
      setSelectedNotes(notes.map((note) => note.id));
    }
  };

  const handleDeleteSelected = () => {
    // Implement delete logic
    console.log('Delete selected notes:', selectedNotes);
    setSelectedNotes([]);
  };

  const handleExportSelected = () => {
    // Implement export logic
    console.log('Export selected notes:', selectedNotes);
  };

  return (
    <div className="h-full flex flex-col bg-gray-900/80">
      {/* Header */}
      <div className="p-4 border-b border-white/10 flex items-center justify-between">
        <h1 className="text-lg font-semibold text-gray-200">笔记管理</h1>
        <div className="flex items-center gap-2">
          <button
            className="px-3 py-1 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 transition-colors"
          >
            导入
          </button>
          <button
            className="px-3 py-1 bg-green-600 text-white text-sm rounded hover:bg-green-700 transition-colors"
          >
            导出
          </button>
        </div>
      </div>

      {/* Toolbar */}
      <div className="p-3 border-b border-white/10 flex items-center gap-4">
        {/* View mode */}
        <div className="flex items-center gap-2">
          <button
            className={`p-2 rounded ${viewMode === 'list' ? 'bg-white/10' : 'text-gray-400'}`}
            onClick={() => setViewMode('list')}
            title="列表视图"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 10h16M4 14h16M4 18h16" />
            </svg>
          </button>
          <button
            className={`p-2 rounded ${viewMode === 'grid' ? 'bg-white/10' : 'text-gray-400'}`}
            onClick={() => setViewMode('grid')}
            title="网格视图"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
            </svg>
          </button>
          <button
            className={`p-2 rounded ${viewMode === 'timeline' ? 'bg-white/10' : 'text-gray-400'}`}
            onClick={() => setViewMode('timeline')}
            title="时间线视图"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
            </svg>
          </button>
        </div>

        {/* Search */}
        <div className="flex-1 relative">
          <svg
            className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            type="text"
            placeholder="搜索笔记..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-9 pr-3 py-1.5 bg-white/5 border border-white/10 rounded-lg text-sm text-gray-200 placeholder-gray-400 focus:outline-none focus:border-blue-500"
          />
        </div>

        {/* Sort */}
        <select className="px-3 py-1.5 bg-white/5 border border-white/10 rounded-lg text-sm text-gray-200 focus:outline-none focus:border-blue-500">
          <option>更新时间</option>
          <option>创建时间</option>
          <option>标题</option>
          <option>置顶</option>
        </select>
      </div>

      {/* Main content */}
      <div className="flex-1 overflow-auto p-4">
        {viewMode === 'list' ? (
          <div className="space-y-2">
            {/* Select all header */}
            <div className="flex items-center gap-3 p-3 bg-white/5 rounded-lg">
              <input
                type="checkbox"
                checked={selectedNotes.length === notes.length && notes.length > 0}
                onChange={handleSelectAll}
                className="w-4 h-4 rounded border-gray-600 bg-white/10 text-blue-600 focus:ring-blue-500"
              />
              <div className="text-sm font-medium text-gray-300">全选</div>
              <div className="text-sm text-gray-400 ml-auto">
                共 {notes.length} 个笔记
              </div>
            </div>

            {/* Notes list */}
            {notes.map((note) => (
              <div
                key={note.id}
                className="flex items-start gap-3 p-3 bg-white/5 rounded-lg hover:bg-white/10 transition-colors border border-white/5 hover:border-white/15"
              >
                <input
                  type="checkbox"
                  checked={selectedNotes.includes(note.id)}
                  onChange={() => handleNoteSelect(note.id)}
                  className="w-4 h-4 rounded border-gray-600 bg-white/10 text-blue-600 focus:ring-blue-500 mt-1"
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    {note.is_pinned && (
                      <svg className="w-4 h-4 text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                      </svg>
                    )}
                    <h3 className="text-sm font-medium text-gray-200 truncate">
                      {note.title}
                    </h3>
                  </div>
                  <p className="text-xs text-gray-400 truncate mb-2">
                    {note.content.substring(0, 100)}
                    {note.content.length > 100 && '...'}
                  </p>
                  <div className="flex items-center gap-4 text-xs text-gray-500">
                    <span>更新: {new Date(note.updated_at).toLocaleString()}</span>
                    <span>{note.content.length} 字符</span>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button className="p-1 text-gray-400 hover:text-gray-200">
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                    </svg>
                  </button>
                  <button className="p-1 text-gray-400 hover:text-red-400">
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : viewMode === 'grid' ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
            {notes.map((note) => (
              <div
                key={note.id}
                className="p-4 bg-white/5 rounded-lg hover:bg-white/10 transition-colors border border-white/5 hover:border-white/15"
              >
                <div className="flex items-start justify-between mb-2">
                  {note.is_pinned && (
                    <svg className="w-4 h-4 text-yellow-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                    </svg>
                  )}
                  <input
                    type="checkbox"
                    checked={selectedNotes.includes(note.id)}
                    onChange={() => handleNoteSelect(note.id)}
                    className="w-4 h-4 rounded border-gray-600 bg-white/10 text-blue-600 focus:ring-blue-500"
                  />
                </div>
                <h3 className="text-sm font-medium text-gray-200 mb-2 truncate">
                  {note.title}
                </h3>
                <p className="text-xs text-gray-400 mb-3 line-clamp-3">
                  {note.content}
                </p>
                <div className="text-xs text-gray-500">
                  {new Date(note.updated_at).toLocaleDateString()}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="space-y-6">
            {notes.map((note) => (
              <div key={note.id} className="relative pl-8 pb-6 border-l-2 border-white/10">
                <div className="absolute left-[-9px] top-0 w-4 h-4 rounded-full bg-blue-600 border-2 border-gray-900" />
                <div className="bg-white/5 rounded-lg p-4 hover:bg-white/10 transition-colors">
                  <div className="flex items-center gap-2 mb-2">
                    {note.is_pinned && (
                      <svg className="w-4 h-4 text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                      </svg>
                    )}
                    <h3 className="text-sm font-medium text-gray-200">
                      {note.title}
                    </h3>
                  </div>
                  <p className="text-xs text-gray-400 mb-2">
                    {note.content.substring(0, 150)}
                    {note.content.length > 150 && '...'}
                  </p>
                  <div className="text-xs text-gray-500">
                    {new Date(note.updated_at).toLocaleString()}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Action bar */}
      {selectedNotes.length > 0 && (
        <div className="p-3 border-t border-white/10 bg-black/30 flex items-center justify-between">
          <div className="text-sm text-gray-300">
            已选择 {selectedNotes.length} 项
          </div>
          <div className="flex items-center gap-2">
            <button
              className="px-3 py-1 bg-red-600 text-white text-sm rounded hover:bg-red-700 transition-colors"
              onClick={handleDeleteSelected}
            >
              删除
            </button>
            <button
              className="px-3 py-1 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 transition-colors"
              onClick={handleExportSelected}
            >
              导出
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
