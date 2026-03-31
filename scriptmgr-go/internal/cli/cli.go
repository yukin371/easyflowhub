package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"sort"
	"strconv"
	"strings"
	"time"

	"scriptmgr/internal/api"
	"scriptmgr/internal/model"
)

const version = "1.0.0"

func Run(args []string, svc *api.API) error {
	// Handle global flags before command processing
	for i := 0; i < len(args); i++ {
		arg := args[i]
		if arg == "--version" || arg == "-v" {
			fmt.Printf("scriptmgr version %s\n", version)
			return nil
		}
		if arg == "--help" || arg == "-h" {
			printUsage()
			return nil
		}
		// Stop processing flags at first non-flag argument
		if !strings.HasPrefix(arg, "-") {
			break
		}
	}

	if len(args) == 0 {
		printUsage()
		return errors.New("missing command")
	}

	switch args[0] {
	case "list":
		return runList(args[1:], svc)
	case "describe":
		return runDescribe(args[1:], svc)
	case "run":
		return runScript(args[1:], svc)
	case "__worker":
		return runWorker(args[1:], svc)
	case "history":
		return runHistory(args[1:], svc)
	case "tasks":
		return runTasks(args[1:], svc)
	case "status":
		return runStatus(args[1:], svc)
	case "log":
		return runLog(args[1:], svc)
	case "favorites":
		return runFavorites(args[1:], svc)
	case "roots":
		return runRoots(args[1:], svc)
	case "sessions":
		return runSessions(args[1:], svc)
	case "cancel":
		return runCancel(args[1:], svc)
	case "mcp":
		return runMCPCommand(args[1:], svc)
	case "mcp-add":
		return runMCPAdd(args[1:])
	case "mcp-remove":
		return runMCPRemove(args[1:])
	case "mcp-import-claude":
		return runMCPImportClaude()
	case "serve":
		return runServe(args[1:], svc, svc.StateDir())
	default:
		printUsage()
		return fmt.Errorf("unknown command: %s", args[0])
	}
}

func runList(args []string, svc *api.API) error {
	var asJSON bool
	var search string
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--json":
			asJSON = true
		case "--search":
			i++
			if i >= len(args) {
				return errors.New("missing value for --search")
			}
			search = args[i]
		default:
			return fmt.Errorf("unknown option for list: %s", args[i])
		}
	}

	scripts, roots, err := svc.ListScripts(search)
	if err != nil {
		return err
	}

	if asJSON {
		return writeJSON(model.ListResponse{
			OK:          true,
			Count:       len(scripts),
			Search:      search,
			Roots:       roots,
			Scripts:     scripts,
			GeneratedAt: nowRFC3339(),
		})
	}
	printTable(scripts)
	return nil
}

func runDescribe(args []string, svc *api.API) error {
	if len(args) == 0 {
		return errors.New("missing script id for describe")
	}
	scriptID := args[0]
	asJSON := contains(args[1:], "--json")

	script, err := svc.DescribeScript(scriptID)
	if err != nil {
		return err
	}
	if asJSON {
		return writeJSON(model.DescribeResponse{
			OK:          true,
			Script:      script,
			GeneratedAt: nowRFC3339(),
		})
	}

	fmt.Printf("id:          %s\n", script.ID)
	fmt.Printf("name:        %s\n", script.Name)
	fmt.Printf("type:        %s\n", script.ScriptType)
	fmt.Printf("path:        %s\n", script.Path)
	fmt.Printf("source_root: %s\n", valueOrDash(script.SourceRoot))
	fmt.Printf("category:    %s\n", valueOrDash(script.Category))
	fmt.Printf("tags:        %s\n", tagsOrDash(script.Tags))
	fmt.Printf("author:      %s\n", valueOrDash(script.Author))
	fmt.Printf("version:     %s\n", valueOrDash(script.Version))
	fmt.Printf("description: %s\n", valueOrDash(script.Description))
	return nil
}

func runScript(args []string, svc *api.API) error {
	if len(args) == 0 {
		return errors.New("missing script id for run")
	}
	scriptID := args[0]
	opts := model.RunOptions{}
	var scriptArgs []string

	for _, arg := range args[1:] {
		switch arg {
		case "--dry-run":
			opts.DryRun = true
		case "--json":
			opts.AsJSON = true
		case "--capture-output":
			opts.CaptureOutput = true
		case "--async":
			opts.Detach = true
		case "--detach":
			opts.Detach = true
		default:
			scriptArgs = append(scriptArgs, arg)
		}
	}

	result, err := svc.RunScript(scriptID, scriptArgs, opts)
	if opts.AsJSON {
		if result != nil {
			return writeJSON(result)
		}
		return err
	}
	if err != nil {
		if runResult, ok := result.(model.RunResult); ok && runResult.Output != "" {
			fmt.Print(runResult.Output)
		}
		return err
	}
	if runResult, ok := result.(model.RunResult); ok {
		fmt.Print(runResult.Output)
	}
	if session, ok := result.(model.SessionRecord); ok {
		fmt.Printf("started session %s (pid %d)\n", session.SessionID, session.PID)
	}
	return nil
}

