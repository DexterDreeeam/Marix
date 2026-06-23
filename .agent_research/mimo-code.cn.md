# Xiaomi MiMo Code / MiMo Agent 外部源码研究

## 1. 来源与活跃度

- 官方 GitHub 组织：<https://github.com/XiaomiMiMo>
- 官方仓库：<https://github.com/XiaomiMiMo/MiMo-Code>
- 仓库描述：`MiMo Code: Where Models and Agents Co-Evolve`
- License：MIT。
- 默认分支：`main`。
- 研究素材中的活跃度证据：
  - `created_at`: 2026-06-10
  - `pushed_at`: 2026-06-22
  - `updated_at`: 2026-06-22
- 近期提交主题包含 `fix(metrics)`, `feat(skill)`, `feat(checkpoint)`, `feat(tool)` 等，说明 agent、tool、memory、checkpoint 相关模块仍在快速迭代。
- 未发现更准确、更官方的独立 “MiMo Agent” 仓库；`XiaomiMiMo/MiMo-Code` 是 MiMo Code / MiMo Agent 架构研究的最佳匹配对象。

## 2. 技术栈与项目性质

MiMo Code 是 terminal-native AI coding assistant。源码保留 OpenCode fork 的工程结构，并在其上扩展 MiMo 侧的 memory、checkpoint、task/subagent、goal-driven loop、compose、dream、distill 等能力。

| 层级 | 技术 |
|---|---|
| Runtime / 包管理 | Bun, TypeScript ESM |
| Agent / LLM | Vercel AI SDK `ai`, 多个 `@ai-sdk/*` provider |
| 状态与副作用 | Effect, Bus/SyncEvent, OpenTelemetry |
| CLI / TUI | yargs, SolidJS, OpenTUI |
| 存储 | SQLite, Drizzle ORM, FTS5 |
| 工具协议 | 内置 tool registry, plugin tools, MCP SDK |
| Shell 分析 | `tree-sitter-bash`, `tree-sitter-powershell` |
| Git / 文件变更 | diff, snapshot, patch/revert, watcher |

`packages/opencode/package.json` 暴露 CLI 包名 `@mimo-ai/cli`，bin 为 `mimo`。开发入口脚本为 `bun run --conditions=browser ./src/index.ts`。

## 3. 入口与模块

主要入口：

- `packages/opencode/src/index.ts`
  - yargs CLI 入口。
  - 注册 `run`, `generate`, `serve`, `mcp`, `agent`, `models`, `session`, `plugin`, `github`, `pr`, `db` 等命令。
  - 初始化日志、heap、环境变量、SQLite migration、Claude session import。
- `packages/opencode/src/session/prompt.ts`
  - 主 agent loop。
- `packages/opencode/src/session/processor.ts`
  - 消费模型 stream events，执行 tool call，更新 message parts。
- `packages/opencode/src/session/llm.ts`
  - 构造 system prompt、model messages、tools、provider request、retry stream。
- `packages/opencode/src/tool/registry.ts`
  - 注册内置工具、plugin tools 与 custom tools。
- `packages/opencode/src/agent/agent.ts`
  - 定义 primary agent、subagent、hidden/system agent 与内置模式。

核心模块图：

| 模块 | 关键路径 | 作用 |
|---|---|---|
| Agent 定义 | `agent/agent.ts` | `build`, `plan`, `compose`, `max`, `general`, `explore`, hidden agents |
| Agent loop | `session/prompt.ts` | 多轮模型调用、tool execution、overflow、goal/task gate |
| LLM 调用 | `session/llm.ts` | provider 适配、system prompt、tools、`streamText`、retry |
| Stream processor | `session/processor.ts` | reasoning/text/tool stream part 状态机 |
| Tool abstraction | `tool/tool.ts` | tool schema、execute、recoverable error、输出截断 |
| Tool registry | `tool/registry.ts` | read/edit/write/bash/grep/glob/actor/memory/task/patch 与 plugins |
| Subagent / Actor | `tool/actor.ts`, `actor/spawn.ts` | spawn/run/status/wait/cancel/send |
| Memory | `memory/service.ts` | SQLite FTS/BM25 memory recall |
| Checkpoint | `session/checkpoint.ts` | checkpoint writer、rebuild context、overflow recovery |
| Permission | `permission/evaluate.ts` | allow/ask/deny ruleset matching |
| MCP | `mcp/index.ts` | stdio/HTTP/SSE/OAuth MCP client |
| Task registry | `task/registry.ts` | create/list/start/done/block/abandon task state |
| Revert/diff | `session/revert.ts` | snapshot diff/restore/revert |

