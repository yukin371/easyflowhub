package mcpcli

import (
	"testing"

	"scriptmgr/internal/extensions"
)

func TestBuildEffectiveCatalogMarksConflictsAndExtensionSources(t *testing.T) {
	config := &Config{
		Version: "1.0",
		Servers: map[string]ServerConfig{
			"persisted-server": {
				Command: "node",
				Args:    []string{"persisted.js"},
			},
			"conflict-server": {
				Command: "python",
				Args:    []string{"persisted.py"},
			},
		},
	}

	catalog := BuildEffectiveCatalog(config, extensions.EffectiveContributions{
		MCPServers: []extensions.EffectiveMCPServerContribution{
			{
				Source: extensions.ContributionSource{ExtensionID: "sample-pack"},
				MCPServerContribution: extensions.MCPServerContribution{
					ID:          "extension-server",
					Name:        "Extension Server",
					Command:     "deno",
					Args:        []string{"run", "server.ts"},
					Description: "from extension",
				},
			},
			{
				Source: extensions.ContributionSource{ExtensionID: "sample-pack"},
				MCPServerContribution: extensions.MCPServerContribution{
					ID:      "conflict-server",
					Name:    "Conflict Server",
					Command: "node",
				},
			},
		},
	})

	if len(catalog.Servers) != 4 {
		t.Fatalf("expected 4 effective entries, got %d", len(catalog.Servers))
	}

	var extensionEntry *EffectiveServerEntry
	var conflictEntry *EffectiveServerEntry
	for i := range catalog.Servers {
		item := catalog.Servers[i]
		if item.Key == "extension:sample-pack:extension-server" {
			extensionEntry = &item
		}
		if item.Key == "extension:sample-pack:conflict-server" {
			conflictEntry = &item
		}
	}

	if extensionEntry == nil || extensionEntry.Status != EffectiveServerStatusExtension {
		t.Fatalf("unexpected extension entry %+v", extensionEntry)
	}
	if extensionEntry.Source != "extension:sample-pack" {
		t.Fatalf("unexpected extension source %q", extensionEntry.Source)
	}
	if conflictEntry == nil || conflictEntry.Status != EffectiveServerStatusConflicted {
		t.Fatalf("unexpected conflict entry %+v", conflictEntry)
	}
	if conflictEntry.ConflictWith != "persisted:conflict-server" {
		t.Fatalf("unexpected conflict target %q", conflictEntry.ConflictWith)
	}
}
