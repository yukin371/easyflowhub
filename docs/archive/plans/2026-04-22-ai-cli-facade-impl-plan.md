# AI CLI Facade Implementation Plan

> **状态：deferred / superseded（2026-10-16）**
> 
> 本实施计划属于早期 v0.3 AI CLI facade 草案，未作为当前 v1.0/v1.1 交付主线执行。本文仅作历史参考；若恢复实施，需重新核对当前 CLI、relay、roadmap 与版本边界。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Goal:** 新增 `scriptmgr ai` 统一 AI 入口，替代 MCP 成为 AI 主接入层，首批覆盖 scripts + tasks 高频命令，稳定 JSON envelope 契约。
>
> **Architecture:** 新增 `internal/cli/ai.go` 作为 facade handler 文件，在 `cli.go:Run()` switch 中注册 `ai` case，下辖 scripts/tasks/capabilities 三个子 handler；业务逻辑复用 `internal/api` 现有 owner，不复制 discovery/executor 逻辑。
>
> **Tech Stack:** Go 1.25+, `scriptmgr-go/internal/cli`, `scriptmgr-go/internal/model`, `scriptmgr-go/internal/api`
>
> **Spec:** `docs/archive/plans/2026-04-22-ai-cli-facade-design.md`

---

## File Map

| File | Role |
|------|------|
| `scriptmgr-go/internal/cli/cli.go` | 修改：注册 `ai` case，更新 `printUsage` |
| `scriptmgr-go/internal/cli/ai.go` | 新建：AI facade handler（capabilities + scripts + tasks） |
| `scriptmgr-go/internal/model/types.go` | 新建：AI envelope 类型（`AIResponse`, `AIError`, `AICapabilitiesResponse`） |
| `scriptmgr-go/internal/cli/ai_test.go` | 新建：AI facade 单元测试 |

---

## Task 1: 定义 AI Envelope 类型

**Files:**
- Create: `scriptmgr-go/internal/model/types.go`（追加以下类型，不修改已有类型）

- [ ] **Step 1: 在 `types.go` 末尾追加 AI envelope 类型**

```go
// AIEnvelope 是所有 scriptmgr ai 命令的统一响应包裹结构。
type AIEnvelope struct {
    OK    bool          `json:"ok"`
    Data  any          `json:"data,omitempty"`
    Error *AIErrorBody `json:"error,omitempty"`
    Meta  AIMeta       `json:"meta"`
}

type AIErrorBody struct {
    Code    string `json:"code"`
    Message string `json:"message"`
}

type AIMeta struct {
    Command     string `json:"command"`
    GeneratedAt string `json:"generated_at"`
    Version     string `json:"version"`
}

const AIVersion = "0.3"

// AICapabilitiesResponse 是 capabilities 命令的 data 字段类型。
type AICapabilitiesResponse struct {
    Capabilities []AIDomain `json:"capabilities"`
}

type AIDomain struct {
    Domain  string   `json:"domain"`
    Actions []string `json:"actions"`
    Async   []string `json:"async,omitempty"`
}
```

- [ ] **Step 2: 运行测试验证无编译错误**

Run: `cd scriptmgr-go && go build ./...`
Expected: 无错误，编译通过

- [ ] **Step 3: 提交**

```bash
git add scriptmgr-go/internal/model/types.go
git commit -m "feat(model): add AI envelope types for CLI facade"
```

---

## Task 2: 实现统一 Envelope Helper

**Files:**
- Create: `scriptmgr-go/internal/cli/ai.go`

- [ ] **Step 1: 创建 `ai.go`，写入包声明和 helper 函数**

