package api

import (
	"os"
	"path/filepath"
	"testing"
)

func TestContainsFold(t *testing.T) {
	tests := []struct {
		name   string
		values []string
		target string
		want   bool
	}{
		{
			name:   "empty values",
			values: []string{},
			target: "test",
			want:   false,
		},
		{
			name:   "case insensitive match",
			values: []string{"Foo", "Bar", "Baz"},
			target: "foo",
			want:   true,
		},
		{
			name:   "case insensitive no match",
			values: []string{"Foo", "Bar", "Baz"},
			target: "qux",
			want:   false,
		},
		{
			name:   "exact match",
			values: []string{"test"},
			target: "test",
			want:   true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := containsFold(tt.values, tt.target)
			if got != tt.want {
				t.Errorf("containsFold() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestContainsPath(t *testing.T) {
	tests := []struct {
		name   string
		values []string
		target string
		want   bool
	}{
		{
			name:   "empty values",
			values: []string{},
			target: "/test",
			want:   false,
		},
		{
			name:   "case insensitive path match",
			values: []string{"C:\\Users\\Test", "/home/test"},
			target: "c:\\users\\test",
			want:   true,
		},
		{
			name:   "no match",
			values: []string{"C:\\Users\\Test", "/home/test"},
			target: "c:\\other",
			want:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := containsPath(tt.values, tt.target)
			if got != tt.want {
				t.Errorf("containsPath() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestNormalizeRootPath(t *testing.T) {
	// Create a temp directory for testing
	tmpDir := t.TempDir()

	// Mock osStat to use the real filesystem
	originalOsStat := osStat
	defer func() { osStat = originalOsStat }()

	tests := []struct {
		name    string
		path    string
		wantErr bool
	}{
		{
			name:    "valid directory",
			path:    tmpDir,
			wantErr: false,
		},
		{
			name:    "empty path",
			path:    "",
			wantErr: true,
		},
		{
			name:    "whitespace only",
			path:    "   ",
			wantErr: true,
		},
		{
			name:    "nonexistent path",
			path:    filepath.Join(tmpDir, "nonexistent"),
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := normalizeRootPath(tt.path)
			if (err != nil) != tt.wantErr {
				t.Errorf("normalizeRootPath() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestNormalizeRootPath_WithFile(t *testing.T) {
	tmpDir := t.TempDir()
	filePath := filepath.Join(tmpDir, "test.txt")

	// Create a file (not a directory)
	if err := os.WriteFile(filePath, []byte("test"), 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

	_, err := normalizeRootPath(filePath)
	if err == nil {
		t.Error("normalizeRootPath() expected error for file, got nil")
	}
}
