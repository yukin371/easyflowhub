import { PageType } from '../types/pages';
import { NotesPage } from './NotesPage';
import { ScriptsPage } from '../features/scripts/ScriptsPage';
import { ManagerPage } from './manager';
import { BeautificationPage } from './beautification';

interface PageRouterProps {
  currentPage: PageType;
}

export function PageRouter({ currentPage }: PageRouterProps) {
  switch (currentPage) {
    case 'notes':
      return <NotesPage />;
    case 'scripts':
      return <ScriptsPage />;
    case 'manager':
      return <ManagerPage />;
    case 'beautification':
      return <BeautificationPage />;
    case 'search':
      return (
        <div className="h-full flex items-center justify-center bg-gray-900/80 text-gray-500">
          <div className="text-center">
            <div className="text-4xl mb-2">🔍</div>
            <p>搜索功能开发中...</p>
            <p className="text-sm mt-1">将集成 Everything 搜索</p>
          </div>
        </div>
      );
    case 'settings':
      return (
        <div className="h-full flex items-center justify-center bg-gray-900/80 text-gray-500">
          <div className="text-center">
            <div className="text-4xl mb-2">⚙️</div>
            <p>设置功能开发中...</p>
          </div>
        </div>
      );
    default:
      return <NotesPage />;
  }
}