## 4. Agent loop

MiMo Code 的 agent loop 围绕 `session/prompt.ts` 的大循环实现。抽象流程：

1. 从 session 读取已 compact 的 message slice。
2. 根据 main agent 或 subagent actor 隔离上下文。
3. 注入 system prompt、provider prompt、memory recall hints、project/session/global memory instructions、skills、environment、本地指令。
4. 构造 LLM request prefix。
5. 进入 `session/llm.ts` 调用模型，使用 AI SDK stream。
6. `session/processor.ts` 消费 stream：
   - reasoning start/delta/end
   - text start/delta/end
   - tool input delta
   - tool call
   - tool result/error
   - finish/step finish
7. 如模型调用工具，执行工具并把结果写回 message history。
8. 如无工具调用，检查 final answer、goal judge、task gate、stop condition、invalid output retry、text repeat recovery。
9. 如上下文溢出：
   - subagent 走 per-actor compaction boundary；
   - main agent 优先等待 checkpoint writer 并 rebuild context；
   - checkpoint 不可用时 fallback 到 LLM compaction。
10. 循环直到完成、失败、abort 或其他终止条件。

值得借鉴的设计点：

- loop 中把模型输出分类做成明确状态：`filtered`, `failed`, `think-only`, `invalid`, `final`, `continue`。
- overflow 不是单点处理，而是 checkpoint、memory、compaction、tail preservation 的组合。
- subagent context 与 main context 分离，降低工具结果污染主上下文的风险。

## 5. 工具协议与模型适配

`tool/tool.ts` 的 `Tool.Def` 是统一抽象，包含：

- `id`
- `description`
- Zod `parameters`
- `execute`
- `formatValidationError`
- shell parser / recover 相关能力

`tool/registry.ts` 注册内置工具：

- shell：`bash`
- 文件：`read`, `edit`, `write`, `patch`
- 搜索：`glob`, `grep`
- agent：`actor`
- 网络：`fetch`, `search`
- 流程：`question`, `planenter`, `planexit`
- 状态：`memory`, `history`, `task`, `workflow`
- plugin/custom tools

模型适配集中在 `session/llm.ts`：

- 使用 Vercel AI SDK `streamText`。
- 支持 Anthropic、OpenAI、Google、Bedrock、Groq、Mistral、OpenRouter、xAI、DeepInfra、GitLab 等 provider。
- 支持 plugin hooks 改写 chat params、headers、system prompt。
- 对 LiteLLM/GitHub Copilot 类兼容场景，在历史有 tool calls 但本轮无 tools 时注入 `_noop` dummy tool，避免协议错误。
- 对 GitLab workflow model 有专门 tool executor / approval handler。
- 外层 retry 会发布 `Session.Event.RetryAttempt`。

## 6. 上下文、状态与记忆

MiMo Code 的上下文策略由多层组成：

| 层级 | 机制 |
|---|---|
| 短期上下文 | session messages / parts |
| 工具结果控制 | processor 统计 token、cost、files changed |
| Memory recall | SQLite FTS5，`memory_fts_idx`，BM25/snippet |
| Durable files | project/session/task/global `MEMORY.md` / `checkpoint.md` / notes |
| Checkpoint writer | hidden subagent `checkpoint-writer` |
| Rebuild context | checkpoint + memory + notes + tail preservation |
| Task progress | `tasks/<id>/progress.md` 等任务文件 |

`session/checkpoint.ts` 的 checkpoint writer 是关键设计。主 agent 不直接承担所有持久化记忆整理，而是让专门 hidden subagent 维护结构化 checkpoint，从而降低主 loop 上下文负担，并让 context rebuild 更可控。

## 7. 权限、沙箱与安全

MiMo Code 主要是权限控制，不是强容器沙箱。

重点机制：

