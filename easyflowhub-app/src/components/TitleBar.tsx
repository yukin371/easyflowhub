import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

interface TitleBarProps {
  alwaysOnTop: boolean;
  onToggleAlwaysOnTop: () => void;
}

export function TitleBar({
  alwaysOnTop,
  onToggleAlwaysOnTop,
}: TitleBarProps) {
  const webviewWindow = getCurrentWebviewWindow();

  const handleMinimize = async () => {
    await webviewWindow.minimize();
  };

  const handleClose = async () => {
    await invoke('hide_window');
  };

  return (
    <div
      className="title-bar flex items-center justify-between px-3 select-none"
      data-tauri-drag-region
    >
      {/* Left side - App title */}
      <div className="flex items-center gap-2" data-tauri-drag-region>
        <span className="text-sm font-medium">EasyFlowHub</span>
      </div>

      {/* Center - Controls */}
      <div className="flex items-center gap-3">
        {/* Always on top toggle */}
        <button
          onClick={onToggleAlwaysOnTop}
          className={`text-xs px-2 py-1 rounded ${
            alwaysOnTop
              ? 'bg-blue-600 text-white'
              : 'opacity-60 hover:opacity-100 hover:bg-black/10'
          }`}
          title="置顶"
        >
          置顶
        </button>
      </div>

      {/* Right side - Window controls */}
      <div className="flex items-center">
        <button
          onClick={handleMinimize}
          className="opacity-60 hover:opacity-100 hover:bg-black/10 w-8 h-8 flex items-center justify-center"
          title="最小化"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
          </svg>
        </button>
        <button
          onClick={handleClose}
          className="opacity-60 hover:opacity-100 hover:bg-red-500 hover:text-white w-8 h-8 flex items-center justify-center"
          title="关闭"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  );
}
