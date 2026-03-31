import { describe, it, expect } from 'vitest';
import {
  deriveDisplayTitle,
  truncateTitle,
  parseNoteContent,
  isEmptyContent,
  buildNoteContent,
  formatNoteForDisplay,
} from './noteParser';

describe('noteParser', () => {
  // ==================== deriveDisplayTitle ====================
  describe('deriveDisplayTitle', () => {
    it('should return trimmed title when title is provided', () => {
      expect(deriveDisplayTitle('  My Title  ', 'content')).toBe('My Title');
    });

    it('should return first meaningful line when title is empty', () => {
      const content = 'First Line\nSecond Line\nThird Line';
      expect(deriveDisplayTitle('', content)).toBe('First Line');
    });

    it('should strip markdown headers from first line', () => {
      const content = '# Header Title\nSome content';
      expect(deriveDisplayTitle('', content)).toBe('Header Title');
    });

    it('should return empty string when both title and content are empty', () => {
      expect(deriveDisplayTitle('', '')).toBe('');
    });

    it('should skip empty lines when deriving from content', () => {
      const content = '\n\n   \nActual Title\nMore content';
      expect(deriveDisplayTitle('', content)).toBe('Actual Title');
    });
  });

  // ==================== truncateTitle ====================
  describe('truncateTitle', () => {
    it('should return title as-is when shorter than maxLength', () => {
      expect(truncateTitle('Short', 30)).toBe('Short');
    });

    it('should return title as-is when equal to maxLength', () => {
      expect(truncateTitle('123456789012345678901234567890', 30)).toBe('123456789012345678901234567890');
    });

    it('should truncate with ellipsis when longer than maxLength', () => {
      expect(truncateTitle('This is a very long title', 10)).toBe('This is a…');
    });

    it('should use default maxLength of 30', () => {
      const longTitle = 'A'.repeat(50);
      expect(truncateTitle(longTitle).length).toBe(30);
    });

    it('should return empty string for empty title', () => {
      expect(truncateTitle('', 20)).toBe('');
    });
  });

  // ==================== parseNoteContent ====================
  describe('parseNoteContent', () => {
    it('should return default values for empty content', () => {
      const result = parseNoteContent('');
      expect(result).toEqual({
        title: '',
        content: '',
        tags: '',
        cleanContent: '',
      });
    });

    it('should return default values for whitespace-only content', () => {
      const result = parseNoteContent('   \n\t\n   ');
      expect(result.title).toBe('');
      expect(result.cleanContent).toBe('');
    });

    it('should parse title from first line', () => {
      const result = parseNoteContent('My Title\nSome content here');
      expect(result.title).toBe('My Title');
    });

    it('should strip markdown headers from title', () => {
      const result = parseNoteContent('# Main Title\nContent');
      expect(result.title).toBe('Main Title');
    });

    it('should handle multiple hashes in title', () => {
      const result = parseNoteContent('## Section Title\nContent');
      expect(result.title).toBe('Section Title');
    });

    it('should extract tags with # format', () => {
      const result = parseNoteContent('Title\nContent line\n# work important');
      expect(result.tags).toBe('work important');
    });

    it('should extract tags with : format', () => {
      const result = parseNoteContent('Title\nContent\n:work:important:urgent');
      expect(result.tags).toBe('work important urgent');
    });

    it('should handle tags without spaces', () => {
      const result = parseNoteContent('Title\nContent\n#物联网iot');
      expect(result.tags).toBe('物联网iot');
    });

    it('should exclude tag line from cleanContent', () => {
      const result = parseNoteContent('Title\nContent\n#work');
      expect(result.cleanContent).toBe('Title\nContent');
    });

    it('should handle inline # tags', () => {
      const result = parseNoteContent('Title\nSome content');
      // No tag line, so content stays the same
      expect(result.cleanContent).toBe('Title\nSome content');
    });

    it('should preserve content structure', () => {
      const content = 'Line 1\nLine 2\nLine 3';
      const result = parseNoteContent(content);
      expect(result.cleanContent).toBe(content);
    });

    it('should preserve trailing whitespace in content', () => {
      const result = parseNoteContent('Title\nContent  \n  \n');
      expect(result.cleanContent).toBe('Title\nContent  \n  \n');
    });

    it('should handle title with only markdown header', () => {
      const result = parseNoteContent('# Only Title');
      expect(result.title).toBe('Only Title');
      expect(result.cleanContent).toBe('');
    });

    it('should handle : format tags inline after content', () => {
      // ":work:important" format - starts with :
      const result = parseNoteContent('Title\nContent\n:work:important');
      expect(result.tags).toBe('work important');
    });
  });

  // ==================== isEmptyContent ====================
  describe('isEmptyContent', () => {
    it('should return true for empty string', () => {
      expect(isEmptyContent('')).toBe(true);
    });

    it('should return true for whitespace only', () => {
      expect(isEmptyContent('   \t\n  ')).toBe(true);
    });

    it('should return false for non-empty content', () => {
      expect(isEmptyContent('Some text')).toBe(false);
    });

    it('should return false for content with leading/trailing whitespace', () => {
      expect(isEmptyContent('  content  ')).toBe(false);
    });
  });

  // ==================== buildNoteContent ====================
  describe('buildNoteContent', () => {
    it('should return cleanContent when no tags', () => {
      const parsed = {
        title: 'Title',
        content: 'Content',
        tags: '',
        cleanContent: 'Content',
      };
      expect(buildNoteContent(parsed)).toBe('Content');
    });

    it('should append # format tags', () => {
      const parsed = {
        title: 'Title',
        content: 'Content',
        tags: 'work important',
        cleanContent: 'Content',
      };
      const result = buildNoteContent(parsed);
      expect(result).toContain('# work important');
    });

    it('should handle multiple tags', () => {
      const parsed = {
        title: 'Title',
        content: 'Content',
        tags: 'tag1 tag2 tag3',
        cleanContent: 'Content',
      };
      const result = buildNoteContent(parsed);
      expect(result).toBe('Content\n# tag1 tag2 tag3');
    });

    it('should handle empty cleanContent with tags', () => {
      const parsed = {
        title: '',
        content: '',
        tags: 'work',
        cleanContent: '',
      };
      const result = buildNoteContent(parsed);
      expect(result).toBe('\n# work');
    });
  });

  // ==================== formatNoteForDisplay ====================
  describe('formatNoteForDisplay', () => {
    it('should return content as-is when no tags', () => {
      const result = formatNoteForDisplay({ content: 'Some content', tags: '' });
      expect(result).toBe('Some content');
    });

    it('should append # format tags', () => {
      const result = formatNoteForDisplay({ content: 'Content', tags: 'work important' });
      expect(result).toBe('Content\n# work important');
    });

    it('should use : format when tags contain colon', () => {
      const result = formatNoteForDisplay({ content: 'Content', tags: 'work:important' });
      expect(result).toBe('Content\n:work:important');
    });

    it('should handle multiple tags', () => {
      const result = formatNoteForDisplay({ content: 'Content', tags: 'tag1 tag2 tag3' });
      expect(result).toBe('Content\n# tag1 tag2 tag3');
    });

    it('should handle empty content with tags', () => {
      const result = formatNoteForDisplay({ content: '', tags: 'work' });
      expect(result).toBe('\n# work');
    });
  });
});
