import '@testing-library/jest-dom/vitest';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { MarkdownPreview } from './MarkdownPreview';

vi.mock('@uiw/react-md-editor', () => ({
  default: {
    Markdown: ({ source }: { source: string }) => <div data-testid="markdown-source">{source}</div>,
  },
}));

vi.mock('../../../lib/imageAssets', async () => {
  const actual = await vi.importActual<typeof import('../../../lib/imageAssets')>('../../../lib/imageAssets');
  return {
    ...actual,
    resolveMarkdownAssetSources: vi.fn(async (content: string) => content),
  };
});

describe('MarkdownPreview', () => {
  it('preserves soft line breaks so text after an image starts on a new line', async () => {
    render(
      <MarkdownPreview content={`第一列\n第二列文字尾部 ![tmp](asset:demo.png)\n第四列`} />
    );

    const rendered = await screen.findByTestId('markdown-source');
    expect(rendered.textContent).toContain('![tmp](asset:demo.png)  \n第四列');
  });
});
