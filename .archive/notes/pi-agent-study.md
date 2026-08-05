# Pi Agent 运行实测与固定源码研究

| 元数据 | 值 |
|---|---|
| 研究日期 | 2026-08-04 |
| 证据类型 | Ubuntu 运行实测、脱敏 HTTP/SSE wire、Session JSONL、固定 commit 源码确认 |
| 官方入口与迁移 | `https://github.com/badlogic/pi-mono.git`；当前 GitHub/API 与包元数据对应 `earendil-works/pi` |
| 固定 commit | `1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705` |
| 固定包版本 | `@earendil-works/pi-coding-agent@0.83.0`、`@earendil-works/pi-agent-core@0.83.0`、`@earendil-works/pi-ai@0.83.0` |
| 运行环境 | Ubuntu；Node.js `v22.23.2`；全部研究资产位于 `/root/pi-agent-study` |
| Provider / Model | OpenAI Chat Completions 兼容端点 `https://api.deepseek.com/chat/completions`；`deepseek-v4-flash` |
| 文档证据标记 | **【运行实测】**表示 wire、Session 或文件状态直接观察；**【源码确认】**表示固定 commit 代码确认；**【分析判断】**表示由两者推导 |
| 敏感信息策略 | API key 仅存在于进程环境；wire 中 Authorization 固定记录为 `<redacted-not-recorded>`；本文不包含任何 key、令牌或 Authorization 值 |
| 官方固定版本链接 | <https://github.com/earendil-works/pi/tree/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705> |

## 1. 目标、范围与结论

### 1.1 目标

本研究回答三个可复核问题：

1. Pi coding agent 在真实 DeepSeek 工具循环中，每一轮究竟向 provider 发送什么；
2. 运行时消息、内部 tool event、Session JSONL 与 provider wire 如何分层及重建；
3. Pi 的上下文压缩、扩展机制、缓存友好性与安全边界，对 Marix 有哪些可复用或不可照搬的部分。

### 1.2 范围

**【运行实测】**覆盖三个独立短会话和 Case A 的同会话 continuation：

- Case A：一次 `read` 后回答；
- Case B：`bash → read → write` 的依赖链；
- Case C：`read` 失败后 `bash → read` 恢复；
- Continuation：恢复 Case A Session 后，再次 `read` 并回答。

运行使用 `--print --approve --offline --no-extensions --no-skills --no-context-files`。因此，本文可以确认默认四工具、默认 system prompt、真实多轮 wire、失败传播和 Session 延续；不能把未运行的扩展、TUI、并行工具、压缩或分支行为冒充实测。

### 1.3 核心结论

| 结论 | 证据 |
|---|---|
| Pi 采用“持久 Session 记录 → agent-core 消息 → provider 投影”三层结构，而非把历史压成单个 Completed Calls 文本块 | 【运行实测】Session 与 wire；【源码确认】`agent-loop.ts`、`session-manager.ts`、`openai-completions.ts` |
| 每次 provider 请求都会重发完整四工具 schema；历史以尾部追加方式增长 | 【运行实测】所有 14 个请求 |
| OpenAI wire 严格保持 `assistant.tool_calls` 与后继 `tool.tool_call_id` 配对，并回放 `reasoning_content` | 【运行实测】Cases A/B/C |
| 内部 `toolResult` 保存 `toolName` 和 `isError`，但本兼容配置的 wire 只发送 `role/content/tool_call_id` | 【运行实测】Case C；【源码确认】投影条件 |
| 短会话没有触发 compaction；源码默认阈值是 `contextTokens > contextWindow - 16384`，并保留约 20,000 个近期 token | 【运行实测】Session 无 compaction entry；【源码确认】compaction |
| Plan、Todo、Subagent 是扩展示例，不是默认四工具核心；Branch Summary 是内置 Session/上下文机制，但不是默认任务规划器 | 【源码确认】examples 与 Session 类型 |
| Pi 的稳定前缀和追加尾部带来明显 provider cache 命中，但 provider 的缓存分块策略仍不可见 | 【运行实测】`cached_tokens` 序列；【分析判断】缓存原因 |
| Pi 默认工具直接继承本机进程、文件系统与网络权限；它不是权限或 sandbox 方案 | 【源码确认】默认 bash/read/write/edit；【运行实测】以 root workspace 执行 |

## 2. 身份迁移、版本与运行环境

### 2.1 身份

**【运行实测】**研究目录中的 evidence index 记录：克隆入口是 `badlogic/pi-mono`，但 GitHub API 和 package metadata 当前将其规范身份解析为 `earendil-works/pi`。本文统一称为 **Pi**，引用固定 commit，避免随仓库重命名或默认分支漂移。

**【源码确认】**Ubuntu 固定源码的三个 `package.json` 分别声明：

- `@earendil-works/pi-coding-agent`：`0.83.0`
- `@earendil-works/pi-agent-core`：`0.83.0`
- `@earendil-works/pi-ai`：`0.83.0`

### 2.2 Ubuntu 布局

| 路径 | 用途 |
|---|---|
| `/root/pi-agent-study/source` | 固定 commit 源码与已准备的运行产物 |
| `/root/pi-agent-study/runtime/node/bin/node` | Node.js `v22.23.2` |
| `/root/pi-agent-study/workspaces/case-{a,b,c}` | 隔离的案例工作目录 |
| `/root/pi-agent-study/traces/case-{a,b,c}` | manifest、stdout/stderr、wire、Session |
| `/root/pi-agent-study/analysis` | evidence index、运行汇总、观察到的 prompt/tools |