func runWorker(args []string, svc *api.API) error {
	if len(args) < 2 {
		return errors.New("worker requires session id and script id")
	}
	return svc.RunWorker(args[0], args[1], args[2:])
}

func runHistory(args []string, svc *api.API) error {
	asJSON := contains(args, "--json")
	limit := 20
	for i := 0; i < len(args); i++ {
		if args[i] == "--limit" && i+1 < len(args) {
			parsed, err := strconv.Atoi(args[i+1])
			if err != nil {
				return fmt.Errorf("invalid --limit value: %s", args[i+1])
			}
			limit = parsed
		}
	}

	history, err := svc.History(limit)
	if err != nil {
		return err
	}
	if asJSON {
		return writeJSON(model.HistoryResponse{
			OK:          true,
			Count:       len(history),
			Entries:     history,
			GeneratedAt: nowRFC3339(),
		})
	}
	if len(history) == 0 {
		fmt.Println("No history entries.")
		return nil
	}
	for _, entry := range history {
		fmt.Printf("%s  %s  %s  exit=%s  %s\n",
			entry.StartedAt, entry.Status, entry.ScriptID, exitCodeString(entry.ExitCode), entry.ScriptName)
	}
	return nil
}

func runTasks(args []string, svc *api.API) error {
	asJSON := contains(args, "--json")
	limit := 20
	status := ""
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--json":
			asJSON = true
		case "--limit":
			i++
			if i >= len(args) {
				return errors.New("missing value for --limit")
			}
			parsed, err := strconv.Atoi(args[i])
			if err != nil {
				return fmt.Errorf("invalid --limit value: %s", args[i])
			}
			limit = parsed
		case "--status":
			i++
			if i >= len(args) {
				return errors.New("missing value for --status")
			}
			status = args[i]
		default:
			return fmt.Errorf("unknown option for tasks: %s", args[i])
		}
	}

	tasks, err := svc.ListTasks(status, limit)
	if err != nil {
		return err
	}
	if asJSON {
		return writeJSON(map[string]any{
			"ok":           true,
			"count":        len(tasks),
			"status":       status,
			"tasks":        tasks,
			"generated_at": nowRFC3339(),
		})
	}
	if len(tasks) == 0 {
		fmt.Println("No tasks.")
		return nil
	}
	for _, task := range tasks {
		fmt.Printf("%s  %s  %s  %s\n", task.TaskID, task.Status, task.ScriptID, task.CreatedAt.Format(time.RFC3339))
	}
	return nil
}

func runStatus(args []string, svc *api.API) error {
	if len(args) == 0 {
		return errors.New("missing task id for status")
	}
	asJSON := contains(args[1:], "--json")
	task, err := svc.GetTask(args[0])
	if err != nil {
		return err
	}
	if asJSON {
		return writeJSON(task)
	}
	fmt.Printf("task_id:      %s\n", task.TaskID)
	fmt.Printf("script_id:    %s\n", task.ScriptID)
	fmt.Printf("status:       %s\n", task.Status)
	fmt.Printf("created_at:   %s\n", task.CreatedAt.Format(time.RFC3339))
	if task.StartedAt != nil {
		fmt.Printf("started_at:   %s\n", task.StartedAt.Format(time.RFC3339))
	}
	if task.CompletedAt != nil {
		fmt.Printf("completed_at: %s\n", task.CompletedAt.Format(time.RFC3339))
	}
	fmt.Printf("output_path:  %s\n", valueOrDash(task.OutputPath))
	fmt.Printf("summary:      %s\n", valueOrDash(task.OutputSummary))
	fmt.Printf("error:        %s\n", valueOrDash(task.Error))
	return nil
}

