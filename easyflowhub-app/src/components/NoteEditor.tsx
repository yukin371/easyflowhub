import { useRef, useEffect } from 'react';

interface NoteEditorProps {
  content: string;
  onChange: (content: string) => void;
  title?: string;
}

export function NoteEditor({ content, onChange, title: _title }: NoteEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-focus on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Editor area */}
      <textarea
        ref={textareaRef}
        value={content}
        onChange={(e) => onChange(e.target.value)}
        placeholder="开始输入...&#10;&#10;# 标题&#10;- 列表项&#10;```代码块```&#10;[链接](url)"
        className="flex-1 w-full p-4 resize-none outline-none text-base bg-transparent"
        style={{
          lineHeight: '1.6',
          color: 'inherit'
        }}
      />

      {/* Status bar */}
      <div
        className="px-4 py-1.5 border-t border-white/10 text-xs text-gray-400 flex items-center justify-between shrink-0"
        style={{ backgroundColor: 'rgba(0, 0, 0, 0.2)' }}
      >
        <div className="flex items-center gap-3">
          <span>{content.length} 字符</span>
          <span>•</span>
          <span>{content.split('\n').length} 行</span>
        </div>
        <div>Markdown · 自动保存</div>
      </div>
    </div>
  );
}
