package executor

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestProcessOutput(t *testing.T) {
	tests := []struct {
		name           string
		output         string
		logPath        string
		wantTruncated  bool
		wantPreviewLen int
		wantLineCount  int
		wantTotalLen   int
	}{
		{
			name:           "short output returns unchanged",
			output:         "Hello, World!",
			logPath:        "/path/to/log.txt",
			wantTruncated:  false,
			wantPreviewLen: 13,
			wantLineCount:  1,
			wantTotalLen:   13,
		},
		{
			name:           "empty string",
			output:         "",
			logPath:        "/path/to/log.txt",
			wantTruncated:  false,
			wantPreviewLen: 0,
			wantLineCount:  0,
			wantTotalLen:   0,
		},
		{
			name:           "single line no newline",
			output:         "single line",
			logPath:        "",
			wantTruncated:  false,
			wantPreviewLen: 11,
			wantLineCount:  1,
			wantTotalLen:   11,
		},
		{
			name:           "multiple lines",
			output:         "line1\nline2\nline3",
			logPath:        "",
			wantTruncated:  false,
			wantPreviewLen: 17,
			wantLineCount:  3,
			wantTotalLen:   17,
		},
		{
			name:           "output with trailing newline",
			output:         "line1\nline2\n",
			logPath:        "",
			wantTruncated:  false,
			wantPreviewLen: 12,
			wantLineCount:  3, // strings.Count("\n") + 1 = 2 + 1 = 3
			wantTotalLen:   12,
		},
		{
			name:           "output exactly at MaxOutputPreview",
			output:         strings.Repeat("x", MaxOutputPreview),
			logPath:        "",
			wantTruncated:  false,
			wantPreviewLen: MaxOutputPreview,
			wantLineCount:  1,
			wantTotalLen:   MaxOutputPreview,
		},
		{
			name:           "output exceeds MaxOutputPreview",
			output:         strings.Repeat("x", MaxOutputPreview+100),
			logPath:        "/log",
			wantTruncated:  true,
			wantPreviewLen: MaxOutputPreview,
			wantLineCount:  1,
			wantTotalLen:   MaxOutputPreview + 100,
		},
		{
			name:           "large multiline output gets truncated",
			output:         strings.Repeat("line\n", 300), // 5 * 300 = 1500 chars
			logPath:        "",
			wantTruncated:  true,
			wantPreviewLen: MaxOutputPreview,
			wantLineCount:  301, // 300 "line\n" creates 301 lines (including empty after final \n)
			wantTotalLen:   1500,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ProcessOutput(tt.output, tt.logPath)

			if result.Truncated != tt.wantTruncated {
				t.Errorf("Truncated = %v, want %v", result.Truncated, tt.wantTruncated)
			}
			if len(result.Preview) != tt.wantPreviewLen {
				t.Errorf("Preview length = %d, want %d", len(result.Preview), tt.wantPreviewLen)
			}
			if result.LineCount != tt.wantLineCount {
				t.Errorf("LineCount = %d, want %d", result.LineCount, tt.wantLineCount)
			}
			if result.TotalLength != tt.wantTotalLen {
				t.Errorf("TotalLength = %d, want %d", result.TotalLength, tt.wantTotalLen)
			}
			if result.LogPath != tt.logPath {
				t.Errorf("LogPath = %q, want %q", result.LogPath, tt.logPath)
			}

			// Verify preview content matches start of output for non-truncated
			if !tt.wantTruncated && result.Preview != tt.output {
				t.Errorf("Preview = %q, want %q", result.Preview, tt.output)
			}

			// Verify preview is prefix of output for truncated
			if tt.wantTruncated && !strings.HasPrefix(tt.output, result.Preview) {
				t.Errorf("Preview should be prefix of output")
			}
		})
	}
}

