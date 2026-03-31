// src/components/beautification/PreviewPanel.tsx
import type { AppearanceConfig } from '../../types/appearance';

interface PreviewPanelProps {
  config: AppearanceConfig;
  resolvedTheme: 'light' | 'dark';
}

export function PreviewPanel({ config, resolvedTheme }: PreviewPanelProps) {
  const { window: windowConfig, font } = config;

  const previewStyle: React.CSSProperties = {
    backgroundColor:
      resolvedTheme === 'dark'
        ? `rgba(17, 24, 39, ${windowConfig.opacity})`
        : `rgba(255, 255, 255, ${windowConfig.opacity})`,
    backdropFilter: windowConfig.isBlurEnabled ? 'blur(10px)' : 'none',
    borderRadius: `${windowConfig.cornerRadius}px`,
    color: resolvedTheme === 'dark' ? '#e5e7eb' : '#1f2937',
  border: '1px solid rgba(255, 255, 255, 0.1)',
  padding: '24px',
  transition: 'all 0.3s ease',
  boxShadow: resolvedTheme === 'dark'
    ? '0 4px 6px -1px rgba(0, 0, 0, 0.3)'
    : '0 4px 6px -1px rgba(0, 0, 0, 0.1)',
  };

  const editorStyle: React.CSSProperties = {
    fontFamily: font.fontFamily,
    fontSize: `${font.fontSize}px`,
    lineHeight: font.lineHeight,
  };

  return (
    <section>
      <h2 className="text-sm font-medium text-gray-300 mb-4">预览</h2>
      <div
        className="p-4 rounded-lg border border-white/10 bg-white/5"
        style={{ borderRadius: `${windowConfig.cornerRadius}px` }}
      >
        <div style={previewStyle}>
          <div className="text-sm mb-2 opacity-70">窗口预览</div>
          <div
            className="p-4 rounded"
            style={{
              backgroundColor: resolvedTheme === 'dark' ? 'rgba(0, 0, 0, 0.2)' : 'rgba(0, 0, 0, 0.05)',
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
