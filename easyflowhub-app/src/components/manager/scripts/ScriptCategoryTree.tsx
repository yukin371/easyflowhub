/**
 * ScriptCategoryTree - 分类侧边栏
 */

import type { CategoryInfo } from '../../../lib/api/scriptmgr';

interface ScriptCategoryTreeProps {
  categories: CategoryInfo[];
  selectedCategory: string | null;
  onSelect: (category: string | null) => void;
}

const CATEGORY_ICONS: Record<string, string> = {
  utils: '工',
  media: '媒',
  dev: '码',
  system: '系',
  uncategorized: '藏',
};

export function ScriptCategoryTree({
  categories,
  selectedCategory,
  onSelect,
}: ScriptCategoryTreeProps) {
  const totalCount = categories.reduce((sum, c) => sum + c.count, 0);

  return (
    <nav className="mt-2 space-y-1">
      {/* All Scripts */}
      <button
        onClick={() => onSelect(null)}
        className={`group flex w-full items-center gap-2 rounded-[12px] border px-2.5 py-2 text-left transition-all ${
          selectedCategory === null
            ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
            : 'border-transparent bg-transparent text-[color:var(--manager-ink-muted)] hover:border-[color:var(--manager-border)] hover:bg-white/55'
        }`}
      >
        <span
          className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-[10px] border text-xs ${
            selectedCategory === null
              ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-white'
              : 'border-[color:var(--manager-border)] bg-white/70 text-[color:var(--manager-ink-soft)]'
          }`}
        >
          全
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-sm font-medium text-current">全部</span>
        </span>
        <span className="text-xs text-[color:var(--manager-ink-subtle)]">{totalCount}</span>
      </button>

      {/* Category Items */}
      {categories.map((category) => {
        const isActive = selectedCategory === category.name;
        const icon = CATEGORY_ICONS[category.name] || '件';

        return (
          <button
            key={category.name}
            onClick={() => onSelect(category.name)}
            className={`group flex w-full items-center gap-2 rounded-[12px] border px-2.5 py-2 text-left transition-all ${
              isActive
                ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                : 'border-transparent bg-transparent text-[color:var(--manager-ink-muted)] hover:border-[color:var(--manager-border)] hover:bg-white/55'
            }`}
          >
            <span
              className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-[10px] border text-xs ${
                isActive
                  ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-white'
                  : 'border-[color:var(--manager-border)] bg-white/70 text-[color:var(--manager-ink-soft)]'
              }`}
            >
              {icon}
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm font-medium text-current">
                {category.name === 'uncategorized' ? '未分类' : category.name}
              </span>
            </span>
            <span className="text-xs text-[color:var(--manager-ink-subtle)]">{category.count}</span>
          </button>
        );
      })}
    </nav>
  );
}