本文研究阶段只读取这些资产，没有重新 build、test 或 deploy。

### 2.3 Provider 兼容配置

**【运行实测】**最终有效配置的关键性质：

- API：OpenAI Chat Completions；
- provider 名：`deepseek-study`；
- model：`deepseek-v4-flash`；
- endpoint：`https://api.deepseek.com/chat/completions`；
- thinking 格式：DeepSeek；
- 不使用 developer role；
- 不发送 `reasoning_effort`；
- API key 只从进程环境读取，不进入 manifest、wire 或本文。

最终 CLI 形态为：

```text
pi --provider deepseek-study --model deepseek-v4-flash
   --session-dir <case-session-dir> --name <case>
   --approve --offline --no-extensions --no-skills --no-context-files
   --print <task>
```

`--offline` 不能解释为“禁止 provider 网络”：真实请求仍发送至 DeepSeek。它只属于本次 CLI 资源加载配置，不能替代网络隔离。

### 2.4 stdin 的实测陷阱

**【运行实测】**最初两次 CLI 尝试在产生任何 model request 之前超时。原因是 SSH 子进程继承了未结束的 stdin，Pi 把非 TTY stdin 当作管道输入，等待 EOF。将子进程 stdin 设为 `DEVNULL` 后正常启动。

**【源码确认】**`packages/coding-agent/src/main.ts:64-84,827-842`：

- 非 TTY 时持续读取 `data`；
- 只有收到 `end` 才 resolve；
- 非 RPC 模式在准备 initial message 之前等待该读取；
- 若交互模式收到管道输入，还会切换到 print 模式。

因此，自动化宿主必须显式关闭 stdin，或提供有限输入并结束；“没有要输入的内容”不等于“继承一个永不 EOF 的管道”。

## 3. 网络捕获、SSE、脱敏与真实性边界

### 3.1 捕获记录

**【运行实测】**每个 `wire.jsonl` 以一行一个 JSON 对象保存：

- request：`timestamp`、`requestId`、`kind`、`method`、`localPath`、`upstreamEndpoint`、`body`、脱敏后的 `authorization`；
- response：`timestamp`、`requestId`、`kind`、HTTP `status`、`contentType`、原始 `bodyText`、解析后的 `sseEvents`。

请求进入本地兼容路径 `/v1/chat/completions`，捕获器转发到真实 DeepSeek endpoint。四个 wire 文件中所有 response 都是 HTTP 200，内容类型均为 `text/event-stream; charset=utf-8`，最后事件均为 `[DONE]`。

### 3.2 SSE 观察

| 文件 | request | SSE 事件数 | `cached_tokens` |
|---|---:|---:|---:|
| Case A `wire.jsonl` | req-001 / req-002 | 71 / 39 | 0 / 1664 |
| Case A `wire-continuation.jsonl` | req-001 / req-002 | 14 / 11 | 1664 / 1792 |
| Case B `wire.jsonl` | req-001…004 | 99 / 29 / 58 / 93 | 0 / 1664 / 1792 / 1920 |
| Case C `wire.jsonl` | req-001…004 | 87 / 41 / 15 / 19 | 640 / 1664 / 1792 / 1920 |

SSE delta 中可出现 `reasoning_content`、文本片段、分片的 `tool_calls[].function.arguments`，终止块给出 `finish_reason`，末尾 usage 块给出 prompt/completion/cache token。Pi 先组装流式片段，再把完成的 assistant 消息和工具调用写入内部状态。

### 3.3 脱敏

**【运行实测】**

- 所有 request 的 `authorization` 都是字面值 `<redacted-not-recorded>`；
- manifest 的 `apiKey` 是 `<redacted-not-persisted>`；
- analysis evidence index 明确说明 key 只存在于进程环境；
- 本文没有记录环境值、header 原值或任何可恢复 secret。

### 3.4 “真实请求”的证据强度

这些记录证明：本地捕获器观察到发往指定 upstream 的完整 request body、HTTP 200 SSE body、DeepSeek 风格 usage/cache 字段，并且工具结果改变了下一轮 body。它比只看 CLI stdout 更强。

但它不是第三方签名或 packet-level 独立取证：捕获器本身仍属于研究环境。因此，本文把“真实”限定为“真实运行进程产生且由本地 wire recorder 捕获的 provider HTTP 往返”，不声称具备密码学不可否认性。

## 4. Cases A/B/C 与同会话 continuation

### 4.1 Case A：单工具往返

**任务：**必须先且只先读取 `note.txt`，再根据内容给出一句中文答案。

**【运行实测】时序：**

```text
user
  → provider req-001
  ← assistant tool_call(read, note.txt)
  → host read
  → provider req-002（回放 assistant tool_call + tool result）
  ← assistant final
```

- 文件初始与结束 SHA-256 均为 `e328fc3b49c151f9f3ec90db9d6dff25fbef73746484eb2c9bdd732083fc52ac`；
- 无文件变化；
- 最终回答正确包含 `PI_A_OK_7429`。

### 4.2 Case B：依赖链

**任务：**依次执行 `bash ls -1 data`、读取唯一 Markdown、写出 `result.txt`。

**【运行实测】调用链：**

```text
req-001 → bash("ls -1 data")
req-002 → read("data/brief.md")
req-003 → write("result.txt", "标题=Nebula Delta")
req-004 → final answer
```

