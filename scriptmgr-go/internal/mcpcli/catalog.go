package mcpcli

import (
	"sort"

	"scriptmgr/internal/extensions"
)

type EffectiveServerStatus string

const (
	EffectiveServerStatusPersisted  EffectiveServerStatus = "persisted"
	EffectiveServerStatusExtension  EffectiveServerStatus = "extension"
	EffectiveServerStatusConflicted EffectiveServerStatus = "conflicted"
)

type EffectiveServerEntry struct {
	Key          string                `json:"key"`
	Name         string                `json:"name"`
	Command      string                `json:"command"`
	Args         []string              `json:"args,omitempty"`
	Env          map[string]string     `json:"env,omitempty"`
	Description  string                `json:"description,omitempty"`
	Status       EffectiveServerStatus `json:"status"`
	Source       string                `json:"source"`
	ConflictWith string                `json:"conflict_with,omitempty"`
}

type EffectiveCatalog struct {
	ConfigPath string                 `json:"config_path,omitempty"`
	Servers    []EffectiveServerEntry `json:"servers"`
}

func LoadEffectiveCatalog(contributions extensions.EffectiveContributions) (EffectiveCatalog, error) {
	config, err := LoadConfig()
	if err != nil {
		return EffectiveCatalog{}, err
	}

	return BuildEffectiveCatalog(config, contributions), nil
}

func BuildEffectiveCatalog(config *Config, contributions extensions.EffectiveContributions) EffectiveCatalog {
	if config == nil {
		config = &Config{Version: "1.0", Servers: map[string]ServerConfig{}}
	}

	names := make([]string, 0, len(config.Servers))
	for name := range config.Servers {
		names = append(names, name)
	}
	sort.Strings(names)

	result := EffectiveCatalog{
		ConfigPath: GetConfigPath(),
		Servers:    make([]EffectiveServerEntry, 0, len(config.Servers)+len(contributions.MCPServers)),
	}

	persisted := make(map[string]ServerConfig, len(config.Servers))
	for _, name := range names {
		server := config.Servers[name]
		persisted[name] = server
		result.Servers = append(result.Servers, EffectiveServerEntry{
			Key:     "persisted:" + name,
			Name:    name,
			Command: server.Command,
			Args:    append([]string(nil), server.Args...),
			Env:     cloneEnv(server.Env),
			Status:  EffectiveServerStatusPersisted,
			Source:  "persisted",
		})
	}

	for _, item := range contributions.MCPServers {
		entry := EffectiveServerEntry{
			Key:         "extension:" + item.Source.ExtensionID + ":" + item.ID,
			Name:        item.ID,
			Command:     item.Command,
			Args:        append([]string(nil), item.Args...),
			Description: item.Description,
			Status:      EffectiveServerStatusExtension,
			Source:      "extension:" + item.Source.ExtensionID,
		}
		if _, exists := persisted[item.ID]; exists {
			entry.Status = EffectiveServerStatusConflicted
			entry.ConflictWith = "persisted:" + item.ID
		}
		result.Servers = append(result.Servers, entry)
	}

	sort.Slice(result.Servers, func(i, j int) bool {
		if result.Servers[i].Name == result.Servers[j].Name {
			return result.Servers[i].Key < result.Servers[j].Key
		}
		return result.Servers[i].Name < result.Servers[j].Name
	})

	return result
}

func cloneEnv(env map[string]string) map[string]string {
	if len(env) == 0 {
		return nil
	}

	result := make(map[string]string, len(env))
	for key, value := range env {
		result[key] = value
	}
	return result
}