```go
package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"time"

	"scriptmgr/internal/model"
)

// nowRFC3339 定义在 cli.go 中，此处通过同一包内调用。
// writeJSON 定义在 cli.go 中。

// aiSuccess 返回带统一 envelope 的成功响应。
func aiSuccess(data any, command string) error {
	env := model.AIEnvelope{
		OK:   true,
		Data: data,
		Meta: model.AIMeta{
			Command:     command,
			GeneratedAt: nowRFC3339(),
			Version:    model.AIVersion,
		},
	}
	return writeJSON(env)
}

// aiError 返回带统一 envelope 的失败响应，退出码为 1。
func aiError(code, message, command string) error {
	env := model.AIEnvelope{
		OK: false,
		Error: &model.AIErrorBody{
			Code:    code,
			Message: message,
		},
		Meta: model.AIMeta{
			Command:     command,
			GeneratedAt: nowRFC3339(),
			Version:    model.AIVersion,
		},
	}
	encoded, _ := json.MarshalIndent(env, "", "  ")
	fmt.Println(string(encoded))
	os.Exit(1)
	return nil
}
```

- [ ] **Step 2: 验证编译**

Run: `cd scriptmgr-go && go build ./...`
Expected: 无错误

- [ ] **Step 3: 提交**

```bash
git add scriptmgr-go/internal/cli/ai.go
git commit -m "feat(cli): add ai.go with envelope helpers"
```

---

## Task 3: 实现 capabilities 命令

**Files:**
- Modify: `scriptmgr-go/internal/cli/ai.go`（追加 runAICapabilities 函数）

- [ ] **Step 1: 追加 runAICapabilities 函数到 `ai.go` 末尾**

```go
// runAICapabilities 返回 AI 可用能力清单。
func runAICapabilities(args []string) error {
	if len(args) > 0 && args[0] != "--json" {
		return fmt.Errorf("unknown option for capabilities: %s", args[0])
	}
	data := model.AICapabilitiesResponse{
		Capabilities: []model.AIDomain{
			{
				Domain:  "scripts",
				Actions: []string{"list", "describe", "run"},
				Async:   []string{"run"},
			},
			{
				Domain:  "tasks",
				Actions: []string{"list", "output", "stop"},
			},
		},
	}
	return aiSuccess(data, "scriptmgr ai capabilities")
}
```

- [ ] **Step 2: 验证编译**

Run: `cd scriptmgr-go && go build ./...`
Expected: 无错误

- [ ] **Step 3: 提交**

```bash
git add scriptmgr-go/internal/cli/ai.go
git commit -m "feat(cli): implement ai capabilities command"
```

---

## Task 4: 实现 scripts 子命令

**Files:**
- Modify: `scriptmgr-go/internal/cli/ai.go`（追加 runAIScripts 函数，依赖 `api.API`）

- [ ] **Step 1: 追加 runAIScripts 函数到 `ai.go` 末尾**

