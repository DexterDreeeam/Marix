# OpenAI Codex CLI 研究

> 研究日期：2026-07-14
> 关注点：Codex CLI 如何在 OpenAI Responses wire 上表示模型 history、重发上下文并调度工具。
> 只读研究：未把 Codex 源码写入 Marix，也未运行 Git。

## 1. 来源与固定版本

| 项目 | 详情 |
|---|---|
| 仓库 | [`openai/codex`](https://github.com/openai/codex) |
| 固定 commit | [`393f64565ab46f09d99ca4d9bd973537e72a114b`](https://github.com/openai/codex/commit/393f64565ab46f09d99ca4d9bd973537e72a114b) |
| 主要语言 | Rust（`codex-rs` workspace） |
| 定位 | 直接构建在 OpenAI Responses API 之上的第一方 CLI coding agent |
| 关注理由 | Responses `ResponseItem` history、加密 reasoning 重放、HTTP 全量与 WebSocket continuation、多种 compaction 路径、有序并行工具、独立 child thread |

以下源码链接均使用 `/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/...#Lx-Ly` 永久链接。

## 2. 核心模块与执行流

Codex 需区分四种表示：

- `RolloutItem`：持久化 JSONL envelope，包含事件、metadata、compaction checkpoint 和 `ResponseItem`，不会整体发 API。
- `ResponseItem`：模型 history，形如 Responses wire 联合类型。
- `ResponseInputItem`：较窄的用户输入和 host tool output 输入类型。
- `TurnItem`：TUI/app-server 语义事件。

见 [`models.rs#L805-L1080`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/protocol/src/models.rs#L805-L1080)、[`#L1613-L1667`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/protocol/src/models.rs#L1613-L1667)。

每轮从 `clone_history().for_prompt()` 生成规范化完整 history，并构造 `Prompt { input, tools, parallel_tool_calls, base_instructions }`。reasoning、assistant message、function/custom call 和 outputs 都进入 history。见 [`turn.rs#L271-L295`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/session/turn.rs#L271-L295)、[`history.rs#L121-L144`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/context_manager/history.rs#L121-L144)。

## 3. System prompt 与重发行为

基础 system/model instructions 独立保存在 `BaseInstructions`，请求时放入 `instructions`；权限、developer instructions、skills、plugins、world state 可形成 developer/user input items。见 [`client_common.rs#L16-L36`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/client_common.rs#L16-L36)、[`session/mod.rs#L3252-L3474`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/session/mod.rs#L3252-L3474)。

base instructions 不是 `input` 中的 item，而是每次请求都携带在顶层 `instructions` 字段。

## 4. 初始用户 task

用户 task 转换成 `role:"user"` 的 `ResponseInputItem::Message`。developer context（权限、skills、world state）作为独立 developer/user input items 加入，而不是并入 task 文本。

## 5. Assistant 文本、reasoning 与工具调用

Responses 返回独立 items：`reasoning` item（可携带 encrypted content）、assistant `message` items、以及 `function_call` / custom-call items。每个 item 是离散的 history 单位；一个工具调用是一个 item，而不是内嵌 `tool_calls[]` 数组的 chat message。

## 6. Native 工具声明

工具是 Responses-native tool declaration，传入 `Prompt.tools`。请求还包含 `include:["reasoning.encrypted_content"]`、reasoning 配置、`parallel_tool_calls`、`prompt_cache_key`、store 和 stream。见 [`client.rs#L864-L907`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/client.rs#L864-L907)。

## 7. 并行/串行工具策略

各种 function/custom/tool-search call 统一成 `{tool_name, call_id, payload}`。模型可一次返回多 call；host 对可并行工具取共享读锁，对不可并行工具取独占写锁。每个 result 保持原 `call_id`，future 虽可并发完成，最终通过 `FuturesOrdered` 按 call 出现顺序写 history。见 [`router.rs#L113-L160`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/tools/router.rs#L113-L160)、[`parallel.rs#L94-L156`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/tools/parallel.rs#L94-L156)。

## 8. 工具结果与关联

每个 call 产生一个 `function_call_output` item，携带相同 `call_id`。由于结果经 `FuturesOrdered` 排序，持久化 item 顺序与模型原始 call 顺序一致，与完成时机无关。

## 9. 下一次请求携带的 history

普通 HTTP `ResponsesApiRequest` 没有 `previous_response_id` 字段，每轮发送完整 `input: Vec<ResponseItem>` —— reasoning、assistant message、call 与 output。见 [`common.rs#L215-L239`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/codex-api/src/common.rs#L215-L239)。

## 10. Context 压缩、截断、continuation 与缓存

Codex 有四种 compact 路径：token-budget、local summary、remote `/responses/compact` v1、remote compaction item v2。它们会替换或重建逻辑 history；这与 prompt cache 和 WS continuation cache 不同。入口见 [`turn.rs#L955-L1028`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/session/turn.rs#L955-L1028)。

只有 WebSocket DTO 才有 `previous_response_id`。仅当 model、instructions、tools、reasoning、store、cache key 等不变，且当前 input 严格以前次 request input + response output 为前缀时，才发送增量 input；否则回退全量。见 [`common.rs#L241-L293`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/codex-api/src/common.rs#L241-L293)、[`client.rs#L1164-L1253`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/client.rs#L1164-L1253)。因此 `previous_response_id` 是传输优化，不是唯一持久状态。

## 11. Subagent

`spawn_agent` 是普通 function tool。child 拥有独立 thread/session/history/model loop，默认新 history，也可 `fork_context=true` 复制过滤后的父 rollout；父子共享控制树与预算。见 [`multi_agents/spawn.rs#L88-L140`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/tools/handlers/multi_agents/spawn.rs#L88-L140)、[`agent/control/spawn.rs#L451-L620`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/agent/control/spawn.rs#L451-L620)。

## 12. Provider adapter

该 commit 只支持 Responses wire；provider adapter 主要处理 base URL、auth、headers、retry、HTTP/WS 和 Azure 差异，而不是 Anthropic/Gemini message 互转。见 [`model-provider-info/src/lib.rs#L55-L83`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/model-provider-info/src/lib.rs#L55-L83)。

## 13. 两轮模型序列

```text
HTTP Request 1:
  instructions S
  input [developer D, user U1]
  tools T
  parallel_tool_calls true

Response:
  R1=reasoning(encrypted_content)
  C1=function_call(call_id=c1)
  C2=function_call(call_id=c2)

Host:
  O1=function_call_output(call_id=c1)
  O2=function_call_output(call_id=c2)

HTTP Request 2:
  instructions S
  input [D,U1,R1,C1,C2,O1,O2]

WS cache-hit alternative:
  previous_response_id=resp1
  incremental input [O1,O2]
```

OpenAI 官方协议交叉证据：[conversation state](https://developers.openai.com/api/docs/guides/conversation-state)、[function calling](https://developers.openai.com/api/docs/guides/function-calling)、[WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)、[compaction](https://developers.openai.com/api/docs/guides/compaction)。

## 14. 证据限制与 Marix 借鉴

证据限制：本文只反映一个 commit；Codex 演进很快，行号范围、compaction 路径与多 agent 控制都可能变动。该 commit 仅面向 Responses wire，无法据此推断 Anthropic/Gemini 转换行为。

Marix 借鉴：

1. 把模型 history 视为有序 item 列表（reasoning / message / call / output），可干净映射到 Marix 规范化层；工具调用不必是 chat message。
2. 保留 `call_id`，即使乱序完成也要按模型 call 顺序稳定记录结果。
3. 把 `previous_response_id` 与连接 continuation 当作可失效的传输优化；始终保留完整本地重放路径。
4. 把 prompt cache、response continuation 与真正 compaction 区分为三种独立机制。
5. 加密 reasoning 重放说明隐藏 reasoning 必须作为 provider 范围内的 opaque 状态携带。
