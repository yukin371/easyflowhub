import { PageType, PAGE_CONFIGS } from '../types/pages';

interface NavBarProps {
  currentPage: PageType;
  onPageChange: (page: PageType) => void;
}

export function NavBar({ currentPage, onPageChange }: NavBarProps) {
  return (
    <nav className="flex items-center gap-1 px-2">
      {Object.values(PAGE_CONFIGS).map((config) => (
        <button
          key={config.type}
          onClick={() => onPageChange(config.type)}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-colors ${
            currentPage === config.type
              ? 'bg-blue-600 text-white'
              : 'opacity-60 hover:opacity-100 hover:bg-black/10'
          }`}
          title={config.description}
        >
          <span>{config.icon}</span>
          <span className="hidden sm:inline">{config.label}</span>
        </button>
      ))}
    </nav>
  );
}
