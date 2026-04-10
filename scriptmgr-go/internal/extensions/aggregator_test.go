package extensions

import "testing"

func TestContributionAggregatorMergeIncludesOnlyLoadedValidContributions(t *testing.T) {
	aggregator := NewContributionAggregator()

	items := []ListedExtension{
		{
			Status:       "loaded",
			ManifestPath: "/exts/alpha/plugin.json",
			Root:         "/exts",
			Manifest: &Manifest{
				ID:      "alpha",
				Name:    "Alpha",
				Version: "1.0.0",
				Contributions: Contributions{
					RelayProviders: []RelayProviderContribution{
						{ID: "provider-alpha", Name: "Provider Alpha", BaseURL: "https://alpha.example.com"},
						{ID: " ", Name: "Invalid", BaseURL: "https://invalid.example.com"},
					},
					RelayRoutes: []RelayRouteContribution{
						{ID: "route-alpha", ProviderIDs: []string{"provider-alpha"}, PathPrefixes: []string{"/v1/"}},
						{ID: "route-invalid", ProviderIDs: []string{" "}},
					},
					ScriptRoots: []ScriptRootContribution{
						{Path: " C:/Scripts "},
						{Path: " "},
					},
					MCPServers: []MCPServerContribution{
						{ID: "mcp-alpha", Name: "Alpha MCP", Command: "node", Args: []string{"server.js"}},
						{ID: "mcp-invalid", Name: "", Command: "node"},
					},
					ManagerModules: []ManagerModuleContribution{
						{ID: "mod-alpha", Name: "Alpha Module"},
						{ID: "", Name: "Missing ID"},
					},
				},
			},
		},
		{
			Status: "invalid",
			Manifest: &Manifest{
				ID:      "broken",
				Name:    "Broken",
				Version: "1.0.0",
				Contributions: Contributions{
					RelayProviders: []RelayProviderContribution{
						{ID: "provider-broken", Name: "Broken", BaseURL: "https://broken.example.com"},
					},
				},
			},
		},
	}

	result := aggregator.Merge(items)

	if len(result.RelayProviders) != 1 {
		t.Fatalf("expected 1 relay provider, got %d", len(result.RelayProviders))
	}
	if result.RelayProviders[0].ID != "provider-alpha" {
		t.Fatalf("unexpected relay provider id %q", result.RelayProviders[0].ID)
	}
	if result.RelayProviders[0].Source.ExtensionID != "alpha" {
		t.Fatalf("unexpected relay provider source %q", result.RelayProviders[0].Source.ExtensionID)
	}

	if len(result.RelayRoutes) != 1 || result.RelayRoutes[0].ID != "route-alpha" {
		t.Fatalf("unexpected relay routes %+v", result.RelayRoutes)
	}
	if len(result.ScriptRoots) != 1 || result.ScriptRoots[0].Path != "C:/Scripts" {
		t.Fatalf("unexpected script roots %+v", result.ScriptRoots)
	}
	if len(result.MCPServers) != 1 || result.MCPServers[0].ID != "mcp-alpha" {
		t.Fatalf("unexpected mcp servers %+v", result.MCPServers)
	}
	if len(result.ManagerModules) != 1 || result.ManagerModules[0].ID != "mod-alpha" {
		t.Fatalf("unexpected manager modules %+v", result.ManagerModules)
	}
}
