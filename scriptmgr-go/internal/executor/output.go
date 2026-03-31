package executor

import (
	"os"
	"strings"

	"scriptmgr/internal/model"
)

const (
	MaxOutputPreview = 1000
	MaxLogLineLength = 10000
)

func ProcessOutput(fullOutput, logPath string) *model.OutputResult {
	totalLen := len(fullOutput)
	lineCount := 0
	if fullOutput != "" {
		lineCount = strings.Count(fullOutput, "\n") + 1
	}

	preview := fullOutput
	truncated := false
	if totalLen > MaxOutputPreview {
		preview = fullOutput[:MaxOutputPreview]
		truncated = true
	}

	return &model.OutputResult{
		Truncated:   truncated,
		Preview:     preview,
		TotalLength: totalLen,
		LineCount:   lineCount,
		LogPath:     logPath,
	}
}

func ReadLog(logPath string, offset, limit int, tail bool) (string, error) {
	data, err := os.ReadFile(logPath)
	if err != nil {
		return "", err
	}

	lines := strings.Split(string(data), "\n")
	for i, line := range lines {
		if len(line) > MaxLogLineLength {
			lines[i] = line[:MaxLogLineLength] + "... (line truncated)"
		}
	}

	if limit <= 0 {
		limit = 100
	}

	if tail {
		if limit >= len(lines) {
			return strings.Join(lines, "\n"), nil
		}
		return strings.Join(lines[len(lines)-limit:], "\n"), nil
	}

	if offset < 0 {
		offset = 0
	}
	if offset >= len(lines) {
		return "", nil
	}
	end := offset + limit
	if end > len(lines) {
		end = len(lines)
	}
	return strings.Join(lines[offset:end], "\n"), nil
}
