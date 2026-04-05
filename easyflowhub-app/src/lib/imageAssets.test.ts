import { describe, expect, it } from 'vitest';
import {
  appendAttachedImageAssets,
  extractAttachedImageAssets,
  normalizeMarkdownAssetSources,
} from './imageAssets';

describe('imageAssets', () => {
  it('normalizes full asset URLs to short asset syntax', () => {
    const content = '![tmp](http://asset.localhost/C%3A%5Cfoo%5Cassets%5Cabc.png)';
    expect(normalizeMarkdownAssetSources(content)).toBe('![tmp](asset:abc.png)');
  });

  it('extracts standalone image lines from content', () => {
    const content = 'hello\n\n![tmp](asset:abc.png)\n![other](asset:def.png)';
    expect(extractAttachedImageAssets(content)).toEqual({
      textContent: 'hello\n',
      images: [
        { alt: 'tmp', filename: 'abc.png' },
        { alt: 'other', filename: 'def.png' },
      ],
    });
  });

  it('appends attached images after text content', () => {
    const result = appendAttachedImageAssets('hello', [{ alt: 'tmp', filename: 'abc.png' }]);
    expect(result).toBe('hello\n\n![tmp](asset:abc.png)');
  });
});