- 输入 Markdown 第一行为 `# Nebula Delta`；
- `result.txt` 是唯一新增文件，SHA-256 为 `b8ffd88e475d72bc2eab02b8ecb2aca77cb78f2b97b36776687a7e63ddb2420a`；
- 原输入文件 SHA-256 前后不变；
- 每个工具调用都形成独立 provider 往返，没有把三步合成一个 shell command。

### 4.3 Case C：失败后恢复

**任务：**先读取不存在的 `missing.txt`，失败后列目录，再读取 `fallback.txt`。

**【运行实测】调用链：**

```text
req-001 → read("missing.txt") → ENOENT，内部 isError=true
req-002 → bash("ls -1")       → fallback.txt
req-003 → read("fallback.txt")→ RECOVERED_C_OK_3816
req-004 → final answer
```

失败没有终止 agent loop。值得注意的是，provider wire 中失败仍只是一个 `role:"tool"` 的文本结果；`isError=true` 只存在于 host event 和 Session JSONL。

### 4.4 Case A continuation

**【运行实测】**初始 Session 为 8 行；恢复同一 Session 并完成第二用户轮后为 12 行。continuation 首次请求不是新会话上下文，而是：

```text
system
→ 原 user
→ 原 assistant(tool_call)
→ 原 tool
→ 原 final assistant
→ 新 user
```

模型再次调用 `read`，下一轮再追加新 `assistant(tool_call) → tool`，最终回答 `复核=PI_A_OK_7429`。这确认了“前一轮 final assistant 也保留并回放”，而不是只保留工具事实。

## 5. Provider wire 的精确消息编排

### 5.1 消息对象字段

以下为实测结构的等价精简示例；内容被缩短，但字段、角色和嵌套关系保持一致：

```json
[
  {
    "role": "system",
    "content": "<system prompt string>"
  },
  {
    "role": "user",
    "content": [
      { "type": "text", "text": "<user task>" }
    ]
  },
  {
    "role": "assistant",
    "content": null,
    "reasoning_content": "<thinking text>",
    "tool_calls": [
      {
        "id": "<call id>",
        "type": "function",
        "function": {
          "name": "read",
          "arguments": "{\"path\":\"note.txt\"}"
        }
      }
    ]
  },
  {
    "role": "tool",
    "content": "<plain text result>",
    "tool_call_id": "<same call id>"
  }
]
```

**【运行实测】关键点：**

1. user 文本是 content-part 数组，不是普通字符串；
2. assistant 工具轮的 `content` 为 `null`；
3. `reasoning_content` 随 assistant 工具调用一起回放；
4. `function.arguments` 在 wire 中是 JSON 字符串；
5. tool 结果使用完全相同的 `tool_call_id`；
6. 当前 DeepSeek 兼容配置不发送 tool `name`；
7. wire 不发送内部 `isError`；
8. 下一轮重发完整历史，不只发送增量。

### 5.2 每轮顶层 request body

所有实测请求的顶层字段集合完全相同：

| 字段 | 实测值或含义 |
|---|---|
| `model` | `"deepseek-v4-flash"` |
| `messages` | 当前完整投影上下文 |
| `stream` | `true` |
| `stream_options` | `{"include_usage":true}` |
| `store` | `false` |
| `max_completion_tokens` | `8192` |
| `tools` | 完整四工具 schema |
| `thinking` | `{"type":"enabled"}` |

以下字段在全部实测请求中均**不存在**：

- `tool_choice`
- `reasoning_effort`
- `prompt_cache_key`
- `developer` 顶层字段（developer 是 message role，也未使用）

**【源码确认】**`buildParams` 仅在 host options 指定 `toolChoice` 时加入 `tool_choice`；DeepSeek thinking 格式在 reasoning 开启时加入 `thinking.type=enabled`，只有兼容配置声明支持时才发送 `reasoning_effort`。

## 6. 默认四工具 schema、重发与 SHA-256

### 6.1 模型面对的 schema

| 工具 | required | optional / 子字段 | `strict` | 运行语义摘要 |
|---|---|---|---:|---|
| `read` | `path:string` | `offset:number`（1-based）、`limit:number` | `false` | 文本默认最多 2000 行或 50KB，支持继续读取；也支持图片 |
| `bash` | `command:string` | `timeout:number`（秒） | `false` | 合并返回 stdout/stderr；默认保留最后 2000 行或 50KB，截断时保存完整临时输出 |
| `edit` | `path:string`、`edits:array` | 每个 edit 要求 `oldText:string`、`newText:string` | `false` | 所有匹配基于原文件；要求唯一且不重叠 |
| `write` | `path:string`、`content:string` | 无 | `false` | 创建或覆盖文件，并自动建父目录 |

一个值得保留的细节：`edit` 的 host 校验要求 `edits` 至少一项，但实测模型 schema 没有 `minItems`。这体现了模型输入 schema 与 host 运行时校验并不总是完全等价。

### 6.2 每轮重发

**【运行实测】**Cases A/B/C 与 continuation 共 14 个 request，每个都包含 `read,bash,edit,write` 四个完整 definition。Pi 没有在第二轮以后只发工具名，也没有依赖 provider 保存上轮 schema。

### 6.3 Hash 的可复核定义

JSON hash 依赖序列化方式，必须同时记录算法：

