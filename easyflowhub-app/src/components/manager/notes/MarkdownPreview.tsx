/**
 * MarkdownPreview - Markdown 渲染预览
 * 手动渲染 checkbox 以精确追踪行号，其余内容用 markdown-to-jsx
 */

import { Fragment } from 'react';
import { useEffect, useState } from 'react';
import MDEditor from '@uiw/react-md-editor';
import { normalizeMarkdownAssetSources, resolveMarkdownAssetSources } from '../../../lib/imageAssets';
import { ImageLightbox } from '../../shared/ImageLightbox';

interface MarkdownPreviewProps {
  content: string;
  onToggleCheckbox?: (lineIndex: number) => void;
}

interface Segment {
  type: 'markdown' | 'todo';
  lineIndex: number;
  text: string;
  checked: boolean;
  indent: string;
  bullet: string;
}

function preserveMarkdownSoftBreaks(content: string): string {
  const lines = content.split('\n');
  let inCodeFence = false;

  return lines
    .map((line, index) => {
      const trimmed = line.trimStart();
      if (trimmed.startsWith('```')) {
        inCodeFence = !inCodeFence;
        return line;
      }

      const isLastLine = index === lines.length - 1;
      const nextLine = isLastLine ? '' : lines[index + 1];
      if (inCodeFence || isLastLine || !line.trim() || !nextLine.trim()) {
        return line;
      }

      return `${line}  `;
    })
    .join('\n');
}

/**
 * 将内容拆分为 markdown 段落和 todo checkbox 段落
 */
function parseSegments(content: string): Segment[] {
  const lines = content.split('\n');
  const segments: Segment[] = [];
  let markdownBuffer: string[] = [];

  for (let i = 0; i < lines.length; i++) {
    const match = /^(\s*)([-*])\s*\[([ xX])\]\s*(.*)$/.exec(lines[i]);
    if (match) {
      // Flush markdown buffer
      if (markdownBuffer.length > 0) {
        segments.push({
          type: 'markdown',
          lineIndex: -1,
          text: preserveMarkdownSoftBreaks(markdownBuffer.join('\n')),
          checked: false,
          indent: '',
          bullet: '',
        });
        markdownBuffer = [];
      }
      segments.push({
        type: 'todo',
        lineIndex: i,
        text: match[4],
        checked: match[3].toLowerCase() === 'x',
        indent: match[1],
        bullet: match[2],
      });
    } else {
      markdownBuffer.push(lines[i]);
    }
  }

  // Flush remaining markdown
  if (markdownBuffer.length > 0) {
    segments.push({
      type: 'markdown',
      lineIndex: -1,
      text: preserveMarkdownSoftBreaks(markdownBuffer.join('\n')),
      checked: false,
      indent: '',
      bullet: '',
    });
  }

  return segments;
}

export function MarkdownPreview({ content, onToggleCheckbox }: MarkdownPreviewProps) {
  const normalizedContent = normalizeMarkdownAssetSources(content);
  const [resolvedContent, setResolvedContent] = useState(normalizedContent);
  const [activeImage, setActiveImage] = useState<{ src: string; alt: string } | null>(null);

  useEffect(() => {
    let cancelled = false;

    const resolveContent = async () => {
      const next = await resolveMarkdownAssetSources(normalizedContent);
      if (!cancelled) {
        setResolvedContent(next);
      }
    };

    void resolveContent();
    return () => {
      cancelled = true;
    };
  }, [normalizedContent]);

  const segments = parseSegments(resolvedContent);

  return (
    <>
      <div
        className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[15px] leading-8 text-[color:var(--manager-ink)]"
        onDoubleClick={(event) => {
          const target = event.target;
          if (!(target instanceof HTMLImageElement) || !target.src) {
            return;
          }

          setActiveImage({ src: target.src, alt: target.alt });
        }}
      >
        {segments.map((seg, idx) => {
          if (seg.type === 'todo') {
            return (
              <div
                key={`todo-${seg.lineIndex}`}
                className="flex items-start gap-2 py-0.5"
                style={{ paddingLeft: `${seg.indent.length * 12}px` }}
              >
                <input
                  type="checkbox"
                  checked={seg.checked}
                  onChange={() => onToggleCheckbox?.(seg.lineIndex)}
                  className="mt-1.5 h-4 w-4 shrink-0 cursor-pointer rounded border-[color:var(--manager-border)] accent-[color:var(--manager-accent)]"
                />
                <span className={seg.checked ? 'text-[color:var(--manager-ink-muted)] line-through' : ''}>
                  {seg.text}
                </span>
              </div>
            );
          }

          // Markdown block
          if (!seg.text.trim()) return <Fragment key={`md-${idx}`} />;

          return (
            <div
              key={`md-${idx}`}
              className="manager-markdown-preview prose prose-sm max-w-none [&_h1]:text-2xl [&_h1]:font-bold [&_h1]:text-[color:var(--manager-ink-strong)] [&_h2]:text-xl [&_h2]:font-semibold [&_h2]:text-[color:var(--manager-ink-strong)] [&_h3]:text-lg [&_h3]:font-semibold [&_h3]:text-[color:var(--manager-ink-strong)] [&_code]:rounded [&_code]:bg-black/5 [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-sm [&_pre]:rounded-xl [&_pre]:bg-black/5 [&_pre]:p-4 [&_blockquote]:border-l-2 [&_blockquote]:border-[color:var(--manager-accent)] [&_blockquote]:pl-4 [&_blockquote]:italic [&_blockquote]:text-[color:var(--manager-ink-soft)] [&_a]:text-[color:var(--manager-accent)] [&_a]:underline [&_img]:cursor-zoom-in [&_img]:rounded-lg [&_img]:shadow-sm [&_ul]:list-disc [&_ul]:pl-6 [&_ol]:list-decimal [&_ol]:pl-6 [&_hr]:border-[color:var(--manager-border)]"
            >
              <MDEditor.Markdown source={seg.text} style={{ backgroundColor: 'transparent', color: 'inherit' }} />
            </div>
          );
        })}
      </div>
      {activeImage ? (
        <ImageLightbox src={activeImage.src} alt={activeImage.alt} onClose={() => setActiveImage(null)} />
      ) : null}
    </>
  );
}