- `permission/evaluate.ts` 将 ruleset flatten 后按 wildcard 匹配，默认 ask。
- 默认规则允许常规安全工具，对 `doom_loop` ask，对 external directory ask，对 `.env` / `.env.*` read ask，对 `.env.example` allow。
- `plan` agent 禁止 edit，仅允许写 plan 路径。
- `bash.ts`：
  - 默认 timeout 约 2 分钟。
  - 使用 tree-sitter 分析 bash / PowerShell 命令涉及路径。
  - 对 external directory 和 bash-sensitive 操作调用 `ctx.ask`。
  - 支持 abort/timeout，并返回 `<bash_metadata>`。
- `edit.ts`：
  - 调用 `assertWriteAllowed`。
  - 执行 edit permission 检查。
  - 记录 diff metadata。
  - 提供处理空白、缩进、escape、context anchor 的 fuzzy replacer。

安全风险：

- 默认仍可执行本机 shell，实际安全主要依赖 permission prompt/rules。
- plugin/custom tools 和 MCP servers 扩展面大，需要来源信任、审核和审计。
- `bypass` 或过宽 allow rule 会明显削弱安全边界。

## 8. 事件、日志与观测

- `session/processor.ts` 发布 `Metrics.ToolCall`、`Metrics.ModelCall` 与 step finish metrics。
- `session/session.ts` 将 session、messages、parts 写入 SQLite/SyncEvent。
- Bus / SyncEvent 负责跨模块事件同步。
- OpenTelemetry 用于模型、工具和性能观测。
- Snapshot/diff/revert 路径会发布 `Session.Event.Diff`。

整体模式是把模型调用、工具调用、消息状态与文件 diff 都作为一等事件观测，而不是散落的临时日志。

## 9. 测试与验证

仓库包含大量测试，尤其在 `packages/opencode/test/`：

- actor lifecycle、spawn、waiter、status
- agent registry / allowlist
- MCP lifecycle / OAuth / headers
- permission abort / non-interactive / disabled
- memory FTS / reconcile / paths
- plugin lifecycle / hooks
- provider conversion / error / model groups
- patch / revert / diff
- task / inbox / session / history
- TUI / plugin / UI 行为

CI workflow 包含：

- `.github/workflows/lint.yml`
- `.github/workflows/test.yml`
- `.github/workflows/typecheck.yml`

## 10. 核心路径

建议后续重点阅读：

- `packages/opencode/src/index.ts`
- `packages/opencode/src/agent/agent.ts`
- `packages/opencode/src/session/prompt.ts`
- `packages/opencode/src/session/processor.ts`
- `packages/opencode/src/session/llm.ts`
- `packages/opencode/src/session/checkpoint.ts`
- `packages/opencode/src/session/compaction.ts`
- `packages/opencode/src/session/max-mode.ts`
- `packages/opencode/src/session/goal.ts`
- `packages/opencode/src/tool/tool.ts`
- `packages/opencode/src/tool/registry.ts`
- `packages/opencode/src/tool/bash.ts`
- `packages/opencode/src/tool/edit.ts`
- `packages/opencode/src/tool/actor.ts`
- `packages/opencode/src/actor/spawn.ts`
- `packages/opencode/src/memory/service.ts`
- `packages/opencode/src/task/registry.ts`
- `packages/opencode/src/mcp/index.ts`
- `packages/opencode/src/permission/evaluate.ts`

## 11. 对 {{proj}} 的借鉴

1. 主 loop 只做调度，checkpoint writer 专门做持久上下文整理。
2. subagent/actor 应作为一级系统对象，而不是普通工具回调。
3. permission rule 应保留“默认 ask + 局部 allow/deny”的可解释链路。
4. 工具定义自带 schema、权限、执行、恢复、输出截断。
5. 上下文溢出优先走可重建 checkpoint，而不是每次全量 LLM summary。
6. task registry 与 actor 生命周期绑定，方便恢复、查询、取消。
7. 对模型协议兼容性做工程兜底，例如 `_noop` tool、retry stream、tool result 补齐。

## 12. 风险与反模式

- 项目在 2026 年 6 月中旬仍非常新，架构活跃但稳定性有待观察。
- Fork 自 OpenCode，模块规模大，学习成本高。
- 本地 shell/file edit 能力强，默认安全依赖权限与用户判断，不等同于沙箱。
- hidden agent、checkpoint、memory、task、plugin、MCP 交叉复杂，调试成本较高。
- 多 provider + plugin hooks 提升灵活性，也增加不可预测输入面。
- 不宜整体照搬架构；应只抽取适合 {{proj}} 边界的 checkpoint、permission、actor lifecycle 模式。
