/**
 * 笔记解析工具
 * 处理标题、标签提取，内容清理等
 */

export interface ParsedNote {
  title: string;
  content: string;
  tags: string;
  // 清理后的内容（去除末尾空行/空格，不包含标签行）
  cleanContent: string;
}

export function normalizeStoredTitle(title: string): string {
  return title.trim();
}

export function deriveDisplayTitle(title: string, content: string): string {
  const normalizedTitle = normalizeStoredTitle(title);
  if (normalizedTitle) {
    return normalizedTitle;
  }

  const firstMeaningfulLine = content
    .split('\n')
    .map((line) => line.trim().replace(/^#+\s*/, ''))
    .find(Boolean);

  return firstMeaningfulLine ?? '';
}

/**
 * 截断标题以适应显示
 * @param title 原始标题
 * @param maxLength 最大长度，默认 30 个字符
 */
export function truncateTitle(title: string, maxLength: number = 30): string {
  if (!title || title.length <= maxLength) {
    return title;
  }
  return title.slice(0, maxLength - 1) + '…';
}

/**
 * 解析笔记内容，提取标题和标签
 * - 第一行为标题
 * - 最后一行如果是 # 或 : 开头的标签行，则解析为标签
 * - 去除末尾的空格和换行
 */
export function parseNoteContent(rawContent: string): ParsedNote {
  // 如果内容为空或只有空白字符，返回默认值
  if (!rawContent || /^\s*$/.test(rawContent)) {
    return {
      title: '',
      content: '',
      tags: '',
      cleanContent: '',
    };
  }

  // 保留原始内容格式，不做 trimEnd — 避免自动保存时清除用户正在编辑的尾部换行
  const content = rawContent;

  // 按行分割
  const lines = content.split('\n');

  // 提取标题（第一行，去除 # 前缀）
  let title = lines[0]?.trim() || '';
  // 移除标题开头的 #
  title = title.replace(/^#+\s*/, '');

  // 检查最后一行是否为标签行
  const lastLine = lines[lines.length - 1]?.trim() || '';
  let tags = '';
  let tagLineIndex = -1;

  // 标签行必须以 # 或 : 开头
  // # 标签: # 工作 重要 待办
  // #标签: #物联网iot
  // : 标签: :工作:重要:待办
  const isTagLine = (line: string): boolean => {
    if (!line) return false;
    const trimmed = line.trim();
    // 以 # 或 : 开头
    return (
      trimmed.startsWith('#') ||   // "#标签" / "# 标签"
      /^\S+\s*#\s/.test(line) ||   // "标题 # 标签" (标题和#之间有空格)
      trimmed.startsWith(':') ||    // ":标签" 或 ":标签1:标签2"
      /^\S+\s*:\s*:/.test(line)    // "标题 :标签1:标签2"
    );
  };

  if (isTagLine(lastLine)) {
    tagLineIndex = lines.length - 1;
    const tagLine = lastLine;

    // 解析标签
    // 检查使用的是 # 还是 : 符号
    if (tagLine.includes('#')) {
      // 使用 # 符号
      // 格式可能是: "# 工作 重要 待办"、"#物联网iot" 或 "标题 # 工作 重要"
      const hashIndex = tagLine.indexOf('#');
      const tagPart = tagLine.slice(hashIndex + 1).trim();
      // 按空格分割，过滤空字符串
      tags = tagPart
        .split(/\s+/)
        .map((tag) => tag.replace(/^#+/, '').trim())
        .filter(Boolean)
        .join(' ');
    } else if (tagLine.includes(':')) {
      // 使用 : 符号
      // 格式可能是: ":工作:重要:待办" 或 "标题 :工作:重要"
      const colonIndex = tagLine.indexOf(':');
      const tagPart = tagLine.slice(colonIndex);
      // 提取 :xxx:xxx:xxx 格式的标签
      const tagMatches = tagPart.match(/:([^:\s]+)/g);
      if (tagMatches) {
        tags = tagMatches.map(t => t.slice(1)).join(' ');
      }
    }
  }

  // 清理后的内容（不包含标签行，但保留尾部格式）
  let cleanContent: string;
  if (tagLineIndex >= 0 && tagLineIndex === lines.length - 1) {
    // 最后一行是标签，去掉它
    cleanContent = lines.slice(0, -1).join('\n');
  } else {
    cleanContent = content;
  }

  return {
    title,
    content: cleanContent,
    tags,
    cleanContent,
  };
}

/**
 * 检查内容是否为空（只有空白字符）
 */
export function isEmptyContent(content: string): boolean {
  return !content || /^\s*$/.test(content);
}

/**
 * 从解析后的笔记构建要保存的内容
 * 标签会被附加到末尾
 */
export function buildNoteContent(parsed: ParsedNote): string {
  let result = parsed.cleanContent;

  // 如果有标签，附加到末尾
  if (parsed.tags) {
    const tagList = parsed.tags.split(/\s+/).filter(Boolean);
    if (tagList.length > 0) {
      // 检查原始内容使用的是哪种分隔符
      // 简单起见，默认使用 # 格式
      const tagLine = '\n# ' + tagList.join(' ');
      result = (result || '') + tagLine;
    }
  }

  return result;
}

/**
 * 格式化笔记内容用于显示
 * 将标签重新附加到内容末尾
 */
export function formatNoteForDisplay(note: { content: string; tags: string }): string {
  let content = note.content || '';

  if (note.tags) {
    const tagList = note.tags.split(/\s+/).filter(Boolean);
    if (tagList.length > 0) {
      // 检测使用哪种格式
      // 如果 tags 中包含 : 则使用 : 格式，否则使用 # 格式
      const useColon = tagList.some(t => t.includes(':'));
      if (useColon) {
        content += '\n:' + tagList.join(':');
      } else {
        content += '\n# ' + tagList.join(' ');
      }
    }
  }

  return content;
}
