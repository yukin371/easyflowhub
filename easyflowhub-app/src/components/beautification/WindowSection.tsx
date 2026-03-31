// src/components/beautification/WindowSection.tsx
import type { WindowAppearance } from '../../types/appearance';

interface WindowSectionProps {
  window: WindowAppearance;
  onChange: (updates: Partial<WindowAppearance>) => void;
}

export function WindowSection({ window: windowConfig, onChange }: WindowSectionProps) {
  return (
    <section>
      <h2 className="text-sm font-medium text-gray-300 mb-4">窗口外观</h2>
      <div className="space-y-4">
        {/* 透明度 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">透明度</label>
            <span className="text-sm text-gray-400">{Math.round(windowConfig.opacity * 100)}%</span>
          </div>
          <input
            type="range"
            min="0.3"
            max="1"
            step="0.05"
            value={windowConfig.opacity}
            onChange={(e) => onChange({ opacity: parseFloat(e.target.value) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>

        {/* 圆角大小 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">圆角大小</label>
            <span className="text-sm text-gray-400">{windowConfig.cornerRadius}px</span>
          </div>
          <input
            type="range"
            min="0"
            max="20"
            step="1"
            value={windowConfig.cornerRadius}
            onChange={(e) => onChange({ cornerRadius: parseInt(e.target.value, 10) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>

        {/* 开关选项 */}
        <div className="space-y-2">
          <label className="flex items-center gap-3 text-sm text-gray-300 cursor-pointer">
            <input
              type="checkbox"
              checked={windowConfig.isAlwaysOnTop}
              onChange={(e) => onChange({ isAlwaysOnTop: e.target.checked })}
              className="w-4 h-4 rounded border-gray-600 bg-white/10 text-blue-600 focus:ring-blue-500"
            />
            窗口置顶
          </label>
          <label className="flex items-center gap-3 text-sm text-gray-300 cursor-pointer">
            <input
              type="checkbox"
              checked={windowConfig.isBlurEnabled}
              onChange={(e) => onChange({ isBlurEnabled: e.target.checked })}
              className="w-4 h-4 rounded border-gray-600 bg-white/10 text-blue-600 focus:ring-blue-500"
            />
            模糊背景
          </label>
        </div>
      </div>
    </section>
  );
}