func runLog(args []string, svc *api.API) error {
	if len(args) == 0 {
		return errors.New("missing task id for log")
	}

	taskID := ""
	offset := 0
	limit := 100
	tail := false
	asJSON := false

	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--offset":
			i++
			if i >= len(args) {
				return errors.New("missing value for --offset")
			}
			parsed, err := strconv.Atoi(args[i])
			if err != nil {
				return fmt.Errorf("invalid --offset value: %s", args[i])
			}
			offset = parsed
		case "--limit":
			i++
			if i >= len(args) {
				return errors.New("missing value for --limit")
			}
			parsed, err := strconv.Atoi(args[i])
			if err != nil {
				return fmt.Errorf("invalid --limit value: %s", args[i])
			}
			limit = parsed
		case "--tail":
			tail = true
		case "--json":
			asJSON = true
		default:
			if taskID != "" {
				return fmt.Errorf("unknown option for log: %s", args[i])
			}
			taskID = args[i]
		}
	}

	if taskID == "" {
		return errors.New("missing task id for log")
	}

	output, task, err := svc.ReadTaskLog(taskID, offset, limit, tail)
	if err != nil {
		return err
	}
	if asJSON {
		return writeJSON(map[string]any{
			"ok":           true,
			"task_id":      taskID,
			"status":       task.Status,
			"offset":       offset,
			"limit":        limit,
			"tail":         tail,
			"log_path":     task.OutputPath,
			"output":       output,
			"generated_at": nowRFC3339(),
		})
	}
	fmt.Print(output)
	return nil
}

func runFavorites(args []string, svc *api.API) error {
	if len(args) == 0 {
		args = []string{"list"}
	}
	switch args[0] {
	case "list":
		asJSON := contains(args[1:], "--json")
		favorites, err := svc.FavoriteScripts()
		if err != nil {
			return err
		}
		if asJSON {
			return writeJSON(model.FavoritesResponse{
				OK:          true,
				Count:       len(favorites),
				Favorites:   favorites,
				GeneratedAt: nowRFC3339(),
			})
		}
		if len(favorites) == 0 {
			fmt.Println("No favorites.")
			return nil
		}
		printTable(favorites)
		return nil
	case "add":
		if len(args) < 2 {
			return errors.New("missing script id for favorites add")
		}
		asJSON := contains(args[2:], "--json")
		id, err := svc.AddFavorite(args[1])
		if err != nil {
			return err
		}
		if asJSON {
			return writeJSON(map[string]any{"ok": true, "action": "add", "script_id": id, "generated_at": nowRFC3339()})
		}
		fmt.Printf("added favorite: %s\n", id)
		return nil
	case "remove":
		if len(args) < 2 {
			return errors.New("missing script id for favorites remove")
		}
		asJSON := contains(args[2:], "--json")
		if err := svc.RemoveFavorite(args[1]); err != nil {
			return err
		}
		if asJSON {
			return writeJSON(map[string]any{"ok": true, "action": "remove", "script_id": args[1], "generated_at": nowRFC3339()})
		}
		fmt.Printf("removed favorite: %s\n", args[1])
		return nil
	default:
		return fmt.Errorf("unknown favorites command: %s", args[0])
	}
}

func runRoots(args []string, svc *api.API) error {
	if len(args) == 0 {
		args = []string{"list"}
	}
	switch args[0] {
	case "list":
		asJSON := contains(args[1:], "--json")
		roots, err := svc.ListRoots()
		if err != nil {
			return err
		}
		if asJSON {
			return writeJSON(model.RootsResponse{OK: true, Count: len(roots), Roots: roots, GeneratedAt: nowRFC3339()})
		}
		if len(roots) == 0 {
			fmt.Println("No custom roots.")
			return nil
		}
		for _, root := range roots {
			fmt.Println(root)
		}
		return nil
	case "add":
		if len(args) < 2 {
			return errors.New("missing path for roots add")
		}
		asJSON := contains(args[2:], "--json")
		root, err := svc.AddRoot(args[1])
		if err != nil {
			return err
		}
		if asJSON {
			return writeJSON(map[string]any{"ok": true, "action": "add", "path": root, "generated_at": nowRFC3339()})
		}
		fmt.Printf("added root: %s\n", root)
		return nil
	case "remove":
		if len(args) < 2 {
			return errors.New("missing path for roots remove")
		}
		asJSON := contains(args[2:], "--json")
		if err := svc.RemoveRoot(args[1]); err != nil {
			return err
		}
		if asJSON {
			return writeJSON(map[string]any{"ok": true, "action": "remove", "path": args[1], "generated_at": nowRFC3339()})
		}
		fmt.Printf("removed root: %s\n", args[1])
		return nil
	default:
		return fmt.Errorf("unknown roots command: %s", args[0])
	}
}

func runSessions(args []string, svc *api.API) error {
	asJSON := contains(args, "--json")
	sessions, err := svc.Sessions()
	if err != nil {
		return err
	}
	if asJSON {
		return writeJSON(model.SessionsResponse{
			OK:          true,
			Count:       len(sessions),
			Sessions:    sessions,
			GeneratedAt: nowRFC3339(),
		})
	}
	if len(sessions) == 0 {
		fmt.Println("No sessions.")
		return nil
	}
	for _, session := range sessions {
		fmt.Printf("%s  pid=%d  %s  %s\n", session.SessionID, session.PID, session.Status, session.ScriptName)
	}
	return nil
}

