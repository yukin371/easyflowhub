package relay

import (
	"fmt"
	"strings"
	"time"

	"scriptmgr/internal/extensions"
)

const (
	DefaultPort       = 8787
	DefaultTimeoutMs  = 60000
	DefaultStrategy   = "weighted_round_robin"
	DefaultRouteID    = "default"
	defaultFailThresh = 2
)

type Config struct {
	Version      int        `json:"version"`
	DefaultRoute string     `json:"default_route,omitempty"`
	Providers    []Provider `json:"providers,omitempty"`
	Routes       []Route    `json:"routes,omitempty"`
}

type Provider struct {
	ID            string            `json:"id"`
	Name          string            `json:"name"`
	BaseURL       string            `json:"base_url"`
	APIKey        string            `json:"api_key,omitempty"`
	APIKeyEnv     string            `json:"api_key_env,omitempty"`
	Source        string            `json:"source,omitempty"`
	Weight        int               `json:"weight,omitempty"`
	Enabled       bool              `json:"enabled"`
	ModelPatterns []string          `json:"model_patterns,omitempty"`
	Headers       map[string]string `json:"headers,omitempty"`
	TimeoutMs     int               `json:"timeout_ms,omitempty"`
}

type Route struct {
	ID            string   `json:"id"`
	Name          string   `json:"name,omitempty"`
	Source        string   `json:"source,omitempty"`
	PathPrefixes  []string `json:"path_prefixes,omitempty"`
	ModelPatterns []string `json:"model_patterns,omitempty"`
	ProviderIDs   []string `json:"provider_ids,omitempty"`
	Strategy      string   `json:"strategy,omitempty"`
}

type ProviderStatus struct {
	ProviderID          string     `json:"provider_id"`
	Healthy             bool       `json:"healthy"`
	ConsecutiveFailures int        `json:"consecutive_failures"`
	LastStatusCode      int        `json:"last_status_code,omitempty"`
	LastError           string     `json:"last_error,omitempty"`
	LastPickedAt        *time.Time `json:"last_picked_at,omitempty"`
	LastSuccessAt       *time.Time `json:"last_success_at,omitempty"`
	LastFailureAt       *time.Time `json:"last_failure_at,omitempty"`
}

type ProviderSnapshot struct {
	Provider Provider       `json:"provider"`
	Status   ProviderStatus `json:"status"`
}

type Snapshot struct {
	Config          Config                       `json:"config"`
	EffectiveConfig Config                       `json:"effective_config,omitempty"`
	Providers       []ProviderSnapshot           `json:"providers"`
	ExtensionRoots  []string                     `json:"extension_roots,omitempty"`
	Extensions      []extensions.ListedExtension `json:"extensions,omitempty"`
	ExtensionError  string                       `json:"extension_error,omitempty"`
}

func DefaultConfig() Config {
	return Config{
		Version:   1,
		Providers: []Provider{},
		Routes:    []Route{},
	}
}

func (cfg Config) Normalize() Config {
	if cfg.Version == 0 {
		cfg.Version = 1
	}
	for i := range cfg.Providers {
		if cfg.Providers[i].Weight <= 0 {
			cfg.Providers[i].Weight = 1
		}
		if cfg.Providers[i].TimeoutMs <= 0 {
			cfg.Providers[i].TimeoutMs = DefaultTimeoutMs
		}
		cfg.Providers[i].ID = strings.TrimSpace(cfg.Providers[i].ID)
		cfg.Providers[i].Name = strings.TrimSpace(cfg.Providers[i].Name)
		cfg.Providers[i].BaseURL = strings.TrimSpace(cfg.Providers[i].BaseURL)
		cfg.Providers[i].APIKey = strings.TrimSpace(cfg.Providers[i].APIKey)
		cfg.Providers[i].APIKeyEnv = strings.TrimSpace(cfg.Providers[i].APIKeyEnv)
	}
	for i := range cfg.Routes {
		if cfg.Routes[i].ID == "" {
			cfg.Routes[i].ID = fmt.Sprintf("route-%d", i+1)
		}
		if cfg.Routes[i].Strategy == "" {
			cfg.Routes[i].Strategy = DefaultStrategy
		}
	}
	return cfg
}

func (cfg Config) Validate() error {
	seenProviders := map[string]bool{}
	for _, provider := range cfg.Providers {
		if provider.ID == "" {
			return fmt.Errorf("provider missing id")
		}
		if provider.Name == "" {
			return fmt.Errorf("provider %q missing name", provider.ID)
		}
		if provider.BaseURL == "" {
			return fmt.Errorf("provider %q missing base_url", provider.ID)
		}
		if provider.APIKey != "" && provider.APIKeyEnv != "" {
			return fmt.Errorf("provider %q cannot set both api_key and api_key_env", provider.ID)
		}
		if seenProviders[provider.ID] {
			return fmt.Errorf("duplicate provider id %q", provider.ID)
		}
		seenProviders[provider.ID] = true
	}

	seenRoutes := map[string]bool{}
	for _, route := range cfg.Routes {
		if route.ID == "" {
			return fmt.Errorf("route missing id")
		}
		if seenRoutes[route.ID] {
			return fmt.Errorf("duplicate route id %q", route.ID)
		}
		seenRoutes[route.ID] = true
		for _, providerID := range route.ProviderIDs {
			if !seenProviders[providerID] {
				return fmt.Errorf("route %q references unknown provider %q", route.ID, providerID)
			}
		}
	}

	return nil
}