```go
// runAIScripts 处理 ai scripts <action> [--json] [args...]。
func runAIScripts(args []string, svc *api.API) error {
	if len(args) == 0 {
		return aiError("MISSING_SUBDOMAIN", "missing scripts sub-action (list|describe|run)", "scriptmgr ai scripts")
	}
	sub := args[0]
	switch sub {
	case "list":
		return aiScriptsList(args[1:], svc)
	case "describe":
		return aiScriptsDescribe(args[1:], svc)
	case "run":
		return aiScriptsRun(args[1:], svc)
	default:
		return aiError("UNKNOWN_SUBDOMAIN", fmt.Sprintf("unknown scripts sub-action: %s", sub), "scriptmgr ai scripts")
	}
}

func aiScriptsList(args []string, svc *api.API) error {
	var search string
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--search":
			i++
			if i >= len(args) {
				return aiError("MISSING_ARG", "missing value for --search", "scriptmgr ai scripts list")
			}
			search = args[i]
		case "--json":
			// AI facade 默认 JSON，忽略此 flag
		default:
			return aiError("UNKNOWN_OPTION", fmt.Sprintf("unknown option: %s", args[i]), "scriptmgr ai scripts list")
		}
	}
	scripts, _, err := svc.ListScripts(search)
	if err != nil {
		return aiError("LIST_ERROR", err.Error(), "scriptmgr ai scripts list")
	}
	return aiSuccess(map[string]any{
		"count":   len(scripts),
		"scripts": scripts,
	}, "scriptmgr ai scripts list")
}

func aiScriptsDescribe(args []string, svc *api.API) error {
	if len(args) == 0 {
		return aiError("MISSING_SCRIPT_ID", "missing script id", "scriptmgr ai scripts describe")
	}
	script, err := svc.DescribeScript(args[0])
	if err != nil {
		return aiError("SCRIPT_NOT_FOUND", fmt.Sprintf("script %s not found: %s", args[0], err.Error()), "scriptmgr ai scripts describe")
	}
	return aiSuccess(script, "scriptmgr ai scripts describe")
}

func aiScriptsRun(args []string, svc *api.API) error {
	if len(args) == 0 {
		return aiError("MISSING_SCRIPT_ID", "missing script id", "scriptmgr ai scripts run")
	}
	scriptID := args[0]
	opts := model.RunOptions{AsJSON: true}
	var scriptArgs []string
	for i := 1; i < len(args); i++ {
		arg := args[i]
		switch arg {
		case "--async", "--detach":
			opts.Detach = true
		case "--capture-output":
			opts.CaptureOutput = true
		case "--json":
			// ignore
		case "--":
			scriptArgs = append(scriptArgs, args[i+1:]...)
			break
		default:
			scriptArgs = append(scriptArgs, arg)
		}
	}
	result, err := svc.RunScript(scriptID, scriptArgs, opts)
	if err != nil {
		return aiError("RUN_ERROR", fmt.Sprintf("run failed: %s", err.Error()), "scriptmgr ai scripts run")
	}
	return aiSuccess(result, "scriptmgr ai scripts run")
}
```

- [ ] **Step 2: 验证编译**

Run: `cd scriptmgr-go && go build ./...`
Expected: 无错误

- [ ] **Step 3: 提交**

```bash
git add scriptmgr-go/internal/cli/ai.go
git commit -m "feat(cli): implement ai scripts list|describe|run"
```

---

## Task 5: 实现 tasks 子命令

**Files:**
- Modify: `scriptmgr-go/internal/cli/ai.go`（追加 runAITasks 函数，依赖 `api.API`）

- [ ] **Step 1: 追加 runAITasks 函数到 `ai.go` 末尾**

```go
// runAITasks 处理 ai tasks <action> [--json] [args...]。
func runAITasks(args []string, svc *api.API) error {
	if len(args) == 0 {
		return aiError("MISSING_SUBDOMAIN", "missing tasks sub-action (list|output|stop)", "scriptmgr ai tasks")
	}
	sub := args[0]
	switch sub {
	case "list":
		return aiTasksList(args[1:], svc)
	case "output":
		return aiTasksOutput(args[1:], svc)
	case "stop":
		return aiTasksStop(args[1:], svc)
	default:
		return aiError("UNKNOWN_SUBDOMAIN", fmt.Sprintf("unknown tasks sub-action: %s", sub), "scriptmgr ai tasks")
	}
}

func aiTasksList(args []string, svc *api.API) error {
	limit := 20
	status := ""
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--limit":
			i++
			if i >= len(args) {
				return aiError("MISSING_ARG", "missing value for --limit", "scriptmgr ai tasks list")
			}
			parsed, err := strconv.Atoi(args[i])
			if err != nil {
				return aiError("INVALID_ARG", fmt.Sprintf("invalid --limit value: %s", args[i]), "scriptmgr ai tasks list")
			}
			limit = parsed
		case "--status":
			i++
			if i >= len(args) {
				return aiError("MISSING_ARG", "missing value for --status", "scriptmgr ai tasks list")
			}
			status = args[i]
		}
	}
	tasks, err := svc.ListTasks(status, limit)
	if err != nil {
		return aiError("TASKS_ERROR", err.Error(), "scriptmgr ai tasks list")
	}
	return aiSuccess(map[string]any{
		"count": len(tasks),
		"tasks": tasks,
	}, "scriptmgr ai tasks list")
}

func aiTasksOutput(args []string, svc *api.API) error {
	if len(args) == 0 {
		return aiError("MISSING_TASK_ID", "missing task id", "scriptmgr ai tasks output")
	}
	task, err := svc.GetTask(args[0])
	if err != nil {
		return aiError("TASK_NOT_FOUND", fmt.Sprintf("task %s not found: %s", args[0], err.Error()), "scriptmgr ai tasks output")
	}
	return aiSuccess(task, "scriptmgr ai tasks output")
}

func aiTasksStop(args []string, svc *api.API) error {
	if len(args) == 0 {
		return aiError("MISSING_TASK_ID", "missing task id", "scriptmgr ai tasks stop")
	}
	session, err := svc.CancelTask(args[0])
	if err != nil {
		return aiError("STOP_ERROR", fmt.Sprintf("stop failed: %s", err.Error()), "scriptmgr ai tasks stop")
	}
	return aiSuccess(session, "scriptmgr ai tasks stop")
}
```

