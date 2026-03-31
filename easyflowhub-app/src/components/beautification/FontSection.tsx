// src/components/beautification/FontSection.tsx
import type { FontSettings } from '../../types/appearance';
import { AVAILABLE_FONTS } from '../../types/appearance';

interface FontSectionProps {
  font: FontSettings;
  onChange: (updates: Partial<FontSettings>) => void;
}

export function FontSection({ font, onChange }: FontSectionProps) {
  return (
    <section>
      <h2 className="text-sm font-medium text-gray-300 mb-4">字体设置</h2>
      <div className="space-y-4">
        {/* 字体选择 */}
        <div>
          <label className="block text-sm text-gray-400 mb-2">字体</label>
          <select
            value={font.fontFamily}
            onChange={(e) => onChange({ fontFamily: e.target.value })}
            className="w-full px-3 py-2 bg-white/5 border border-white/10 rounded-lg text-sm text-gray-200 focus:outline-none focus:border-blue-500"
          >
            {AVAILABLE_FONTS.map((fontName) => (
              <option key={fontName} value={fontName} style={{ fontFamily: fontName }}>
                {fontName}
              </option>
            ))}
          </select>
        </div>

        {/* 字体大小 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">字体大小</label>
            <span className="text-sm text-gray-400">{font.fontSize}px</span>
          </div>
          <input
            type="range"
            min="12"
            max="24"
            step="1"
            value={font.fontSize}
            onChange={(e) => onChange({ fontSize: parseInt(e.target.value, 10) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>

        {/* 行高 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-gray-400">行高</label>
            <span className="text-sm text-gray-400">{font.lineHeight.toFixed(1)}</span>
          </div>
          <input
            type="range"
            min="1"
            max="2"
            step="0.1"
            value={font.lineHeight}
            onChange={(e) => onChange({ lineHeight: parseFloat(e.target.value) })}
            className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>
      </div>
    </section>
  );
}