| 对象 | 序列化 | SHA-256 |
|---|---|---|
| 每轮 `tools` 结构 | Python `json.dumps(tools, sort_keys=True)` 默认分隔符；analysis 使用的结构 hash | `c760175ee743551974d44df87a01e02c7aeaf7ded05bf10449b1bca3c1d48012` |
| 每轮 `tools` 结构 | 紧凑 `JSON.stringify(tools)` | `2f8cd9c986260c3ca9b6022bf63651063f546ccb746eaf04f83c180b00e69b17` |
| `analysis/observed-tools.json` 文件字节 | 文件 SHA-256 | `f6369c2388a1cc7a44bad39003e9a92ccdc93adba012956122e6330ab7d34f79` |

三者不同不是 schema 漂移，而是空格、缩进和 key 顺序不同。跨轮稳定性应使用第一种明确算法比较；所有 request 均为同一个 `c760…`。

## 7. System prompt：构成、长度与动态性

### 7.1 实测构成

**【运行实测】**Case A 的完整 prompt 位于 `analysis/observed-system-prompt.txt`：

- 长度：2560 个字符；
- 文件 SHA-256：`e1685370edb7beca14fb3b74caf4672c6983d75fd064c4dfbefd37ab8dc6c6c2`；
- 内容区段：
  1. coding assistant 身份；
  2. Available tools 的四个一行摘要；
  3. 文件探索、read/edit/write 等 guidelines；
  4. Pi 文档、docs、examples 的绝对路径与按主题读取提示；
  5. 最后一行 `Current working directory: ...`。

本文不复制完整第三方 prompt；长期复核应读取上述 evidence 文件。

### 7.2 稳定与动态部分

**【运行实测】**

- 同一 case 的每轮 system prompt 字节稳定；
- 不同 case 的主体相同，最后 CWD 分别指向 `case-a`、`case-b`、`case-c`；
- message history 只在尾部增加；
- 本次关闭 skills、extensions、context files，所以观察不到这些动态段。

**【源码确认】**`buildSystemPrompt` 还支持：

- `customPrompt`：替换默认主体；
- `appendSystemPrompt`：追加文本；
- `contextFiles`：包装进 project context / project instructions 区段；
- `skills`：仅在 `read` 可用时加入 skills 文本；
- 工具 snippets 和 tool-specific guidelines；
- 无论默认或 custom prompt，最后追加当前 CWD。

因此，不能把 2560 字符当作所有 Pi 启动的固定常量；它只对本次“默认四工具 + 禁用扩展/skills/context + 特定 CWD”成立。

### 7.3 system 与 developer

**【运行实测】**第一条消息角色始终是 `system`，没有 `developer`。

**【源码确认】**provider 投影仅在 `model.reasoning && compat.supportsDeveloperRole` 时把 system prompt 改投为 `developer`。本次 compatibility profile 不支持该角色，所以保持 `system`。

## 8. Agent loop 与内部 event / provider wire 的差异

### 8.1 执行流

**【源码确认】**核心数据流：

```text
AgentMessage[]
  → 可选 transformContext
  → convertToLlm
  → Context{systemPrompt,messages,tools}
  → provider stream
  → assistant partial/final events
  → 提取 toolCall
  → host 执行工具
  → ToolResultMessage
  → 追加状态
  → 下一轮完整投影
```

agent loop 同时支持 steering/follow-up queue。工具调用可按配置或工具属性顺序执行，也可并行执行；即使并行完成，结果仍按原 tool-call 顺序组装。

### 8.2 内部 `toolResult`

内部记录包含：

```json
{
  "role": "toolResult",
  "toolCallId": "<id>",
  "toolName": "read",
  "content": [{ "type": "text", "text": "<result>" }],
  "details": {},
  "isError": true,
  "timestamp": 0
}
```

此外，`tool_execution_end` event 也带 `toolCallId`、`toolName`、`result` 和 `isError`。这让 UI、Session 和扩展能够区分成功/失败，而不必解析错误字符串。

### 8.3 provider 投影

OpenAI-compatible wire 只保留：

```json
{
  "role": "tool",
  "content": "<flattened text>",
  "tool_call_id": "<id>"
}
```

只有 `compat.requiresToolResultName` 为真时才加入 `name`；本次为假。`isError` 没有 OpenAI Chat Completions 对应字段，因此完全不进入 wire。

**Case C 的直接证据：**

- Session：`role=toolResult, toolName=read, isError=true`；
- wire：`role=tool`，content 是 ENOENT 文本，只有 `tool_call_id`；
- 模型仍从自然语言错误内容判断失败并恢复。

这说明 host-facing output schema 可以改善本地状态和审计，但不会自动改善模型看到的错误类型。

## 9. Session JSONL、tree、parentId 与 continuation

### 9.1 记录类型

**【运行实测】**一个 Session 文件先有 header，随后是 append-only entry。初始常见序列：

```text
session header
session_info
model_change
thinking_level_change
message(user)
message(assistant)
message(toolResult)
message(assistant)
...
```

**【源码确认】**entry 类型还包括：

- `compaction`
- `branch_summary`
- `custom`
- `custom_message`
- `label`

除 Session header 外，entry 基类有 `id`、`parentId`、`timestamp`。`custom` 不进入 LLM context；`custom_message` 会进入。

### 9.2 tree 与 active branch

Session 不是单纯数组语义。`parentId` 形成树：

1. 选择当前 leaf；
2. 通过 `parentId` 向 root 回溯；
3. reverse 得到 root-to-leaf active path；
4. 从 active path 推导当前 model/thinking setting；
5. 应用最新 compaction boundary；
6. 把可见 entry 转为 AgentMessage，再投影给 provider。

