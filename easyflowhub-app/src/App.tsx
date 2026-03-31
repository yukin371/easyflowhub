import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { QuickNotePage } from './pages/QuickNotePage';
import { TodoCardPage } from './pages/TodoCardPage';
import { ManagerPage } from './components/manager';
import { FolderWidget } from './components/widget';
import './styles.css';

// 模块系统初始化
import './modules/init';

/** 窗口类型 */
type WindowType = 'quick-note' | 'manager' | 'folder-widget' | 'todo-card' | null;

/**
 * 检测当前窗口类型
 */
function useWindowType(): WindowType {
  const [windowType, setWindowType] = useState<WindowType>(null);

  useEffect(() => {
    const detectWindow = async () => {
      const params = new URLSearchParams(window.location.search);
      const forcedWindow = params.get('force_window');
      if (forcedWindow === 'manager') {
        setWindowType('manager');
        return;
      }
      if (forcedWindow === 'quick-note') {
        setWindowType('quick-note');
        return;
      }
      if (forcedWindow === 'folder-widget') {
        setWindowType('folder-widget');
        return;
      }

      try {
        const win = await getCurrentWindow();
        const label = win.label;

        if (label.startsWith('quick-note-')) {
          setWindowType('quick-note');
        } else if (label === 'manager') {
          setWindowType('manager');
        } else if (label.startsWith('folder-widget-')) {
          setWindowType('folder-widget');
        } else if (label.startsWith('todo-card-')) {
          setWindowType('todo-card');
        } else {
          // 检查 URL 参数（备用检测方式）
          const mode = params.get('mode');
          if (mode === 'widget') {
            setWindowType('folder-widget');
          } else if (mode === 'todo-card') {
            setWindowType('todo-card');
          } else if (mode === 'quick') {
            setWindowType('quick-note');
          } else {
            setWindowType('quick-note');
          }
        }
      } catch {
        setWindowType('quick-note');
      }
    };

    detectWindow();
  }, []);

  return windowType;
}

function App() {
  const windowType = useWindowType();

  // 加载中
  if (windowType === null) {
    return (
      <div style={{ width: '100vw', height: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', backgroundColor: 'transparent' }}>
        <div style={{ width: 20, height: 20, border: '2px solid rgba(255,255,255,0.2)', borderTopColor: '#3b82f6', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
      </div>
    );
  }

  // 根据窗口类型渲染对应页面
  if (windowType === 'manager') {
    return <ManagerPage />;
  }

  if (windowType === 'folder-widget') {
    return <FolderWidget />;
  }

  if (windowType === 'todo-card') {
    return <TodoCardPage />;
  }

  // 默认渲染快速笔记页面
  return <QuickNotePage />;
}

export default App;
