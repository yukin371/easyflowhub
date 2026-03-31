package discovery

import (
	"testing"

	"scriptmgr/internal/model"
)

func TestMatchesSearch(t *testing.T) {
	tests := []struct {
		name   string
		script model.ScriptRecord
		search string
		want   bool
	}{
		{
			name: "matches id",
			script: model.ScriptRecord{
				ID:   "test-script",
				Name: "Test Script",
			},
			search: "test",
			want:   true,
		},
		{
			name: "matches name",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Backup Script",
			},
			search: "backup",
			want:   true,
		},
		{
			name: "matches description",
			script: model.ScriptRecord{
				ID:          "script1",
				Name:        "Script",
				Description: "This script backs up files",
			},
			search: "backs",
			want:   true,
		},
		{
			name: "matches category",
			script: model.ScriptRecord{
				ID:       "script1",
				Name:     "Script",
				Category: "utilities",
			},
			search: "util",
			want:   true,
		},
		{
			name: "matches tags",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Script",
				Tags: []string{"backup", "automation"},
			},
			search: "backup",
			want:   true,
		},
		{
			name: "no match",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Script",
			},
			search: "xyz123",
			want:   false,
		},
		{
			name: "case insensitive",
			script: model.ScriptRecord{
				ID:   "test-script",
				Name: "Test Script",
			},
			search: "TEST",
			want:   true,
		},
		{
			name: "empty search matches all",
			script: model.ScriptRecord{
				ID:   "script1",
				Name: "Script",
			},
			search: "",
			want:   true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := matchesSearch(tt.script, tt.search)
			if got != tt.want {
				t.Errorf("matchesSearch() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestIsSameOrNestedRoot(t *testing.T) {
	tests := []struct {
		name      string
		root      string
		candidate string
		want      bool
	}{
		{
			name:      "same path",
			root:      "C:\\Users\\Test",
			candidate: "C:\\Users\\Test",
			want:      true,
		},
		{
			name:      "nested path",
			root:      "C:\\Users\\Test\\Scripts",
			candidate: "C:\\Users\\Test",
			want:      true,
		},
		{
			name:      "not nested",
			root:      "C:\\Users\\Other",
			candidate: "C:\\Users\\Test",
			want:      false,
		},
		{
			name:      "case insensitive windows",
			root:      "C:\\Users\\Test",
			candidate: "c:\\users\\test",
			want:      true,
		},
		{
			name:      "different drive",
			root:      "D:\\Users\\Test",
			candidate: "C:\\Users\\Test",
			want:      false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := isSameOrNestedRoot(tt.root, tt.candidate)
			if got != tt.want {
				t.Errorf("isSameOrNestedRoot() = %v, want %v", got, tt.want)
			}
		})
	}
}
