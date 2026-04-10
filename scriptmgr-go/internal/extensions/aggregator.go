package extensions

import "strings"

type ContributionSource struct {
	ExtensionID      string `json:"extension_id"`
	ExtensionName    string `json:"extension_name"`
	ExtensionVersion string `json:"extension_version"`
	ManifestPath     string `json:"manifest_path"`
	Root             string `json:"root"`
}

type EffectiveContributions struct {
	RelayProviders []EffectiveRelayProviderContribution `json:"relay_providers,omitempty"`
	RelayRoutes    []EffectiveRelayRouteContribution    `json:"relay_routes,omitempty"`
	ScriptRoots    []EffectiveScriptRootContribution    `json:"script_roots,omitempty"`
	MCPServers     []EffectiveMCPServerContribution     `json:"mcp_servers,omitempty"`
	ManagerModules []EffectiveManagerModuleContribution `json:"manager_modules,omitempty"`
}

type EffectiveRelayProviderContribution struct {
	Source ContributionSource `json:"source"`
	RelayProviderContribution
}

type EffectiveRelayRouteContribution struct {
	Source ContributionSource `json:"source"`
	RelayRouteContribution
}

type EffectiveScriptRootContribution struct {
	Source ContributionSource `json:"source"`
	ScriptRootContribution
}

type EffectiveMCPServerContribution struct {
	Source ContributionSource `json:"source"`
	MCPServerContribution
}

type EffectiveManagerModuleContribution struct {
	Source ContributionSource `json:"source"`
	ManagerModuleContribution
}

type ContributionAggregator struct{}

func NewContributionAggregator() *ContributionAggregator {
	return &ContributionAggregator{}
}

func (a *ContributionAggregator) Merge(items []ListedExtension) EffectiveContributions {
	var result EffectiveContributions
	for _, item := range items {
		manifest, source, ok := activeManifest(item)
		if !ok {
			continue
		}

		for _, provider := range manifest.Contributions.RelayProviders {
			provider = normalizeRelayProviderContribution(provider)
			if !validRelayProviderContribution(provider) {
				continue
			}
			result.RelayProviders = append(result.RelayProviders, EffectiveRelayProviderContribution{
				Source:                    source,
				RelayProviderContribution: provider,
			})
		}

		for _, route := range manifest.Contributions.RelayRoutes {
			route = normalizeRelayRouteContribution(route)
			if !validRelayRouteContribution(route) {
				continue
			}
			result.RelayRoutes = append(result.RelayRoutes, EffectiveRelayRouteContribution{
				Source:                 source,
				RelayRouteContribution: route,
			})
		}

		for _, scriptRoot := range manifest.Contributions.ScriptRoots {
			scriptRoot = normalizeScriptRootContribution(scriptRoot)
			if !validScriptRootContribution(scriptRoot) {
				continue
			}
			result.ScriptRoots = append(result.ScriptRoots, EffectiveScriptRootContribution{
				Source:                 source,
				ScriptRootContribution: scriptRoot,
			})
		}

		for _, server := range manifest.Contributions.MCPServers {
			server = normalizeMCPServerContribution(server)
			if !validMCPServerContribution(server) {
				continue
			}
			result.MCPServers = append(result.MCPServers, EffectiveMCPServerContribution{
				Source:                source,
				MCPServerContribution: server,
			})
		}

		for _, module := range manifest.Contributions.ManagerModules {
			module = normalizeManagerModuleContribution(module)
			if !validManagerModuleContribution(module) {
				continue
			}
			result.ManagerModules = append(result.ManagerModules, EffectiveManagerModuleContribution{
				Source:                    source,
				ManagerModuleContribution: module,
			})
		}
	}

	return result
}

func activeManifest(item ListedExtension) (*Manifest, ContributionSource, bool) {
	if item.Status != "loaded" || item.Manifest == nil {
		return nil, ContributionSource{}, false
	}

	return item.Manifest, ContributionSource{
		ExtensionID:      strings.TrimSpace(item.Manifest.ID),
		ExtensionName:    strings.TrimSpace(item.Manifest.Name),
		ExtensionVersion: strings.TrimSpace(item.Manifest.Version),
		ManifestPath:     item.ManifestPath,
		Root:             item.Root,
	}, true
}

func normalizeRelayProviderContribution(item RelayProviderContribution) RelayProviderContribution {
	item.ID = strings.TrimSpace(item.ID)
	item.Name = strings.TrimSpace(item.Name)
	item.BaseURL = strings.TrimSpace(item.BaseURL)
	item.ModelPatterns = trimNonEmpty(item.ModelPatterns)
	if item.Weight <= 0 {
		item.Weight = 1
	}
	return item
}

func validRelayProviderContribution(item RelayProviderContribution) bool {
	return item.ID != "" && item.Name != "" && item.BaseURL != ""
}

func normalizeRelayRouteContribution(item RelayRouteContribution) RelayRouteContribution {
	item.ID = strings.TrimSpace(item.ID)
	item.Name = strings.TrimSpace(item.Name)
	item.PathPrefixes = trimNonEmpty(item.PathPrefixes)
	item.ModelPatterns = trimNonEmpty(item.ModelPatterns)
	item.ProviderIDs = trimNonEmpty(item.ProviderIDs)
	item.Strategy = strings.TrimSpace(item.Strategy)
	return item
}

func validRelayRouteContribution(item RelayRouteContribution) bool {
	return item.ID != "" && len(item.ProviderIDs) > 0
}

func normalizeScriptRootContribution(item ScriptRootContribution) ScriptRootContribution {
	item.Path = strings.TrimSpace(item.Path)
	item.Description = strings.TrimSpace(item.Description)
	return item
}

func validScriptRootContribution(item ScriptRootContribution) bool {
	return item.Path != ""
}

func normalizeMCPServerContribution(item MCPServerContribution) MCPServerContribution {
	item.ID = strings.TrimSpace(item.ID)
	item.Name = strings.TrimSpace(item.Name)
	item.Command = strings.TrimSpace(item.Command)
	item.Args = trimNonEmpty(item.Args)
	item.Description = strings.TrimSpace(item.Description)
	return item
}

func validMCPServerContribution(item MCPServerContribution) bool {
	return item.ID != "" && item.Name != "" && item.Command != ""
}

func normalizeManagerModuleContribution(item ManagerModuleContribution) ManagerModuleContribution {
	item.ID = strings.TrimSpace(item.ID)
	item.Name = strings.TrimSpace(item.Name)
	item.Caption = strings.TrimSpace(item.Caption)
	item.Icon = strings.TrimSpace(item.Icon)
	item.Description = strings.TrimSpace(item.Description)
	return item
}

func validManagerModuleContribution(item ManagerModuleContribution) bool {
	return item.ID != "" && item.Name != ""
}

func trimNonEmpty(items []string) []string {
	if len(items) == 0 {
		return nil
	}

	result := make([]string, 0, len(items))
	for _, item := range items {
		item = strings.TrimSpace(item)
		if item != "" {
			result = append(result, item)
		}
	}
	if len(result) == 0 {
		return nil
	}
	return result
}
