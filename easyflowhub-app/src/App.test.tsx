import '@testing-library/jest-dom/vitest';
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App';

vi.mock('./pages/QuickNotePage', () => ({
  QuickNotePage: () => <div>Quick note surface</div>,
}));

vi.mock('./pages/TodoCardPage', () => ({
  TodoCardPage: () => <div>Todo card surface</div>,
}));

vi.mock('./components/manager', () => ({
  ManagerPage: () => <div>Manager surface</div>,
}));

vi.mock('./components/widget', () => ({
  FolderWidget: () => <div>Folder widget surface</div>,
}));

vi.mock('./pages/LauncherPage', () => ({
  LauncherPage: () => <div>Launcher surface</div>,
}));

vi.mock('./hooks/useAppearance', () => ({
  useAppearance: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(async () => ({ label: 'manager' })),
}));

describe('App window selection', () => {
  beforeEach(() => {
    window.history.replaceState({}, '', '/?force_window=todo-card');
  });

  it('renders the todo card from the preview query parameter', async () => {
    render(<App />);

    expect(await screen.findByText('Todo card surface')).toBeInTheDocument();
    expect(screen.queryByText('Quick note surface')).not.toBeInTheDocument();
  });
});