func TestReadLog(t *testing.T) {
	t.Run("reads entire file", func(t *testing.T) {
		content := "line1\nline2\nline3\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 0, 100, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// File has trailing newline, Split creates empty element at end
		expected := "line1\nline2\nline3\n"
		if result != expected {
			t.Errorf("result = %q, want %q", result, expected)
		}
	})

	t.Run("offset skips first N lines", func(t *testing.T) {
		content := "line1\nline2\nline3\nline4\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 2, 100, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Skip 2 lines (line1, line2), get rest
		if !strings.Contains(result, "line3") {
			t.Errorf("expected 'line3' in result, got: %q", result)
		}
		if strings.Contains(result, "line1") || strings.Contains(result, "line2") {
			t.Errorf("should not contain line1 or line2, got: %q", result)
		}
	})

	t.Run("limit returns at most N lines", func(t *testing.T) {
		content := "line1\nline2\nline3\nline4\nline5\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 0, 2, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		lines := strings.Split(strings.TrimSuffix(result, "\n"), "\n")
		if len(lines) > 2 {
			t.Errorf("expected at most 2 lines, got %d: %q", len(lines), result)
		}
	})

	t.Run("offset and limit together", func(t *testing.T) {
		content := "line1\nline2\nline3\nline4\nline5\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 1, 2, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Should get line2 and line3 (offset 1, limit 2)
		if !strings.Contains(result, "line2") || !strings.Contains(result, "line3") {
			t.Errorf("expected line2 and line3, got: %q", result)
		}
		if strings.Contains(result, "line1") || strings.Contains(result, "line4") {
			t.Errorf("should not contain line1 or line4, got: %q", result)
		}
	})

	t.Run("offset beyond file length returns empty", func(t *testing.T) {
		content := "line1\nline2\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 100, 10, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if result != "" {
			t.Errorf("expected empty result, got: %q", result)
		}
	})

	t.Run("tail returns last N lines", func(t *testing.T) {
		content := "line1\nline2\nline3\nline4\nline5\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 0, 3, true)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Should get last 3 lines (line4, line5, and empty trailing from final newline)
		// When split by \n: [line1, line2, line3, line4, line5, ""]
		// Last 3: [line4, line5, ""]
		if !strings.Contains(result, "line4") || !strings.Contains(result, "line5") {
			t.Errorf("expected line4 and line5, got: %q", result)
		}
		if strings.Contains(result, "line1") || strings.Contains(result, "line2") || strings.Contains(result, "line3") {
			t.Errorf("should not contain first 3 lines, got: %q", result)
		}
	})

	t.Run("tail with limit >= total lines returns all", func(t *testing.T) {
		content := "line1\nline2\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 0, 100, true)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if !strings.Contains(result, "line1") || !strings.Contains(result, "line2") {
			t.Errorf("expected all lines, got: %q", result)
		}
	})

	t.Run("negative offset treated as 0", func(t *testing.T) {
		content := "line1\nline2\nline3\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, -5, 10, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if !strings.Contains(result, "line1") {
			t.Errorf("expected line1 with negative offset, got: %q", result)
		}
	})

	t.Run("limit <= 0 defaults to 100", func(t *testing.T) {
		content := strings.Repeat("line\n", 150) // 150 lines
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		// With limit=0, should default to 100
		result, err := ReadLog(logPath, 0, 0, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Count lines in result
		lines := strings.Count(result, "\n")
		if lines > 100 {
			t.Errorf("expected at most 100 lines with limit=0, got %d", lines)
		}

		// With negative limit, should also default to 100
		result2, err := ReadLog(logPath, 0, -1, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		lines2 := strings.Count(result2, "\n")
		if lines2 > 100 {
			t.Errorf("expected at most 100 lines with limit=-1, got %d", lines2)
		}
	})

	t.Run("non-existent file returns error", func(t *testing.T) {
		_, err := ReadLog("/nonexistent/path/to/file.log", 0, 10, false)
		if err == nil {
			t.Error("expected error for non-existent file, got nil")
		}
	})

	t.Run("empty file returns empty", func(t *testing.T) {
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "empty.log")
		if err := os.WriteFile(logPath, []byte(""), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 0, 10, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if result != "" {
			t.Errorf("expected empty result for empty file, got: %q", result)
		}
	})

	t.Run("file with no trailing newline", func(t *testing.T) {
		content := "line1\nline2\nline3" // No trailing newline
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 0, 10, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if !strings.Contains(result, "line3") {
			t.Errorf("expected line3 without trailing newline, got: %q", result)
		}
	})

	t.Run("file with only newlines", func(t *testing.T) {
		content := "\n\n\n"
		tmpDir := t.TempDir()
		logPath := filepath.Join(tmpDir, "test.log")
		if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write temp file: %v", err)
		}

		result, err := ReadLog(logPath, 0, 10, false)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Should have 3 newlines (4 empty strings after split)
		newlineCount := strings.Count(result, "\n")
		if newlineCount != 3 {
			t.Errorf("expected 3 newlines, got %d", newlineCount)
		}
	})
}

