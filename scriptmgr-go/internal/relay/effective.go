package relay

import (
	"strings"

	"scriptmgr/internal/extensions"
)

func (s *Service) ExtensionRoots() []string {
	if s.extensions == nil {
		return nil
	}
	return s.extensions.Roots()
}

func (s *Service) EffectiveContributions() (extensions.EffectiveContributions, error) {
	if s.extensions == nil {
		return extensions.EffectiveContributions{}, nil
	}
	return s.extensions.EffectiveContributions()
}

func (s *Service) EffectiveConfig() (Config, error) {
	cfg, err := s.LoadConfig()
	if err != nil {
		return Config{}, err
	}

	return s.mergeEffectiveConfig(cfg)
}

func (s *Service) mergeEffectiveConfig(cfg Config) (Config, error) {
	merged := cfg.Normalize()

	contributions, err := s.EffectiveContributions()
	if err != nil {
		return Config{}, err
	}

	seenProviders := make(map[string]bool, len(merged.Providers))
	for _, provider := range merged.Providers {
		seenProviders[strings.TrimSpace(provider.ID)] = true
	}

	for _, contribution := range contributions.RelayProviders {
		if seenProviders[contribution.ID] {
			continue
		}
		merged.Providers = append(merged.Providers, Provider{
			ID:            contribution.ID,
			Name:          contribution.Name,
			BaseURL:       contribution.BaseURL,
			Source:        "extension:" + contribution.Source.ExtensionID,
			Weight:        contribution.Weight,
			Enabled:       true,
			ModelPatterns: append([]string(nil), contribution.ModelPatterns...),
			Headers:       cloneHeaders(contribution.Headers),
			TimeoutMs:     DefaultTimeoutMs,
		})
		seenProviders[contribution.ID] = true
	}

	seenRoutes := make(map[string]bool, len(merged.Routes))
	for _, route := range merged.Routes {
		seenRoutes[strings.TrimSpace(route.ID)] = true
	}

	for _, contribution := range contributions.RelayRoutes {
		if seenRoutes[contribution.ID] {
			continue
		}

		providerIDs := make([]string, 0, len(contribution.ProviderIDs))
		for _, providerID := range contribution.ProviderIDs {
			if seenProviders[providerID] {
				providerIDs = append(providerIDs, providerID)
			}
		}
		if len(providerIDs) == 0 {
			continue
		}

		merged.Routes = append(merged.Routes, Route{
			ID:            contribution.ID,
			Name:          contribution.Name,
			Source:        "extension:" + contribution.Source.ExtensionID,
			PathPrefixes:  append([]string(nil), contribution.PathPrefixes...),
			ModelPatterns: append([]string(nil), contribution.ModelPatterns...),
			ProviderIDs:   providerIDs,
			Strategy:      contribution.Strategy,
		})
		seenRoutes[contribution.ID] = true
	}

	return merged.Normalize(), nil
}

func cloneHeaders(headers map[string]string) map[string]string {
	if len(headers) == 0 {
		return nil
	}

	result := make(map[string]string, len(headers))
	for key, value := range headers {
		result[key] = value
	}
	return result
}