切换 leaf 即可回到旧节点并产生分支，不需要复制整个 transcript。

### 9.3 continuation 的直接证据

Case A：

- initial snapshot：8 行；
- continuation 后：12 行；
- 新四项依次为 `user → assistant(toolCall) → toolResult → assistant(text)`；
- 每项 `parentId` 指向前一 active entry；
- 新 provider 请求包含原 final assistant。

这证明 Session continuation 是在同一树叶追加，而非把用户第二轮伪装成新 Session 的首轮。

### 9.4 context rebuild 与 compaction boundary

**【源码确认】**

- `message` 直接还原；
- `custom_message` 转为 custom user message；
- `branch_summary` 与 `compaction` 转为带标签的 user message；
- 若 active path 没有 compaction，全部 active path 可见消息进入 context；
- 若有多个 compaction，取最新一个，以其 `firstKeptEntryId` 为保留边界；
- context 由“最新 compaction summary + 保留消息 + compaction 后消息”组成，更旧被总结的 entry 仍留在 JSONL，但不再投影。

## 10. Context 与 compaction

### 10.1 实测与源码必须分开

**【运行实测】**Cases A/B/C 和 continuation 都很短，最大 request prompt 约 2006 token，Session 中没有 `compaction` entry，也没有 compaction SSE 请求。因此：

- 本研究没有实测自动 compaction；
- 没有实测 split turn；
- 没有实测 context-overflow retry；
- 下面的算法结论均为固定源码确认，不应标记为运行成功。

### 10.2 触发公式与默认值

**【源码确认】**

```text
shouldCompact =
  settings.enabled
  && contextTokens > contextWindow - reserveTokens
```

默认：

| 设置 | 值 |
|---|---:|
| `enabled` | `true` |
| `reserveTokens` | `16384` |
| `keepRecentTokens` | `20000` |

context token 优先取最近有效 assistant usage 的 `totalTokens`；其后若还有消息，再估算 trailing tokens。错误、abort、全零 usage 不直接作为有效基准。

### 10.3 切分与 CompactionEntry

Pi 从最新消息反向累计估算 token，达到 `keepRecentTokens` 后选择合法 cut point。正常优先保留完整近期 turn；单个 turn 过大时允许 split turn，并单独总结其前缀。

`CompactionEntry` 的核心字段：

| 字段 | 含义 |
|---|---|
| `summary` | 旧历史的结构化总结 |
| `firstKeptEntryId` | 未被总结、继续保留的第一项 |
| `tokensBefore` | 压缩前重建 context 的 token 数 |
| `details` | 默认保存累计 read/modified file 列表，也可由扩展定义 |
| `usage` | 生成 summary 的 LLM usage |
| `fromHook` | 是否由扩展 hook 提供 |

entry 是追加而非覆盖。旧消息继续可审计，provider context 则按最新 boundary 重建。

### 10.4 summary 的 provider 投影

**【源码确认】**compaction summary 不是替换 system prompt，而是投影成一个 `role:"user"` 消息，文本由固定前后标签包围。branch summary 同样投影为 user message，但使用不同标签。

这保留了 system prompt 的稳定身份，同时把“压缩历史”明确放入对话层。

### 10.5 工具结果 2000 字符截断

**【源码确认】**`serializeConversation()` 为“生成 summary 的请求”序列化历史时，每个 tool result 最多保留**前 2000 字符**，并附加省略字符数。

必须区分：

- 这不是正常 agent loop wire 的统一 2000 字符上限；
- read/bash 自身另有 2000 行/50KB 截断；
- 2000 字符规则只发生在 compaction/branch summary 的文本序列化层；
- 它只保留头部，可能隐藏决定性的尾部错误或统计结果。

### 10.6 overflow recovery

**【源码确认】**`AgentSession._checkCompaction` 有两类路径：

1. **阈值路径**：达到阈值后压缩，但已有成功回答时不自动重跑；
2. **可恢复失败路径**：context overflow 或可恢复的 length stop 时，移除 agent state 尾部失败/truncated assistant，执行 compaction，并最多自动 retry 一次。

失败 assistant 仍保留在 Session 历史中，但从 retry context 排除。`_overflowRecoveryAttempted` 防止无限“压缩—重试”循环；第二次失败会发出明确错误，不再重试。

## 11. Plan、Todo、Subagent 与 Branch Summary 的定位

| 能力 | 定位 | 本次运行 |
|---|---|---|
| Plan Mode | `examples/extensions/plan-mode` 扩展示例；可切换 active tools、注入上下文、拦截 destructive bash | `--no-extensions`，未加载 |
| Todo | `examples/extensions/todo.ts` 注册自定义 `todo` tool 和 `/todos` command | 未加载 |
| Subagent | `examples/extensions/subagent` 注册自定义 subagent tool，并提供 planner/reviewer/scout/worker 示例 | 未加载 |
| Branch Summary | coding-agent 内置 Session/上下文机制，在 `/tree` 导航离开分支时可总结 | 未触发 |

因此，准确说法不是“Pi 完全没有 branch summary”，而是：

- 默认工具循环不要求 Plan/Todo/Subagent；
- Branch Summary 属于 Session 导航与上下文保存，不是一个默认的任务分解或完成判定协议；
- 本次 wire 只有四个默认工具，没有 plan/todo/subagent tool。

## 12. Cache 行为：稳定前缀、schema、尾部与 CWD