func TestReadLog_TruncatesLongLines(t *testing.T) {
	// Create a line that exceeds MaxLogLineLength
	longLine := strings.Repeat("x", MaxLogLineLength+500)
	content := longLine + "\nshort line\n"

	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "longline.log")
	if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write temp file: %v", err)
	}

	result, err := ReadLog(logPath, 0, 10, false)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Verify the long line was truncated
	if !strings.Contains(result, "... (line truncated)") {
		t.Errorf("expected line truncation marker, got: %q", result)
	}

	// Verify the truncated line length
	truncatedLine := strings.Split(result, "\n")[0]
	if len(truncatedLine) > MaxLogLineLength+50 { // +50 for truncation suffix
		t.Errorf("truncated line still too long: %d", len(truncatedLine))
	}

	// Verify other lines are intact
	if !strings.Contains(result, "short line") {
		t.Errorf("expected 'short line' to be intact, got: %q", result)
	}
}

func TestReadLog_MultipleLongLines(t *testing.T) {
	// Multiple long lines
	line1 := strings.Repeat("a", MaxLogLineLength+100)
	line2 := strings.Repeat("b", MaxLogLineLength+200)
	line3 := "short"
	content := line1 + "\n" + line2 + "\n" + line3 + "\n"

	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "multilong.log")
	if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write temp file: %v", err)
	}

	result, err := ReadLog(logPath, 0, 10, false)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Count truncation markers
	truncCount := strings.Count(result, "... (line truncated)")
	if truncCount != 2 {
		t.Errorf("expected 2 truncation markers, got %d", truncCount)
	}

	// Verify short line is intact
	if !strings.Contains(result, "short") {
		t.Errorf("expected 'short' to be intact, got: %q", result)
	}
}

func TestConstants(t *testing.T) {
	// Verify constants have expected values
	if MaxOutputPreview != 1000 {
		t.Errorf("MaxOutputPreview = %d, want 1000", MaxOutputPreview)
	}
	if MaxLogLineLength != 10000 {
		t.Errorf("MaxLogLineLength = %d, want 10000", MaxLogLineLength)
	}
}

func TestProcessOutput_PreviewExactly1000(t *testing.T) {
	// Verify that output of exactly MaxOutputPreview is not truncated
	output := strings.Repeat("a", 1000)
	result := ProcessOutput(output, "")

	if result.Truncated {
		t.Error("output of exactly MaxOutputPreview should not be truncated")
	}
	if result.Preview != output {
		t.Error("preview should equal input when not truncated")
	}
}

func TestProcessOutput_Preview1001(t *testing.T) {
	// Verify that output of MaxOutputPreview + 1 is truncated
	output := strings.Repeat("a", 1001)
	result := ProcessOutput(output, "")

	if !result.Truncated {
		t.Error("output of MaxOutputPreview + 1 should be truncated")
	}
	if len(result.Preview) != MaxOutputPreview {
		t.Errorf("preview length = %d, want %d", len(result.Preview), MaxOutputPreview)
	}
	if result.TotalLength != 1001 {
		t.Errorf("TotalLength = %d, want 1001", result.TotalLength)
	}
}

func TestReadLog_TailWithFewerLinesThanLimit(t *testing.T) {
	content := "line1\nline2\n"
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "test.log")
	if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write temp file: %v", err)
	}

	// Request 10 lines in tail mode, but file only has 2
	result, err := ReadLog(logPath, 0, 10, true)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Should return all available lines
	if !strings.Contains(result, "line1") || !strings.Contains(result, "line2") {
		t.Errorf("expected all lines, got: %q", result)
	}
}

func TestReadLog_TailWithOffsetIgnored(t *testing.T) {
	// In tail mode, offset should be ignored
	content := "line1\nline2\nline3\nline4\nline5\n"
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "test.log")
	if err := os.WriteFile(logPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write temp file: %v", err)
	}

	// Tail mode should return last 3 lines regardless of offset
	// When split by \n: [line1, line2, line3, line4, line5, ""]
	// Last 3: [line4, line5, ""]
	result, err := ReadLog(logPath, 10, 3, true)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Should get last 2 non-empty lines (line4, line5)
	if !strings.Contains(result, "line4") || !strings.Contains(result, "line5") {
		t.Errorf("expected last 2 lines in tail mode, got: %q", result)
	}
}