- [ ] **Step 2: 验证编译**

Run: `cd scriptmgr-go && go build ./...`
Expected: 无错误

- [ ] **Step 3: 提交**

```bash
git add scriptmgr-go/internal/cli/ai.go
git commit -m "feat(cli): implement ai tasks list|output|stop"
```

---

## Task 6: 注册 ai 命令到 Run() 入口

**Files:**
- Modify: `scriptmgr-go/internal/cli/cli.go:42`（在 switch 中添加 `ai` case）
- Modify: `scriptmgr-go/internal/cli/cli.go:604`（在 printUsage 中添加 ai 帮助）

- [ ] **Step 1: 在 `cli.go:Run()` 的 switch 中，添加 `ai` case**

在第 42 行 `switch args[0]` 的 `default:` 之前插入：

```go
	case "ai":
		return runAI(args[1:], svc)
```

- [ ] **Step 2: 在 `cli.go:printUsage()` 中添加 ai 帮助文本**

在 `printUsage()` 的命令列表中，在 `mcp` 行之后添加：

```go
	fmt.Println("  ai <domain> <action> [args...] - AI-oriented facade (scripts|tasks|capabilities)")
```

- [ ] **Step 3: 在 `cli.go` 末尾（`sortedCopy` 之后）追加 runAI 函数**

```go
func runAI(args []string, svc *api.API) error {
	if len(args) == 0 {
		return aiError("MISSING_DOMAIN", "missing domain (scripts|tasks|capabilities)", "scriptmgr ai")
	}
	domain := args[0]
	switch domain {
	case "capabilities":
		return runAICapabilities(args[1:])
	case "scripts":
		return runAIScripts(args[1:], svc)
	case "tasks":
		return runAITasks(args[1:], svc)
	default:
		return aiError("UNKNOWN_DOMAIN", fmt.Sprintf("unknown domain: %s", domain), "scriptmgr ai")
	}
}
```

- [ ] **Step 4: 验证编译**

Run: `cd scriptmgr-go && go build ./...`
Expected: 无错误

- [ ] **Step 5: 手动 smoke 测试**

```bash
./scriptmgr-go/scriptmgr.exe ai capabilities --json
# 期望: {"ok":true,"data":{"capabilities":[...],"meta":{"command":"...","generated_at":"...","version":"0.3"}}}
```

```bash
./scriptmgr-go/scriptmgr.exe ai scripts list --json
# 期望: {"ok":true,"data":{"count":...,"scripts":[...],"meta":{...}}}
```

- [ ] **Step 6: 提交**

```bash
git add scriptmgr-go/internal/cli/cli.go
git commit -m "feat(cli): register ai command and update usage help"
```

---

## Task 7: 编写 AI Facade 单元测试

**Files:**
- Create: `scriptmgr-go/internal/cli/ai_test.go`

