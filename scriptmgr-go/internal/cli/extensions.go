package cli

import (
	"fmt"

	"scriptmgr/internal/extensions"
)

func runExtensionsCommand(args []string, stateDir string) error {
	subcommand := "list"
	if len(args) > 0 && args[0] != "--json" {
		subcommand = args[0]
		args = args[1:]
	}

	switch subcommand {
	case "list":
		return runExtensionsList(args, stateDir)
	case "--help", "-h", "help":
		printExtensionsUsage()
		return nil
	default:
		return fmt.Errorf("unknown extensions subcommand: %s", subcommand)
	}
}

func runExtensionsList(args []string, stateDir string) error {
	asJSON := contains(args, "--json")

	registry, err := extensions.NewRegistry(stateDir)
	if err != nil {
		return err
	}

	items, err := registry.List()
	if err != nil {
		return err
	}

	if asJSON {
		return writeJSON(map[string]any{
			"ok":         true,
			"roots":      registry.Roots(),
			"count":      len(items),
			"extensions": items,
		})
	}

	fmt.Printf("Extension roots (%d):\n", len(registry.Roots()))
	for _, root := range registry.Roots() {
		fmt.Printf("  %s\n", root)
	}
	fmt.Println()

	if len(items) == 0 {
		fmt.Println("No extensions found.")
		return nil
	}

	fmt.Printf("Extensions (%d):\n", len(items))
	for _, item := range items {
		if item.Manifest == nil {
			fmt.Printf("  [%-7s] %s (%s)\n", item.Status, item.ManifestPath, item.Error)
			continue
		}
		fmt.Printf("  [%-7s] %-18s %s\n", item.Status, item.Manifest.ID, item.Manifest.Version)
		if item.Error != "" {
			fmt.Printf("             error: %s\n", item.Error)
		}
	}

	return nil
}

func printExtensionsUsage() {
	fmt.Println("Usage: scriptmgr extensions [list] [--json]")
}