func runCancel(args []string, svc *api.API) error {
	if len(args) == 0 {
		return errors.New("missing task/session id for cancel")
	}
	asJSON := contains(args[1:], "--json")
	session, err := svc.CancelTask(args[0])
	if err != nil {
		return err
	}
	if asJSON {
		return writeJSON(model.CancelResponse{OK: true, Session: session, GeneratedAt: nowRFC3339()})
	}
	fmt.Printf("cancelled session: %s\n", session.SessionID)
	return nil
}

func printTable(scripts []model.ScriptRecord) {
	if len(scripts) == 0 {
		fmt.Println("No scripts found.")
		return
	}
	idWidth := len("ID")
	typeWidth := len("TYPE")
	nameWidth := len("NAME")
	rootWidth := len("ROOT")
	for _, script := range scripts {
		idWidth = max(idWidth, len(script.ID))
		typeWidth = max(typeWidth, len(script.ScriptType))
		nameWidth = max(nameWidth, len(script.Name))
		rootWidth = max(rootWidth, len(shortRoot(script.SourceRoot)))
	}
	header := fmt.Sprintf("%-*s  %-*s  %-*s  %-*s  DESCRIPTION", idWidth, "ID", typeWidth, "TYPE", nameWidth, "NAME", rootWidth, "ROOT")
	fmt.Println(header)
	fmt.Println(strings.Repeat("-", len(header)))
	for _, script := range scripts {
		fmt.Printf("%-*s  %-*s  %-*s  %-*s  %s\n",
			idWidth, script.ID,
			typeWidth, script.ScriptType,
			nameWidth, script.Name,
			rootWidth, shortRoot(script.SourceRoot),
			script.Description)
	}
}

func shortRoot(root string) string {
	if root == "" {
		return "-"
	}
	parts := strings.FieldsFunc(root, func(r rune) bool { return r == '\\' || r == '/' })
	if len(parts) <= 2 {
		return root
	}
	return strings.Join(parts[len(parts)-2:], "/")
}

func writeJSON(value any) error {
	encoded, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	fmt.Println(string(encoded))
	return nil
}

func printUsage() {
	fmt.Println("scriptmgr - Script management tool")
	fmt.Println()
	fmt.Println("Usage: scriptmgr [flags] <command> [args...]")
	fmt.Println()
	fmt.Println("Flags:")
	fmt.Println("  --version, -v    Show version")
	fmt.Println("  --help, -h       Show this help")
	fmt.Println()
	fmt.Println("Commands:")
	fmt.Println("  list [--json] [--search TEXT]")
	fmt.Println("  describe <script-id> [--json]")
	fmt.Println("  run <script-id> [--dry-run] [--json] [--capture-output] [--detach|--async] [args...]")
	fmt.Println("  history [--json] [--limit N]")
	fmt.Println("  tasks [--json] [--status STATUS] [--limit N]")
	fmt.Println("  status <task-id> [--json]")
	fmt.Println("  log <task-id> [--offset N] [--limit N] [--tail] [--json]")
	fmt.Println("  favorites [list|add|remove] [script-id] [--json]")
	fmt.Println("  roots [list|add|remove] [path] [--json]")
	fmt.Println("  sessions [--json]")
	fmt.Println("  cancel <task-or-session-id> [--json]")
	fmt.Println("  mcp                      - Start MCP server (for AI assistants)")
	fmt.Println("  mcp <server>             - List tools for an MCP server")
	fmt.Println("  mcp <server> <tool> [args...] - Call an MCP tool")
	fmt.Println("  mcp-add <name> <command> - Add an MCP server")
	fmt.Println("  mcp-remove <name>        - Remove an MCP server")
	fmt.Println("  mcp-import-claude        - Import MCP servers from Claude config")
	fmt.Println("  serve [addr]             - Start HTTP API server (default :8765)")
}

func tagsOrDash(tags []string) string {
	if len(tags) == 0 {
		return "-"
	}
	return strings.Join(tags, ", ")
}

func valueOrDash(value string) string {
	if value == "" {
		return "-"
	}
	return value
}

func exitCodeString(code *int) string {
	if code == nil {
		return "-"
	}
	return strconv.Itoa(*code)
}

func contains(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func nowRFC3339() string {
	return time.Now().Format(time.RFC3339)
}

func readAll(r io.Reader) string {
	content, _ := io.ReadAll(r)
	return string(content)
}

func sortedCopy(values []string) []string {
	cloned := append([]string(nil), values...)
	sort.Strings(cloned)
	return cloned
}
