// src/components/beautification/PreviewPanel.tsx
import type { AppearanceConfig } from '../../types/appearance';

interface PreviewPanelProps {
  config: AppearanceConfig;
  resolvedTheme: 'light' | 'dark';
}

export function PreviewPanel({ config }: PreviewPanelProps) {
  const { window: windowConfig, font } = config;

  const previewStyle: React.CSSProperties = {
    backgroundColor: `color-mix(in srgb, var(--manager-panel-strong) ${Math.round(windowConfig.opacity * 100)}%, transparent)`,
    backdropFilter: windowConfig.isBlurEnabled ? 'blur(10px)' : 'none',
    borderRadius: `${windowConfig.cornerRadius}px`,
    color: 'var(--manager-ink)',
    border: '1px solid var(--manager-border)',
    padding: '24px',
    transition: 'all 0.3s ease',
    boxShadow: 'var(--manager-shadow)',
  };

  const editorStyle: React.CSSProperties = {
    fontFamily: font.fontFamily,
    fontSize: `${font.fontSize}px`,
    lineHeight: font.lineHeight,
  };

  return (
    <section>
      <h2 className="mb-4 text-sm font-medium text-[color:var(--manager-ink-strong)]">预览</h2>
      <div
        className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4"
        style={{ borderRadius: `${windowConfig.cornerRadius}px` }}
      >
        <div style={previewStyle}>
          <div className="text-sm mb-2 opacity-70">窗口预览</div>
          <div
            className="p-4 rounded"
            style={{
              backgroundColor: 'var(--manager-accent-soft)',
              ...editorStyle
            }}
          >
            <h3 className="font-medium mb-2">示例笔记</h3>
            <p className="opacity-70">
              这是一个示例笔记，用于预览字体和窗口设置效果。
              <br />
              字体: {font.fontFamily}
              <br />
              大小: {font.fontSize}px | 行高: {font.lineHeight.toFixed(1)}
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
