package extensions

type Manifest struct {
	ID            string        `json:"id"`
	Name          string        `json:"name"`
	Version       string        `json:"version"`
	Description   string        `json:"description,omitempty"`
	Author        string        `json:"author,omitempty"`
	Homepage      string        `json:"homepage,omitempty"`
	Contributions Contributions `json:"contributions,omitempty"`
}

type Contributions struct {
	RelayProviders []RelayProviderContribution `json:"relay_providers,omitempty"`
	RelayRoutes    []RelayRouteContribution    `json:"relay_routes,omitempty"`
	ScriptRoots    []ScriptRootContribution    `json:"script_roots,omitempty"`
	MCPServers     []MCPServerContribution     `json:"mcp_servers,omitempty"`
	ManagerModules []ManagerModuleContribution `json:"manager_modules,omitempty"`
}

type RelayProviderContribution struct {
	ID            string            `json:"id"`
	Name          string            `json:"name"`
	BaseURL       string            `json:"base_url"`
	Weight        int               `json:"weight,omitempty"`
	ModelPatterns []string          `json:"model_patterns,omitempty"`
	Headers       map[string]string `json:"headers,omitempty"`
}

type RelayRouteContribution struct {
	ID            string   `json:"id"`
	Name          string   `json:"name,omitempty"`
	ModelPatterns []string `json:"model_patterns,omitempty"`
	PathPrefixes  []string `json:"path_prefixes,omitempty"`
	ProviderIDs   []string `json:"provider_ids,omitempty"`
	Strategy      string   `json:"strategy,omitempty"`
}

type ScriptRootContribution struct {
	Path        string `json:"path"`
	Description string `json:"description,omitempty"`
}

type MCPServerContribution struct {
	ID          string   `json:"id"`
	Name        string   `json:"name"`
	Command     string   `json:"command"`
	Args        []string `json:"args,omitempty"`
	Description string   `json:"description,omitempty"`
}

type ManagerModuleContribution struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Caption     string `json:"caption,omitempty"`
	Icon        string `json:"icon,omitempty"`
	Description string `json:"description,omitempty"`
}

type ListedExtension struct {
	ManifestPath string    `json:"manifest_path"`
	Root         string    `json:"root"`
	Status       string    `json:"status"`
	Error        string    `json:"error,omitempty"`
	Manifest     *Manifest `json:"manifest,omitempty"`
}