### 12.1 `cached_tokens` 序列

```text
Case A:              0 → 1664
Case A continuation: 1664 → 1792
Case B:              0 → 1664 → 1792 → 1920
Case C:              640 → 1664 → 1792 → 1920
```

Pi 没有发送 `prompt_cache_key`。命中由 provider 自行识别。

### 12.2 可确认的稳定性

**【运行实测】**

- 同一 run 的 system prompt 不变；
- 四工具 schema 每轮结构 hash 不变；
- 旧消息顺序与内容不变；
- 新 assistant/tool 消息只追加到尾部；
- continuation 继续使用旧 Session 历史；
- CWD 位于 system prompt 最后一行，是 workspace 级动态内容。

### 12.3 谨慎解释

**【分析判断】**高 cache 命中与“稳定 system 前缀 + 稳定工具 schema + 追加历史”一致。Case C 首轮已有 640 cached tokens，也与跨 case 共用 prompt 前缀一致，即使最后 CWD 不同。

但 wire 不暴露 provider 的分块边界、TTL 或 cache key 算法。不能由 `640/1664/1792/1920` 反推出 DeepSeek 的确切缓存实现，也不能证明 tools schema 一定属于同一个缓存块。

## 13. 固定源码证据表

所有行号均针对 commit `1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705`，并已从 Ubuntu `/root/pi-agent-study/source` 读取。