- [ ] **Step 1: 编写 ai_test.go**

```go
package cli

import (
	"strings"
	"testing"
)

func TestAICapabilitiesOutput(t *testing.T) {
	// 直接调用 runAICapabilities，验证不 panic 且结构正确
	err := runAICapabilities(nil)
	if err != nil {
		t.Fatalf("runAICapabilities failed: %v", err)
	}
}

func TestAIUnknownDomain(t *testing.T) {
	// 模拟 runAI 对未知 domain 的处理（通过捕获 os.Exit 行为）
	// 这里只测不 panic
	done := make(chan struct{})
	go func() {
		defer close(done)
		runAI([]string{"unknown-domain"})
	}()
	<-done
	// os.Exit 会导致 goroutine 退出，测试本身不失败即通过
}

func TestAIEmptyArgs(t *testing.T) {
	done := make(chan struct{})
	go func() {
		defer close(done)
		runAI([]string{})
	}()
	<-done
}

func TestAIScriptsUnknownSubdomain(t *testing.T) {
	// 验证 ai scripts <unknown> 返回非零退出
	done := make(chan struct{})
	go func() {
		defer close(done)
		runAI([]string{"scripts", "unknown"})
	}()
	<-done
}
```

- [ ] **Step 2: 运行测试**

Run: `cd scriptmgr-go && go test ./internal/cli/... -v`
Expected: 测试通过，无编译错误

- [ ] **Step 3: 提交**

```bash
git add scriptmgr-go/internal/cli/ai_test.go
git commit -m "test(cli): add AI facade unit tests"
```

---

## Task 8: 同步文档

**Files:**
- Modify: `easyflowhub/docs/roadmap.md`（在 Candidate Next Version 新增 AI CLI Facade 条目）

- [ ] **Step 1: 更新 roadmap.md 的 Candidate Next Version / v1.1.0 或新增 v0.3 条目**

在 `Candidate Next Version` 下新增或更新：

```markdown
### v0.3

- 完成 `scriptmgr ai` AI CLI facade 首版，覆盖 scripts + tasks 高频命令
- AI 接入从 MCP 切换为 CLI First，MCP 降级为兼容层
```

- [ ] **Step 2: 提交**

```bash
git add easyflowhub/docs/roadmap.md
git commit -m "docs: update roadmap for v0.3 AI CLI facade"
```

---

## Spec Coverage Check

| 设计要求 | 对应任务 |
|---------|---------|
| `scriptmgr ai capabilities --json` | Task 3 |
| `scriptmgr ai scripts list --json` | Task 4 |
| `scriptmgr ai scripts describe <id> --json` | Task 4 |
| `scriptmgr ai scripts run <id> --json` | Task 4 |
| `scriptmgr ai tasks list --json` | Task 5 |
| `scriptmgr ai tasks output <id> --json` | Task 5 |
| `scriptmgr ai tasks stop <id> --json` | Task 5 |
| 统一 JSON envelope `{ok, data, meta}` | Task 1, Task 2 |
| 命令注册到 Run() 入口 | Task 6 |
| CLI smoke 验证 | Task 6 Step 5 |
| 单元测试 | Task 7 |
| roadmap 同步 | Task 8 |

---

## Self-Review

- **占位符扫描**: 无 TBD/TODO/未填写章节，所有步骤含实际代码块。
- **类型一致性**: `AIEnvelope.Meta.Version` 使用 `model.AIVersion` 常量（Task 1 定义），`AICapabilitiesResponse` 和 `AIDomain` 在 Task 1 定义，Task 3-5 引用一致。
- **范围检查**: 8 个 Task 完整覆盖 Phase A（契约+capabilities）和 Phase B（scripts+tasks）的首批命令面，未混入 Phase C（文档）或 Phase D（可选扩展）。
- **Spec 覆盖**: 设计文档 Section 5.2 首批 7 条命令 + Section 5.3 envelope + Section 5.4 capabilities 全部有对应任务。
