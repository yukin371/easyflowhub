export type DiffChangeType = 'add' | 'delete' | 'modify' | 'same';

export interface DiffLine {
  type: DiffChangeType;
  currentLine?: string;
  backupLine?: string;
  lineNumber: number;
}

export interface DiffResult {
  hasChanges: boolean;
  changes: Array<{
    type: 'add' | 'delete' | 'modify';
    start: number;
    end: number;
  }>;
  lines: DiffLine[];
}

export function diffText(currentText: string, backupText: string): DiffResult {
  const currentLines = currentText.split('\n');
  const backupLines = backupText.split('\n');
  const maxLines = Math.max(currentLines.length, backupLines.length);
  const lines: DiffLine[] = [];
  const changes: DiffResult['changes'] = [];

  for (let index = 0; index < maxLines; index += 1) {
    const currentLine = currentLines[index];
    const backupLine = backupLines[index];

    if (currentLine === backupLine) {
      lines.push({
        type: 'same',
        currentLine,
        backupLine,
        lineNumber: index + 1,
      });
      continue;
    }

    let type: DiffChangeType = 'modify';
    if (currentLine === undefined) {
      type = 'delete';
    } else if (backupLine === undefined) {
      type = 'add';
    }

    lines.push({
      type,
      currentLine,
      backupLine,
      lineNumber: index + 1,
    });

    changes.push({
      type,
      start: index + 1,
      end: index + 1,
    });
  }

  return {
    hasChanges: changes.length > 0,
    changes,
    lines,
  };
}