| 文件与固定链接 | 准确行区间 | 确认内容 |
|---|---:|---|
| [`packages/agent/src/agent-loop.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/agent/src/agent-loop.ts#L155-L275) | 155-275 | 主循环、assistant → tool results → 下一轮、steering/follow-up |
| 同上 | 281-371 | context transform、LLM 投影、stream events、final assistant 追加 |
| 同上 | 411-553 | 顺序/并行 tool execution、结果按 call 顺序组装 |
| 同上 | 709-791 | afterToolCall、错误归一化、`tool_execution_end`、内部 ToolResultMessage |
| [`packages/ai/src/api/openai-completions.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/ai/src/api/openai-completions.ts#L676-L860) | 676-860 | request 顶层字段、tools、tool choice、DeepSeek thinking 兼容参数 |
| 同上 | 1020-1244 | system/developer 选择、assistant/tool call、reasoning 回放、tool result 投影 |
| [`packages/coding-agent/src/core/system-prompt.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/coding-agent/src/core/system-prompt.ts#L28-L161) | 28-161 | 默认/custom prompt、工具摘要、guidelines、context、skills、CWD |
| [`packages/coding-agent/src/core/session-manager.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/coding-agent/src/core/session-manager.ts#L46-L153) | 46-153 | Session entry、CompactionEntry、BranchSummaryEntry、custom 类型 |
| 同上 | 334-360 | 由 leaf 沿 `parentId` 重建 active path |
| 同上 | 383-407 | entry → context message |
| 同上 | 418-469 | 最新 compaction boundary 与完整 context rebuild |
| [`packages/coding-agent/src/core/messages.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/coding-agent/src/core/messages.ts#L11-L187) | 11-24、100-187 | compaction/branch 标签与 user-role 投影 |
| [`packages/coding-agent/src/core/compaction/compaction.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/coding-agent/src/core/compaction/compaction.ts#L126-L237) | 126-237 | 默认设置、usage/estimate、触发公式 |
| 同上 | 403-448 | 反向累计与 cut point |
| 同上 | 692-918 | preparation、旧 summary、保留边界、summary 生成与 CompactionResult |
| [`packages/coding-agent/src/core/compaction/utils.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/coding-agent/src/core/compaction/utils.ts#L88-L149) | 88-149 | summary 序列化与 tool result 2000 字符截断 |
| [`packages/coding-agent/src/core/agent-session.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/coding-agent/src/core/agent-session.ts#L1938-L2046) | 1938-2046 | threshold/overflow 两路径、最多一次 compact-and-retry |
| [`packages/coding-agent/src/main.ts`](https://github.com/earendil-works/pi/blob/1d0c97471359a7c1dc6bfc9ac7ce5b4aa9afd705/packages/coding-agent/src/main.ts#L64-L84) | 64-84、827-842 | 非 TTY stdin 等待 EOF 与 initial message 顺序 |
| `packages/coding-agent/src/core/tools/read.ts` | 20-24、212-215 | read 模型 schema 与描述 |
| `packages/coding-agent/src/core/tools/bash.ts` | 40-43、327-332 | bash 模型 schema 与描述 |
| `packages/coding-agent/src/core/tools/edit.ts` | 33-53、295-305 | edit 模型 schema 与描述 |
| `packages/coding-agent/src/core/tools/write.ts` | 14-17、189-193 | write 模型 schema 与描述 |

Ubuntu 固定源码文件 SHA-256：

| 文件 | SHA-256 |
|---|---|
| `agent-loop.ts` | `3f2bef7cd470395d62d869eba2b8d6ade47d4643db71b67f7a25dd2686cc462c` |
| `openai-completions.ts` | `34dca8872d8c5d3646cf61232eadef2dea9b639d3919824e675c9816e4498c49` |
| `system-prompt.ts` | `677c3cf2ca259d15c27466961702a489244e9505829a6c994d0de314b2a469ef` |
| `session-manager.ts` | `9f00ce7d51fa8bb7e3dbc82a83bd6b28c5922e474a5cd86282a746bf24116fdb` |
| `messages.ts` | `8229aedb49e0cf12ebb9ed72670e24770c23e4d3ea814fe6f4e91cabe0288745` |
| `compaction/compaction.ts` | `0ce643e2e3c97e4dc160e888e93cb8b5b53914e609a6d333765493d630b6b731` |
| `compaction/utils.ts` | `6e9d2c0b6076d5cac5fd444e5381a1fc01da5acf30eb776319a6b562fc42a02c` |
| `agent-session.ts` | `59b428b8bcc44a9c1ff0285b3c028428783ccf3a2747366ea67fa4b8d6ebb17c` |

## 14. 运行证据索引与 req-001 映射

### 14.1 证据索引

| ID | Ubuntu 路径 | 内容 |
|---|---|---|
| R1 | `/root/pi-agent-study/analysis/evidence-index.md` | 身份、版本、运行资产与源码行号总索引 |
| R2 | `/root/pi-agent-study/analysis/runtime-summary.json` | Cases A/B/C 的 manifest、逐请求 messages、response、usage |
| R3 | `/root/pi-agent-study/analysis/observed-system-prompt.txt` | Case A 观察到的完整 2560 字符 prompt |
| R4 | `/root/pi-agent-study/analysis/observed-tools.json` | 四工具完整模型 schema |
| R5 | `/root/pi-agent-study/traces/case-a/wire.jsonl` | Case A 两次 HTTP/SSE 往返 |
| R6 | `/root/pi-agent-study/traces/case-a/wire-continuation.jsonl` | 同 Session 第二用户轮的两次往返 |
| R7 | `/root/pi-agent-study/traces/case-b/wire.jsonl` | Case B 四次往返 |
| R8 | `/root/pi-agent-study/traces/case-c/wire.jsonl` | Case C 四次往返 |
| R9 | `/root/pi-agent-study/traces/case-*/sessions/*.jsonl` | tree-shaped Session entries |
| R10 | `/root/pi-agent-study/traces/case-*/manifest.json` | task、command、前后文件 SHA、changed 集合 |

### 14.2 `req-001` 映射

request ID 在不同 wire 文件内独立编号，不能跨文件当作全局 ID。

| 文件 | `req-001` 输入角色 | `req-001` 结果 |
|---|---|---|
| Case A `wire.jsonl` | `system → user` | assistant 调用 `read(note.txt)` |
| Case A `wire-continuation.jsonl` | `system → 原user → 原assistant → 原tool → 原final assistant → 新user` | assistant 再次调用 `read(note.txt)` |
| Case B `wire.jsonl` | `system → user` | assistant 调用 `bash("ls -1 data")` |
| Case C `wire.jsonl` | `system → user` | assistant 调用 `read(missing.txt)` |

复核时应使用 `(wire 文件路径, requestId)` 作为复合键。

## 15. 与 Marix 当前架构的差异

### 15.1 结构对比

| 维度 | Pi 0.83.0 | Marix 当前 |
|---|---|---|
| 任务组织 | 默认是连续消息/tool loop；无强制层级任务协议 | 层级 `Intent` / subintent；当前项标记 `EXECUTING NOW` |
| 已完成工作 | 回放原始 ordered assistant/tool pairs | `append_tool_calls` 合成 `[COMPLETED CALLS]`，工具输出压成单行 |
| 路由 | 模型可在普通工具与停止之间逐轮决定 | System + WorkflowPolicy + 当前任务尾部 gate，要求恰好一个 native function |
| 动态尾部检查 | 新 user/工具结果直接作为 transcript 尾部 | `CurrentTaskContextHeader` 在 Current Task 前执行最终 Plan/普通工具/Complete gate |
| 调用关联 | 稳定 `tool_call_id` 原样配对 | Completed Calls 是宿主合成的描述，不保留 provider-native assistant/tool 结构 |
| 状态 | append-only Session tree，可按 leaf/compaction 重建 | Intent context、step results、subintents 分层组织 |
| 压缩 | 显式 CompactionEntry + 保留边界 | 当前比较重点是层级 Intent 与合成执行尾部，不等价于 Pi 的 transcript compaction |

### 15.2 Pi 的优势

1. 原始调用顺序、call ID、reasoning 与 tool result 不丢失；
2. Session 持久层和 provider 投影层分离；
3. branch/continuation 不必重写历史；
4. tool schema 与 prompt 稳定，天然利于 provider prefix cache；
5. compaction 是显式可审计 entry，而不是静默删消息。

### 15.3 Pi 不替代 Marix 的部分

Pi 默认不提供 Marix 的：

- 层级 Intent 隔离；
- 明确的 Plan / Complete / Infeasible workflow contract；
- “当前任务”与“父任务已完成事实”的确定性边界；
- 一次只选择一个 native function 的路由约束；
- 宿主级权限、部署和远程执行策略。

直接把 Pi 的 transcript loop 当作 Marix workflow engine，会失去 Marix 当前最重要的任务边界。

## 16. 对 Marix 的建议与安全边界

### 16.1 建议采用

1. **持久记录与 provider 投影分层**  
   保存原始 assistant/tool pairs、call ID、错误位和 usage；按当前 Intent/branch 动态投影，而不是只保留 Completed Calls 文本。

2. **保留 ordered native tool history**  
   Completed Calls 可继续作为路由摘要，但不应成为唯一事实源。原始结构用于恢复、审计、provider continuation 和错误分析。

3. **稳定 schema 与 prompt 前缀**  
   工具 schema 按 canonical hash 管理；非必要不要逐轮重排描述。将动态检查尽量集中在短尾部，减少破坏缓存的变化。

4. **显式压缩边界**  
   借鉴 `CompactionEntry{summary,firstKeptEntryId,tokensBefore}`，使“保存了什么、隐藏了什么、从何处继续”可审计。

5. **内部错误类型不要降级**  
   provider wire 可能只能传文本，但 Marix 内部必须保留 `isError`、error kind、tool name、call ID；完成判定不应仅解析文本。

6. **overflow 最多一次恢复**  
   采用 typed context-overflow 条件、压缩后一次 retry 和显式终止，避免无限循环。

7. **混合而非替换**  
   推荐形态是“Marix Intent 层级 + 原始调用事件日志 + 可选 Completed Calls 路由摘要 + 短动态尾部 gate”，不是删除 Intent 后完全照搬 Pi。

### 16.2 不应照搬

1. compaction summary 对 tool result 只保留前 2000 字符；Marix 更适合 head+tail 或按工具类型预算；
2. tool failure 在模型 wire 中只有自然语言；Marix 若 provider/schema允许，应强化结构化错误提示；
3. Plan/Todo/Subagent 仅靠可选扩展；Marix 的 workflow contract 不应降级为示例扩展；
4. 默认 bash/read/write/edit 的本机权限模型。

### 16.3 Pi 的安全边界

**【源码确认】**

- `bash` 默认启动本地 shell，继承进程环境并可访问网络；
- read/edit/write 接受相对或绝对路径，权限由 OS 决定；
- write 自动创建父目录；
- 工具层有 abort/timeout/输出截断，但这些不是 sandbox；
- 扩展可以注册工具、hook、消息与 Session 数据，扩大可信计算基。

**【运行实测】**本研究用 `--approve` 且工作目录属于 root。它验证功能，不验证交互审批或最小权限。

Marix 必须独立实现并保持：

- workspace path policy；
- shell allow/deny 与审批；
- 进程、网络和凭据隔离；
- 超时、取消与进程树清理；
- secret redaction；
- 可审计的 host error；
- extension/skill 信任边界。

`store:false` 只是 request 参数，不能被解释为 provider 绝不保留任何服务端日志。

## 17. 限制

1. 只研究一个 commit、一个包版本和一个 provider/model；
2. 只有三个短 headless cases 与一个 continuation；
3. 未加载 extensions、skills、project context；
4. 未实测 TUI、RPC、并行工具、steering queue、用户取消；
5. 未触发 compaction、split turn、branch summary 或 overflow retry；
6. 未覆盖 rate limit、provider 5xx、断网、SSE 中断；
7. Case C 的失败只是文件不存在，不代表所有工具错误；
8. provider cache 策略不可见，`cached_tokens` 只能作为观察值；
9. wire 是本地捕获证据，不是第三方签名；
10. 本次研究不重新 build/test/deploy，因此结论绑定已有 Ubuntu 产物与固定源码。

## 18. 安全复现步骤

以下步骤只描述可复核流程，不包含 secret：

1. 在隔离 Ubuntu 目录准备 commit `1d0c974…` 的源码和 Node.js `v22.23.2`；
2. 确认三个 package version 均为 `0.83.0`；
3. 将 provider key 只放进运行进程环境，不写入配置、task、manifest 或命令历史；
4. 在 OpenAI-compatible recorder 中，**写盘前**把 Authorization 替换为固定占位符；
5. recorder 转发 `/v1/chat/completions` 到 DeepSeek endpoint，同时保存 request body、HTTP status、raw SSE 与 parsed events；
6. CLI 子进程 stdin 使用 `DEVNULL`，避免非 TTY 管道等待 EOF；
7. 使用隔离 workspace 和 Session 目录分别执行 A/B/C；关闭 extensions、skills、context files；
8. continuation 必须复用 Case A 的 Session 文件，而不是新建 Session；
9. 比较 manifest 的 before/after SHA 与 changed 集合；
10. 以 `(wire path, requestId)` 对齐 request/response；
11. 使用明确序列化算法计算 tools structural hash，不能只写“SHA-256”；
12. 最后扫描 artifacts，确认没有 key、Bearer、Authorization 原值或其它凭据。

## 19. 后续建议

按优先级补充以下实测：

1. 构造可控长会话，分别触发 threshold compaction、split turn 和一次 overflow retry；
2. 运行 `/tree` 并验证 BranchSummaryEntry、旧/新 leaf 与 provider user-summary 投影；
3. 运行两个并行 tool calls，核对完成顺序与 wire 回放顺序；
4. 在不使用 `--approve` 时记录审批、拒绝与取消 event；
5. 加载一个最小 skill、context file 和 extension，分别测量 system prompt 与 schema hash 的变化；
6. 模拟 SSE 中断、429、5xx 和 tool timeout；
7. 对 compaction 的“tool result 前 2000 字符”做反例测试，评估 head+tail 改进；
8. 为 Marix 建立同样的三层证据：持久状态、模型投影、真实 wire，并用固定 case 做差异回归。

---

**最终判断：**Pi 最值得 Marix 借鉴的不是某段 prompt，而是“append-only、tree-shaped、provider-neutral session state 与 provider wire 投影分离”的架构，以及稳定 schema/前缀和显式 compaction boundary。最不应照搬的是把本机 OS 权限当作安全模型、把错误只留在自然语言 wire，以及把任务规划完全交给可选扩展。
