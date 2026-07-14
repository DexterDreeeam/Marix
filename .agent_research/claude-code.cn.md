# Anthropic Claude Code 研究（官方证据边界）

> 研究日期：2026-07-14
> 关注点：Claude Code 消息编排中哪些可公开证实、哪些不可。
> 只读研究：未把任何源码写入 Marix，也未运行 Git。

> **证据边界。** Claude Code 完整 native agent loop、完整默认 system prompt、内部 provider adapter **未**开源。本文只以官方 Claude Code 文档、官方 Python Agent SDK 和公开 Messages API 为证据。它刻意**不**把本目录中第三方的 `claude-code-from-source` 笔记当作 Anthropic 私有实现的证据；那一对是独立的逆向参考，在此明确区分。

## 1. 来源与固定版本

| 项目 | 详情 |
|---|---|
| 产品文档 | [Claude Code docs](https://code.claude.com/docs/en/overview) |
| Agent SDK 文档 | [Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview) |
| API 文档 | [Messages API](https://platform.claude.com/docs/en/api/messages/create) |
| 公开 SDK 仓库 | [`anthropics/claude-agent-sdk-python`](https://github.com/anthropics/claude-agent-sdk-python) |
| 固定 SDK commit | [`059d3449bfc2e0dd64230bde65282df93dd21b8d`](https://github.com/anthropics/claude-agent-sdk-python/tree/059d3449bfc2e0dd64230bde65282df93dd21b8d) |
| 证据定位 | 固定 SDK 只作为 SDK→CLI 桥接与公开 message 类型的证据，不作为 native loop 的证据 |

## 2. 核心模块与执行流（可证实部分）

官方 Python Agent SDK 通过 `SubprocessCLITransport` 启动捆绑 CLI 的 `stream-json` 模式；公开 wrapper 不是 native loop。见 [`subprocess_cli.py#L111-L128`](https://github.com/anthropics/claude-agent-sdk-python/blob/059d3449bfc2e0dd64230bde65282df93dd21b8d/src/claude_agent_sdk/_internal/transport/subprocess_cli.py#L111-L128)、[`#L467-L543`](https://github.com/anthropics/claude-agent-sdk-python/blob/059d3449bfc2e0dd64230bde65282df93dd21b8d/src/claude_agent_sdk/_internal/transport/subprocess_cli.py#L467-L543)。

可证实的外层流程：SDK 启动 CLI，交换 `stream-json` 消息，并暴露 transcript/message 流。CLI 二进制内部每次请求的 payload 构造不公开。

## 3. System prompt 与重发行为

- CLI 默认使用 Claude Code coding-agent prompt，但完整文本未公开。
- Agent SDK 默认是更小的 prompt；选择 `claude_code` preset 可取得 CLI 风格 prompt；自定义 string 会替换默认 prompt；`append` 保留 preset 并追加内容。见 [modifying system prompts](https://code.claude.com/docs/en/agent-sdk/modifying-system-prompts)。
- Messages API 把 system prompt 放在顶层 `system` 字段。CLI 完整默认 prompt 文本仍不可证实。

## 4. 初始用户 task

公开证据：task 作为 `user` message 进入。直接 Messages API 无状态，每次调用需完整 history。Agent SDK/CLI session 可 continue/resume/fork，由 transcript 重建，不等于 provider 端 thread。见 [Agent SDK sessions](https://code.claude.com/docs/en/agent-sdk/sessions)。

## 5. Assistant 文本、reasoning 与工具调用

Assistant content 可含 `text`、`thinking`、`tool_use` block。见 [`types.py#L920-L1037`](https://github.com/anthropics/claude-agent-sdk-python/blob/059d3449bfc2e0dd64230bde65282df93dd21b8d/src/claude_agent_sdk/types.py#L920-L1037)。thinking block 协议公开，但 CLI 对 thinking 的 native 处理不公开。

## 6. Native 工具声明

工具使用是公开 API/SDK 能力：声明工具后模型返回 `tool_use` block。见 [handle tool calls](https://platform.claude.com/docs/en/agents-and-tools/tool-use/handle-tool-calls)。闭源 CLI 内部实际 tool registry 不公开。

## 7. 并行/串行工具策略

一个 assistant turn 可有多个 `tool_use` block。Messages API 本身不规定 host 是否真正并发；SDK 文档说明只读工具可并行，改变状态的工具通常顺序执行。任何具体内部调度阈值不可证实。

## 8. 工具结果与关联

client tool result 位于下一条 `user` message 的 `tool_result` block，`tool_use_id` 关联 assistant 的 `tool_use.id`。多个结果通常集中到该下一条 user message。此关联规则是公开 API 行为。

## 9. 下一次请求携带的 history

由于直接 Messages API 无状态，下一次请求重复完整上一轮：user task、完整 assistant blocks（text/thinking/`tool_use`）、以及聚合 `tool_result` block 的 user message。session continue/resume/fork 是 transcript 重建，不是隐藏的服务端 thread。

## 10. Context 压缩、截断、continuation 与缓存

Claude Code/SDK 接近上限会自动 compact，也可 `/compact`；公开证据能确认 compact boundary/summary 存在，但不能确认私有摘要 prompt 或算法。见 [agent loop](https://code.claude.com/docs/en/agent-sdk/agent-loop)。CLI 内部是否直接调用某个公开 beta compaction API 不可证实。

## 11. Subagent

subagent 是 fresh conversation，拥有独立 prompt/tools/model/context；中间历史不进入 parent，最终结果返回 parent，可并行。见 [subagents](https://code.claude.com/docs/en/agent-sdk/subagents)。内部路由阈值不公开。

## 12. Provider adapter

Agent SDK 可把 model 名称传给 Anthropic API、Bedrock、Vertex AI、Microsoft Foundry 或自定义 gateway，但 native provider 转换代码不公开。这**不能**证明任意非 Claude 模型兼容；Bedrock/Vertex/Foundry/gateway 支持仍是 Claude 部署适配。

## 13. 两轮模型序列（协议级，非复刻）

以下只是 Messages API/公开 SDK block 协议，不声称是 Claude Code 私有请求构造的逐字复刻：

```text
Request 1:
  system S
  tools [read, search]
  messages [user U1]

Assistant:
  [thinking/signature?, text?,
   tool_use(id=tu1),
   tool_use(id=tu2)]

Request 2:
  system S
  messages [
    user U1,
    assistant 完整 blocks,
    user [
      tool_result(tool_use_id=tu1),
      tool_result(tool_use_id=tu2)
    ]
  ]

Assistant:
  [final text]
```

## 14. 证据限制与 Marix 借鉴

**不应**写成事实的断言：

- 默认 system prompt 全文。
- native binary 如何精确裁剪、缓存和构造每个 HTTP payload。
- compactor 私有摘要 prompt/算法。
- subagent 自动路由阈值或隐藏 planner。
- Claude Code 是否内部直接使用某个公开 beta compaction API。
- model 名称 passthrough 代表任意非 Claude 模型可用。

Marix 借鉴：

1. 把公开 block 协议（system / user / assistant text-thinking-tool_use / user tool_result）当作稳定基线，超出部分明确标注为推测。
2. 优先按无状态 API 语义（全量重放）设计，再把 session continue/resume/fork 作为 transcript 重建，而非假设服务端 thread。
3. 严格区分官方证据笔记与逆向笔记；不要把 `claude-code-from-source` 断言并入官方基线。
4. 把 subagent 建模为只返回最终结果的 fresh conversation，符合文档化的隔离契约。
5. 记住到 Bedrock/Vertex/Foundry/gateway 的 provider passthrough 仍是 Claude 适配，不代表任意模型可移植。
