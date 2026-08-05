# Plandex + DeepSeek Planner/Executor 运行实测与固定源码研究

| 元数据 | 值 |
|---|---|
| 研究日期 | 2026-08-04 |
| 归档日期 | 2026-08-05 |
| 官方仓库 | `plandex-ai/plandex`（<https://github.com/plandex-ai/plandex>） |
| 固定 commit | `e2d772072efadbe41d2946d97d79be55532dbab5` |
| 固定版本 | Plandex CLI `2.2.1`；Server `2.2.1` |
| Provider / Model | OpenAI Chat Completions 兼容协议；`deepseek-v4-flash` |
| Ubuntu 研究根目录 | `/root/plandex-study` |
| 正式 Case | **A、B、C-real** |
| 无效前测 | `case-c`（首次真实执行前已虚构黑盒失败，不能作为 retry 证据） |
| 证据类型 | Ubuntu 运行实测、脱敏 HTTP/SSE wire、终端 trace、Postgres/plan data/CLI state、固定 commit 源码 |
| 证据标记 | **【运行实测】**为 wire、日志、终端或持久状态直接观察；**【源码确认】**为固定 commit 行为；**【分析判断】**为二者结合后的结论 |
| 敏感信息策略 | 凭据仅由运行环境临时提供；wire 不记录请求认证头；本文不包含任何密钥或可恢复凭据 |
| 固定源码链接 | <https://github.com/plandex-ai/plandex/tree/e2d772072efadbe41d2946d97d79be55532dbab5> |

## 1. 研究目标、范围与证据纪律

### 1.1 目标

本研究回答以下问题：

1. Plandex 是否真有内置 Planner/Executor，而不是把普通 agent loop 包装成“计划”；
2. Context、Planner、Subtask Executor、Builder、Status、Apply/debug 各阶段如何串联；
3. DeepSeek 在每一阶段收到的完整消息和请求字段是什么；
4. Plan、Subtask、conversation、pending files、apply 与 git ref 分别存在哪里；
5. 命令失败如何变成下一轮可用的真实 observation；
6. Plandex 与 Pi、Marix 在工具协议、状态、完成判断、缓存和安全边界上有什么本质差异。

### 1.2 范围

**【运行实测】**覆盖四组已完成运行：

- Case A：读取既有契约，实现 `invoice_summary`，执行验证；
- Case B：第一阶段生成依赖产物，第二阶段消费产物、构建并验证；
- Case C-real：第一轮严格按公开契约实现并真实失败，第二轮按真实输出修正；
- `case-c`：保留为无效前测，用来说明文本 observation 的污染风险。

**【源码确认】**覆盖固定 commit 下的 stage、prompt、parser、subtask store、context builder、status、builder、apply/debug、summary、stream/UI 链路。

本次归档只读取既有研究资产，没有重新部署或重跑 Case。九条缺失的 validation response 不值得为此重跑完整实验。

### 1.3 证据边界

- “真实失败”只指 CLI 实际执行命令后产生非零退出，并能在终端、失败回注 prompt 和后续 wire 中交叉确认。
- 模型在执行前预测或复述出的 traceback，不算运行 observation。
- 本地 wire proxy 提供的是研究环境内的真实 HTTP 往返记录，不是第三方签名或 packet-level 取证。
- 小 Case 未触发的次数上限、task 删除、同名修改、上下文溢出和强模型 fallback，只按源码结论记录，不冒充运行实测。

## 2. 结论摘要

| 结论 | 证据 |
|---|---|
| Plandex 确有核心内置的 **Context → Planner → Subtask Executor** 控制流 | 【运行实测】stage/request 序列；【源码确认】`tell_stage.go`、`tell_exec.go` |
| 它不是 API Tool ReAct | 146 个 request 的 `tools`、legacy `functions` 总数均为 0；`tool_choice`、`response_format` 均未发送 |
| 计划、文件操作、完成和状态判断依赖文本/XML协议 | `### Tasks`、`Uses:`、`<PlandexBlock>`、`<PlandexFinish/>`、`<subtaskStatus>` |
| 正式三 Case 共 75 request、71 个完整 response、859,609 tokens | `runtime-summary.json` |
| 唯一完整成立的“真实失败 → 回注 → replan → 修正 → 成功”证据是 C-real | 终端 `Commands failed`、`pdx-0132`、`pdx-0134`、最终 `case-c-real-ok` |
| Case B 有真实跨轮产物和 builder 修正，但没有真实命令失败 | 终端无 `Commands failed`；模型在 apply 前已写出预期 traceback |
| `case-c` 证明自由文本可把虚构 observation 污染为后续事实 | 真实执行前模型已声称黑盒失败；最终终端只出现成功 |
| completion 有“精确 marker 快路径”和“独立 Status LLM 慢路径” | `exec_status.go:49-125` |
| 同一 task 达到四条 previous messages 后会被强制完成 | `MaxPreviousMessages=4`，存在 false-completion 风险 |
| 历史 assistant 消息通常被重新投影为 wire `user`；summary 才是 `assistant` | `tell_summary.go:144-224`；wire 角色序列 |
| provider 返回的 `reasoning_content` 不进入下一轮 replay | 完整 SSE 有 reasoning；持久 conversation 与下一轮 messages 只有文本 content |
| Safety 主要是确认、自动模式和 rollback，不是 OS sandbox | 命令继承 CLI 用户权限；未观察到强制文件、进程或网络隔离 |

## 3. 此前“卡住”的根因与恢复

### 3.1 直接根因

**【运行实测】**遗留进程链 `528255/528257/528258` 是交互式 `plandex sign-in`，却运行在无 TTY 的自动化环境。脱敏日志为：

```text
Failed to get user input
Could not open a new TTY
Open /dev/tty
No such device or address
```

它等待交互输入，表面上像研究卡住；实际与 A/B/C-real 的 Case 执行无关。正式 Case 的 model streams、workspace 和验证结果早已完成。真正缺失的是原研究会话的最终归纳。

### 3.2 恢复方式

恢复没有重新跑实验，而是只读检查：

1. 固定源码与版本；
2. 4 个 Case 的 283 行聚合 wire；
3. Postgres 导出、plan data、plan git ref 和 CLI state；
4. workspace 文件及 SHA-256；
5. 终端 raw trace、server/proxy/sign-in 日志；
6. 146 个 request 的完整重组；
7. 清理和密钥扫描结果。

遗留 sign-in 链随后按精确 PID 停止。它不是模型、server、Postgres 或 Case 控制流的死锁。

## 4. 部署、自托管、模型配置与 wire proxy

### 4.1 自托管拓扑

```text
Plandex CLI 2.2.1
  │ local-mode API
  ▼
Plandex Server 2.2.1 :18099
  ├─ Postgres 17 :15432
  ├─ bind mount: /root/plandex-study/server-data → /plandex-server
  └─ custom provider base URL: http://127.0.0.1:18080/v1
                                      │
                                      ▼
                         redacting wire proxy :18080
                                      │
                                      ▼
                         DeepSeek-compatible upstream
```

**【运行实测】**持久 container config 证明：

| 组件 | 实际配置 |
|---|---|
| 私有容器运行时 | 独立 `dockerd/containerd`，数据位于 `/root/plandex-study/docker-data` |
| Server | image `plandex-study-server:e2d7720`；`LOCAL_MODE=1`；`PORT=18099`；host network；非 privileged |
| Server data | `/root/plandex-study/server-data:/plandex-server` |
| Postgres | `postgres:17`，实际 `17.10`；监听 `15432`；host network；非 privileged |
| Postgres data | `/root/plandex-study/postgres-data:/var/lib/postgresql/data` |
| CLI | `/root/plandex-study/bin/plandex`，版本 `2.2.1` |
| 工作区状态 | 每个 Case 的 `.plandex-dev-v2/projects-v2.json` 记录本地 project id |

Server 启动日志显示其内部 LiteLLM proxy 也会启动，但本次自定义模型路径最终通过研究 wire proxy 发出 OpenAI-compatible Chat Completions。

### 4.2 Custom DeepSeek model

`/root/plandex-study/config/models.json` 定义：

- custom provider：`deepseek-study-proxy`；
- local base URL：`http://127.0.0.1:18080/v1`；
- custom model id：`deepseek/deepseek-v4-flash-study`；
- provider model name：`deepseek-v4-flash`；
- preferred output：XML；
- conversation/model/output/reserved limits：64,000 / 128,000 / 8,192 / 8,192 tokens；
- Planner、Coder、Architect、Summarizer、Builder、WholeFileBuilder、Names、CommitMessages、AutoContinue 全部映射到同一个 DeepSeek model；
- org-wide default model pack：`deepseek-v4-flash-study`。

因此本研究能比较阶段 prompt 和控制逻辑，而不会把角色差异误归因于不同模型。

### 4.3 Wire proxy 与脱敏

**【源码/运行资产确认】**`/root/plandex-study/bin/wire_proxy.py`：

1. 仅监听 `127.0.0.1:18080`；
2. 为每个 request 生成 `pdx-NNNN-xxxxxxxx`；
3. 在 request JSONL 中只写 `proxy_path` 和解析后的 body，不写入站请求头；
4. 凭据仅从进程环境读取，并只在转发时临时使用；
5. response headers 会过滤认证、cookie、连接和传输相关字段；
6. response body 完整转发并在读完后写入 JSONL；
7. proxy 日志与最终 analysis 文件均做过脱敏。

这也解释了 BrokenPipe：proxy 一边向 Plandex 转发 chunk，一边积累 response；只有完整读完并转发成功后才落 response 行。Plandex 命中自身终止条件后提前关连接，proxy 在写 chunk 时收到 BrokenPipe，留下 request 而没有完整 response 记录。

## 5. 真实控制流：不是 API Tool ReAct

### 5.1 模块图

```text
CLI user prompt
  → Server Tell / activate
  → resolveCurrentStage
      ├─ Planning.Context  → Architect 选择要加载的文件
      ├─ Planning.Tasks    → Planner 输出任务表
      └─ Implementation    → Coder 只处理 current subtask
  → 文本 parser
      ├─ ### Tasks / Uses:
      ├─ ### Remove Tasks
      ├─ <PlandexBlock ...>
      └─ 精确 completion marker
  → Builder
      ├─ whole-file merge / diff
      ├─ syntax 与 placement validation
      └─ 最多 3 轮 fix
  → Status
      ├─ marker 快路径
      └─ XML status-model 慢路径
  → 持久化 conversation / result / subtask
  → 第一个未完成 subtask
  → auto-continue 或 finished
  → CLI Apply pending files
  → CLI 执行 _apply.sh
      ├─ success → apply/commit bookkeeping
      └─ failure → rollback → 新 Tell(真实 exit/output) → replan/debug → 再 Apply
```

### 5.2 为什么不是 ReAct

模型没有调用 `read_file`、`write_file`、`run_command`、`apply_patch` 一类 native tools：

- Context 由 server 预组装；
- Planner 输出自然语言任务表；
- Executor 输出 `<PlandexBlock>`；
- server parser 把文本 block 转为 file operation；
- Builder 完成合并、diff 和修复；
- CLI Apply 后才执行 `_apply.sh`；
- 失败输出由 CLI 构造为新的 user prompt。

所以真实控制流是 **stage controller + 文本协议 + host pipeline**，不是“assistant tool call → tool result → assistant”的 API Tool ReAct。

## 6. 完整 wire 编排与请求字段

### 6.1 全量扫描

**【运行实测】**`wire-requests-full.jsonl` 全文件扫描结果：

- 146 行 / 146 request；
- SHA-256：`9a84d87ffc4b7e09d12b8427e02bb0745c24edb69e53614c51518060786fd7a6`；
- 137 个完整 response，9 个 request-only；
- `tools` schema 总数：0；
- legacy `functions` 总数：0；
- 63 个 request 带 `stop`，83 个不带；
- 每个 request 恰有一个 `system`；
- 全部 messages：`system=146`、`user=377`、`assistant=118`；
- 641 个 message 的 `content` 都是 multipart array，共 783 个 `type:"text"` part；
- 所有 request：`model="deepseek-v4-flash"`、`stream=true`、`stream_options.include_usage=true`；
- `tool_choice` 与 `response_format` 未发送；分析摘要将缺失值规范化为 `null`。

请求 body 实际只有两种字段集合：

```text
messages, model, stream, stream_options, temperature, top_p
messages, model, stop, stream, stream_options, temperature, top_p
```

这比“工具数组为空”更强：模型面对的 request 中根本没有 native tool list。

### 6.2 各阶段的文本协议

| Stage | 模型输入/输出协议 | Host 后处理 |
|---|---|---|
| Planner.Context | 末尾 user wrapper 标明 `CONTEXT`；输出 `### Categories` / `### Files` / `<PlandexFinish/>` | 激活并加载文件 |
| Planner.Tasks | 末尾 wrapper 标明 `PLANNING`；输出 `### Commands`、`### Tasks`、`Uses:`、`<PlandexFinish/>` | `ParseSubtasks` 持久化 |
| Executor.Implementation | system 首部含 `CURRENT TASK`；输出 action explanation + `<PlandexBlock lang="..." path="...">` + completion marker | stream parser 生成 file operation |
| Executor.Status | 单个 system message；输出 `<subtaskStatus><reasoning>...` XML | 解析 `subtaskFinished` |
| Builder validation/fix | system + XML/文本修复协议 | 更新 candidate、语法复验 |
| Builder whole-file merge | system + 原文件/候选片段 | 合并为完整文件 |
| Conversation summary | system + conversation 文本 | Postgres 新增 summary |
| Response description | system + assistant reply | 生成 UI 描述 |
| Pending summary | system + pending result | 生成 pending 摘要 |

### 6.3 Response

可得的 137 个 response 保存：

- 原始 SSE；
- `content`；
- `reasoning_content`；
- usage 和 cache token；
- finish reasons；
- chunk count 与解析错误。

九个缺失 response 并非上游失败：proxy 日志有 146 次 upstream HTTP 200；Plandex 提前关闭了 9 条 validation stream，导致本地完整 response 未落盘。

## 7. Plan、Subtask 与持久状态

### 7.1 Subtask 数据结构

```text
Subtask {
  title: string
  description: string
  usesFiles: string[]
  isFinished: bool
  numTries: int
}
```

**【源码确认】**

- 存储路径是每个 plan 目录下的 `subtasks.json`；
- current task 不单独持久化，而是每次按数组顺序取第一个 `isFinished=false`；
- task 未完成且没有任务表变化时，`numTries++`；
- task 完成后标记 `isFinished=true`，再从头选择首个未完成项。

### 7.2 新增、完成、删除和修改

文本协议允许：

- `### Tasks`：新增或按 prompt 契约修改 unfinished task；
- `### Remove Tasks`：按精确 title 删除；
- finished task 被保留，不可修改或删除；
- replan 的新 title 追加到 finished items 后。

但有一个源码风险：`tell_subtasks.go:122-137` 实际只保留 finished items，并追加“不存在同名 unfinished task”的新项；同名 unfinished replacement 没有显式加入 `updatedSubtasks`。这与 prompt 声称的“同名即可修改 unfinished task”不完全一致。小 Case 没触发，不能称为已发生故障。

### 7.3 多层状态

| 层 | 内容 |
|---|---|
| Postgres | plan/branch status、token counts、plan builds、model streams、conversation summaries |
| plan data 目录 | conversation、description、results、applies、context、`subtasks.json` |
| plan git repo | plan 自身 `HEAD`、branch ref、commit |
| CLI workspace | 项目文件、`.plandex-dev-v2/projects-v2.json` |
| 内存 active state | current stage、current subtask、pending build queues、stream 状态 |

四个 branch 最终都是 `finished`。只读导出中有 4 个 plan/branch、24 条 `plan_builds`、8 条 model streams、37 条 conversation summaries。`report.md` 恢复矩阵写了“26 builds”，而 `persistent-state.json` 的实际 Postgres 导出数组是 24 行；长期复核时应以原始导出为行数依据，避免把 26 个 builder-validation request 与 DB build row 混为一谈。

### 7.4 Plan git ref 与 CLI state

最终 plan git commit：

| Case | Plan id | Plan git commit |
|---|---|---|
| A | `875e867d-2d96-42c8-9b77-2acd76b2b0d6` | `555b124e0e6fc0852734dd7925d28fb77313422d` |
| B | `47cb2688-e029-4465-b950-470157a2c969` | `e59091f03583cbc6472b221441ad06e02c5416d6` |
| C（无效） | `8067a165-2c8a-4f99-b594-5390380e15d9` | `83f150f5ce0ca0dda4158e1991e900467db381eb` |
| C-real | `eaef972d-5563-46bb-96c3-02c4a93419e4` | `313b7b52b2babd11f58bdc0b514972ed3a901d46` |

CLI state 为每个 workspace 记录本地 project id；它与 server plan id 不是同一个标识。

## 8. Executor prompt 与 context

### 8.1 每轮可见信息

Implementation system prompt 包含：

1. 完整最新 task list：title、description、`Uses`、`Done`；
2. 唯一 current task 的 `Current subtask: yes`；
3. 独立 `### Current subtask` 区块；
4. current task 的 context files；
5. pending files 的最新 build 内容，覆盖旧 context body；
6. current task 使用 `_apply.sh` 时的已执行脚本历史与当前脚本；
7. conversation 或选定的 summary；
8. 末尾 user wrapper：stage、auto-continue prompt、OS、UTC timestamp、执行和格式规则。

Case A `pdx-0009` 的 system 长 101,485 chars；Case B `pdx-0040` 为 102,309 chars；C-real `pdx-0140` 为 100,843 chars。大 system 的主要来源是详细的 implementation/edit protocol，而不是 native tool schema。

### 8.2 Smart context

**【源码确认】**

- Implementation + smart context 时，普通 file context 只保留 current task 的 `UsesFiles`；
- pending files 同样按 `UsesFiles` 过滤；
- `_apply.sh` 不作为普通 pending file 注入，而由专门的 apply-script 区块处理；
- pending file body 优先于原 context body；
-激活文件按模型选择顺序，其余按名称稳定排序，利于缓存；
-超过 context token budget 时按排序顺序停止追加。

### 8.3 Apply script history

`ExecHistory()` 只注入已经 applied 的 `_apply.sh` **脚本文本**：

```text
Previously executed _apply.sh:
<script content>
```

它不会自动带回成功命令的 stdout。成功输出主要展示在 CLI；失败 stdout/stderr 则由 apply-debug 路径明确构造成新 Tell。

## 9. Completion、重试上限与风险

### 9.1 两条 completion 路径

1. **快路径**：当前 assistant 文本包含精确的  
   `**[exact task title]** has been completed`  
   服务端直接标记完成，不再调用 Status 模型。
2. **慢路径**：没有 marker 时，独立 status request 收到 user prompt、current task、同 task previous messages、latest message，并返回：

```xml
<subtaskStatus>
  <reasoning>...</reasoning>
  <subtaskFinished>true|false</subtaskFinished>
</subtaskStatus>
```

Case B 的 `pdx-0022`、`pdx-0038` 是两次真实慢路径，均判断 `_apply.sh` 验证子任务尚未完成。C-real 的 executor 使用精确 marker，未产生 status request。

### 9.2 各层上限

| 层 | 上限 | 风险 |
|---|---:|---|
| 同 task previous messages | 4 | 达到后直接强制完成，可能 false positive |
| CLI auto-debug | 默认 5 次 | 可避免无限命令重试，但仍可能重复高成本 replan |
| 主 Tell auto-continue | 200 iterations | 只是总保险丝，不代表 200 次内一定正确 |
| 单文件 validation/fix | 3 attempts | 最终仍可能保留 invalid candidate 并报告问题 |

`case-c` 的两个执行类 task 最终 `numTries=4`，恰好展示了四轮保护与文本循环风险，但不能证明所有上限路径。

### 9.3 Apply failure → observation → Tell → replan

CLI 失败路径：

1. 收到真实 `status` 和完整 `output`；
2. 根据 auto-debug/交互策略决定是否继续；
3. 有 pending workspace changes 时 rollback；
4. 构造：

```text
Execution failed with exit status N. Output:

<real stdout/stderr>

--
```

5. 设置 apply-debug flags；
6. 新调用 Tell；
7. Tell 重新进入 Context/Planning；
8. 再次 Apply，attempt 加一。

因此 failure observation 不是隐式 memory，也不是 tool result；它是 CLI 创建的新 user Tell。

## 10. Builder、diff、validation 与用户进度流

### 10.1 Builder

Executor 的 `<PlandexBlock>` 先被 stream processor 解析为：

- file path；
- candidate content；
- operation type；
- description；
- move/remove/reset 元数据。

Builder 随后：

1. 按 path 排队，同 path 串行、不同 path 可并行；
2. 加载原文件和 pending state；
3. 新文件直接构造；已有文件执行 whole-file merge/diff；
4. 做 syntax/placement validation；
5. validation 失败时最多三轮 fix；
6. 保存 PlanResult；
7. 发送 build progress。

Case B 和 C-real 均有 whole-file merge 与 validation/fix request；这部分是 Plandex 相比直接 write/edit 工具最值得借鉴的可观察流水线。

### 10.2 用户进度流

用户看到的不是模型 tool events，而是 Plandex 自有 stream protocol：

- `start` / `connectActive` / `heartbeat`；
- `reply`；
- `describing`；
- `buildInfo(path,numTokens,finished,removed)`；
- `loadContext`；
- `repliesFinished`；
- `finished`；
- `error` / `aborted`。

TUI 根据这些消息更新 reply、spinner、build token、折叠状态和退出状态。模型 request 与用户进度流是两套协议。

## 11. Summary、context compression 与 cache

### 11.1 Summary

每个主回复后会异步生成 conversation summary。实测：

- 37 个 summary request；
- Postgres 有 37 条 summary；
- 正式三 Case 有 23 个 summary request。

projection 只有在：

- `tokensBeforeConvo + conversationTokens` 超过有效模型上限，或
- conversation tokens 超过 planner conversation 上限

时，才选择一个已有 summary 替换较早 conversation。若任何 summary 仍不能压到限制内，Tell 失败。

这不是 Pi 的显式 compaction entry，而是 append-oriented summary store + 请求时选择边界。

### 11.2 Cache

Wire 没有显式 cache key，也没有实际发送 cache-control 字段。源码可在内部 part 上设置 ephemeral cache control，但 custom DeepSeek wire 最终没有该字段。

正式 Case 的观测命中：

- A `pdx-0005` Context：4,864 cached tokens；
- B `pdx-0024`、`pdx-0040` Implementation：各 18,688；
- B/C-real pending summary：各 128。

Plandex 的 cache-friendly 设计：

- planning shared context 位于 system 前部；
- context 文件稳定排序；
-动态 wrapper 放尾部。

但 implementation system 会随 current task、pending files 和 task table 整体变化，不如 Pi 的“稳定 system + 稳定 tool schema + append-only history”天然稳定。

## 12. DeepSeek role projection：特点与风险

### 12.1 实测角色形态

146 个 request 的角色总数：

```text
system    146
user      377
assistant 118
```

可出现：

- `system,user,user,...`；
- 多个连续 `user`；
- `system,assistant,...`；
- summary 插入在若干重新标记的历史消息之间。

DeepSeek 对全部 146 个 request 返回 upstream HTTP 200，没有发生角色格式拒绝。

### 12.2 为什么 assistant 会变成 user

`addConversationMessages()` 遍历持久 conversation 时，不检查原始 role，统一构造 wire `role=user`；只有选中的 summary 使用 `role=assistant`。因此：

- 原 assistant code proposal 可能成为下一轮的 user text；
- observation、用户指令和历史模型输出失去角色级来源区分；
- prompt injection 或虚构事实更容易在后续阶段获得“用户消息”权重。

### 12.3 `reasoning_content` 不 replay

DeepSeek SSE response 中存在 `reasoning_content`，但：

- plan conversation JSON 只保存可见 `message`；
-下一轮 projection 只使用该 message；
-没有重建原 assistant response 的 reasoning 字段。

本次没有 native tool call，所以未触发 DeepSeek thinking tool-call replay 的兼容要求；若未来加入 API tools，当前 projection 不能直接复用，否则会同时丢失原 role、tool call 配对和 reasoning provenance。

## 13. 正式 Case A：单轮实现与验证

### 13.1 输入与请求

目标：读取 `pricing.py`、`policy.txt` 和验收文件，实现 `invoice.py::invoice_summary`，运行 `python3 verify.py`。

共 15 request：

| Stage | 数量 | Request |
|---|---:|---|
| Context | 3 | `0001,0003,0005` |
| Plan | 1 | `0007` |
| Executor | 2 | `0009,0012` |
| Conversation summary | 6 | `0002,0004,0006,0008,0011,0014` |
| Response description | 2 | `0010,0013` |
| Pending summary | 1 | `0015` |

### 13.2 Plan 与 Subtask

`pdx-0007` 输出：

1. 在新 `invoice.py` 实现 `invoice_summary`；
2. 在 `_apply.sh` 中加入验证并运行。

Uses 精确限定了 `verify.py`、`pricing.py`、`policy.txt`、`invoice.py`、`_apply.sh`。

### 13.3 执行、Builder、Apply 与验证

- `pdx-0009` 只实现第一个 current task；
- `invoice.py` 导入并复用 `subtotal`、`discounted`；
-校验 discount 为 0–100；
-输出 subtotal、percent、final total，金额两位小数；
- `pdx-0012` 创建 `_apply.sh` 和 `.plandexignore`；
- CLI 真实运行后输出 `case-a-ok`。

### 13.4 文件结果

| 文件 | 结果 |
|---|---|
| `invoice.py` | 新增；SHA-256 `170dbb30a41e5196b1c329195b082b6dd49920ed6b85686c63435b47050fd79e` |
| `.plandexignore` | 新增；排除 `__pycache__/` 和 `*.pyc` |
| `pricing.py`、`policy.txt`、`verify.py` | 未修改 |

### 13.5 异常限定

验证成功后，CLI 对 `/apply` 的 bookkeeping PATCH 超时，显示 “Failed to set pending results applied”。因此：

- workspace 正确；
-验证真实成功；
- PlanResult 存在；
- branch 最终 `finished`；
-但 `applies/` 没有 Case A apply record。

这是 post-success metadata failure，不应误写成代码或验证失败。

## 14. 正式 Case B：跨轮产物、Builder 修正与成功验证

### 14.1 总体请求

Case B 共 32 request、30 完整 response；分两阶段：

- 第一阶段 10 request；
-第二阶段 22 request。

| Stage | 数量 | Request |
|---|---:|---|
| Context | 2 | `0016,0026` |
| Plan | 2 | `0018,0028` |
| Executor | 5 | `0020,0024,0030,0034,0040` |
| Status | 2 | `0022,0038` |
| Builder validation/fix | 5 | `0031,0036,0041,0042,0046` |
| Whole-file merge | 2 | `0035,0045` |
| Summary | 9 | `0017,0019,0023,0025,0027,0029,0033,0039,0044` |
| Description | 4 | `0021,0032,0037,0043` |
| Pending summary | 1 | `0047` |

`0036`、`0046` 因 BrokenPipe 没有完整 response。

### 14.2 第一阶段：生成真实依赖产物

Plan 只有一个执行 task：

1. 通过 `_apply.sh` 运行 `discover.py`；
2. 生成、打印并解析 `generated_schema.json`；
3. 不修改 `consumer.py`。

真实结果：

```json
{
  "keys": ["alpha", "beta"],
  "total": 10,
  "format": "key=count"
}
```

来源：

- `keys`：输入 key 去重排序；
- `total`：`3 + 5 + 2 = 10`；
- `format`：固定 `"key=count"`。

这是真实跨轮文件 observation：第二阶段重新加载了 workspace 中的 `generated_schema.json`。

### 14.3 第二阶段：实现与验证

Plan：

1. 在 `consumer.py` 实现 `render(schema)`；
2. 用 `_apply.sh` 运行 `verify.py` 并迭代。

执行：

-初始实现从 `source.txt` 累加得到 `alpha=5,beta=5`；
-验收要求是 `alpha=5; beta=3; total=10`；
- Builder 做 whole-file merge 和 validation/fix；
-两次 Status 判断执行 task 尚未完成；
-最终实现使用 `{"alpha":5,"beta":3}`，真实终端输出 `case-b-ok`。

最终 `consumer.py` SHA-256：

`9b38146b87afbe7ff4f1dd22169a8c0b1cbfd210cc7887d7d2e8bf18616090ed`

最终 `generated_schema.json` SHA-256：

`a665434daa7605248de082be5b78e92f9d0e47688256404af8c362ebf5d5f877`

### 14.4 真实 observation 与虚构 observation 的边界

模型在真正 Apply 前已经在自己的文本中预测了：

```text
AssertionError: alpha=5; beta=5 ... alpha=5; beta=3
```

并据此继续修正。`run-2.raw` 有模型文本里的 traceback，但没有 `Commands failed`。所以：

-真实：最终命令执行成功并打印 `case-b-ok`；
-真实：builder merge/validation/fix；
-真实：跨阶段生成和加载 `generated_schema.json`；
-不真实：把 apply 前预测的 AssertionError 当成命令 observation。

Case B 是正式 Case，但不是 runtime command-failure retry 证据。

## 15. 无效 `case-c` 前测：虚构 observation 污染

### 15.1 计划与循环

初始计划有四项：

1. 按 v1 契约实现 `normalize`；
2. 执行第一次黑盒验证；
3. 按第一次真实失败修正；
4. 重跑直到 `case-c-ok`。

最终 `subtasks.json` 显示两个执行 task 的 `numTries=4`。

### 15.2 为什么无效

模型在 `_apply.sh` 尚未真实执行时，就把隐藏规则和失败内容写进 assistant 文本，后续 Status/Builder 又把这些文本当作历史事实。最终终端只有 `case-c-ok`，没有 `Commands failed`。

因此不能从该 Case 推出：

```text
真实失败输出 → retry → 成功
```

它只能证明：

```text
模型虚构 output → 自由文本进入 conversation → 后续阶段据此循环
```

### 15.3 成本

- 71 request、66 完整 response；
- 763,677 tokens；
- 12 Executor；
- 10 Status；
- 21 Builder；
-最终真实执行只成功。

与 C-real 相比，这个前测直观展示了文本 observation 没有 provenance/type 边界时的成本和风险。

## 16. 正式 Case C-real：真实失败、回注、replan 与修正

### 16.1 请求总览

共 28 request：

| Stage | 数量 | Request |
|---|---:|---|
| Context | 2 | `0119,0132` |
| Plan | 2 | `0121,0134` |
| Executor | 4 | `0123,0127,0136,0140` |
| Builder validation/fix | 5 | `0124,0131,0137,0142,0143` |
| Whole-file merge | 2 | `0130,0141` |
| Summary | 8 | `0120,0122,0126,0129,0133,0135,0139,0145` |
| Description | 4 | `0125,0128,0138,0144` |
| Pending summary | 1 | `0146` |

`0131`、`0142` 没有完整 response。

### 16.2 第一轮（`0119`–`0131`）

Plan：

1. 按 `spec.md` v1 契约实现 `name.strip().lower()`；
2. 创建 `_apply.sh`，启用 `set -euo pipefail`，只运行一次验证并保留输出。

Executor：

- `pdx-0123` 修改 `normalizer.py`；
- `pdx-0127` 创建一次性 `_apply.sh`；
-没有读取环境变量；
-没有预写迁移后缀。

真实 Apply：

```text
Traceback ...
AssertionError: runtime migration mismatch:
actual='alice'; required suffix revealed by execution='-r7'
Commands failed
Rolled back all changes
```

这是唯一同时满足终端非零退出、rollback、失败 output 和后续 wire 回注的正式证据。

### 16.3 第二轮（`0132`–`0146`）

`pdx-0132` 的最后 user message 完整包含：

- `Execution failed with exit status 1`；
-真实 traceback；
- `actual='alice'`；
-执行揭示的 `-r7`。

它不是模型自己生成的历史 assistant 文本，而是 CLI apply-debug 创建的新 Tell。

`pdx-0134` 重新规划，保留前两项 finished task，并新增：

3. 按真实失败输出为 `normalize` 追加 `-r7`；
4. 再次通过 `_apply.sh` 执行一次真实验证。

随后：

- `pdx-0136` 修改 `normalizer.py`；
- `pdx-0140` 保持单次验证脚本；
-第二次 Apply 成功；
-终端和 `verification_output.txt` 均为 `case-c-real-ok`。

### 16.4 最终文件

| 文件 | 结果 |
|---|---|
| `normalizer.py` | `name.strip().lower() + "-r7"`；SHA-256 `6bb95594ec6dcc967428923702ea489bad00986a94c0f471cbcd1bee1697d2fc` |
| `verification_output.txt` | `case-c-real-ok`；SHA-256 `56058d19498fa53578b6146d47342456e041f31670d2b72d17368d583e242ad6` |
| `verify.py` | 未修改；SHA-256 `149c8c4f87cd476a37e8bdc999450649e072e886001d706731794debbcd5e45d` |
| `spec.md` | 未修改；SHA-256 `1acf0ca475c2287c3a9d6750145d671e249e24755fed0fb20c16055c1fdfdd40` |

### 16.5 判定

这是 **apply-debug retry + Context/Planning replan**，不是初始计划内的预写修复。`-r7` 修正对应的两个 task 只在失败 observation 后出现。

## 17. Request ID / Case / Stage 映射

以下 ID 省略公共 `pdx-` 前缀和随机后缀。

| Case | Stage | Request IDs |
|---|---|---|
| A | Context | `0001,0003,0005` |
| A | Plan | `0007` |
| A | Executor | `0009,0012` |
| A | Summary / Description / Pending | `0002,0004,0006,0008,0011,0014` / `0010,0013` / `0015` |
| B | Context / Plan | `0016,0026` / `0018,0028` |
| B | Executor / Status | `0020,0024,0030,0034,0040` / `0022,0038` |
| B | Validation / Whole merge | `0031,0036,0041,0042,0046` / `0035,0045` |
| B | Summary | `0017,0019,0023,0025,0027,0029,0033,0039,0044` |
| B | Description / Pending | `0021,0032,0037,0043` / `0047` |
| C 无效 | Context / Plan | `0048` / `0050` |
| C 无效 | Executor | `0052,0057,0063,0068,0075,0081,0087,0094,0099,0104,0109,0114` |
| C 无效 | Status | `0055,0059,0066,0073,0079,0092,0097,0102,0107,0112` |
| C 无效 | Validation | `0053,0061,0064,0070,0071,0076,0077,0083,0084,0089,0090,0095,0101,0105,0110,0115` |
| C 无效 | Whole merge | `0060,0069,0082,0088,0100` |
| C-real | Context / Plan | `0119,0132` / `0121,0134` |
| C-real | Executor | `0123,0127,0136,0140` |
| C-real | Validation / Whole merge | `0124,0131,0137,0142,0143` / `0130,0141` |
| C-real | Summary | `0120,0122,0126,0129,0133,0135,0139,0145` |
| C-real | Description / Pending | `0125,0128,0138,0144` / `0146` |

关键锚点：

| Request | 含义 |
|---|---|
| `pdx-0007` | A 的 `### Tasks` 计划 |
| `pdx-0009` | A 首个 current task；system 含完整 task/context |
| `pdx-0022`,`pdx-0038` | B 的真实 Status 慢路径 |
| `pdx-0024`,`pdx-0040` | B Implementation，各 18,688 cached tokens |
| `pdx-0132` | C-real 真实 exit 1/output 进入新 Tell |
| `pdx-0134` | C-real replan，新增 `-r7` 修正与重跑 task |
| `pdx-0140` | C-real 最终验证 task |
| `pdx-0146` | C-real pending summary，128 cached tokens |

## 18. 固定源码证据表

以下路径相对于固定源码 `/root/plandex-study/source`。

| 模块 | 路径 / 符号 / 行区间 | 证实内容 |
|---|---|---|
| Stage | `app/server/model/plan/tell_stage.go::resolveCurrentStage:22-104` | user prompt→Planning；`DidMakePlan`/implementation→Implementation；Context/Tasks phase |
| 主循环/角色选择 | `tell_exec.go::execTellPlan:86-207,392-430,494-581` | Architect/Planner/Coder 选择、request 组装、stream |
| Planner prompt | `app/server/model/prompts/planning.go::GetPlanningPrompt:12-145`; `ReviseSubtasksPrompt:311-330` | `### Commands`、`### Tasks`、`Uses:`、finish、增删改协议 |
| Parser | `app/server/model/parse/subtasks.go::ParseSubtasks/ParseRemoveSubtasks:10-120` | split、编号行、Uses、Remove Tasks |
| Subtask schema | `app/server/db/data_models.go::Subtask:784-801` | title/description/UsesFiles/isFinished/numTries |
| Subtask store | `app/server/db/subtask_helpers.go::GetPlanSubtasks/StorePlanSubtasks:10-49` | `subtasks.json` |
| Current task | `tell_load.go:352-369`; `tell_stream_store.go:62-98` | 首个 unfinished；完成后前移；未完成 `NumTries++` |
| Task prompt/merge | `tell_subtasks.go:14-77,85-249` | 完整 task table、current、Done、新增/删除与同名风险 |
| Executor prompt | `prompts/user_prompt.go:107-180,300-339`; `prompts/implement.go:271-304` | `PlandexBlock`、current-only、completion marker、auto-continue |
| Context | `tell_context.go::formatModelContext:16-365` | UsesFiles、pending overlay、稳定排序、token cap、apply history |
| Status | `exec_status.go::execStatusShouldContinue:21-190`; `prompts/exec_status.go:10-98` | marker 快路径、XML 慢路径、四条强制完成 |
| Request builder | `app/server/model/model_request.go::ModelRequest:20-263` | tools 仅非空才发送；temperature/top-p/stop/stream/usage |
| Text operation parser | `tell_stream_processor.go:538-587,738-747` | `<PlandexBlock>` → host file operations |
| Builder | `build_exec.go:30-220`; `build_validate_and_fix.go::buildValidateLoop:22-159` | path queue、whole build、三轮 validation/fix |
| Apply/debug | `app/cli/plan_exec/apply_exec.go::getOnApplyExecFail:18-169` | rollback、真实 output 新 Tell、retry |
| Exec history | `app/shared/plan_result_exec_history.go::ExecHistory:3-17` | 只回放已执行脚本文本 |
| Summary/角色 | `tell_summary.go::addConversationMessages:22-230` | token 阈值；历史统一 user；summary assistant |
| Auto limits | `tell_stream_finish.go:18`; `tell_stream_status.go:212-213`; `shared/plan_config.go:10,136-143` | 200 auto-continue；默认 5 auto-debug |
| Status enum | `app/shared/plan_status.go:3-13` | draft/replying/describing/building/missing/finished/stopped/error |
| Stream/UI | `app/shared/stream.go:3-48`; `app/cli/stream_tui/update.go:400-546` | reply/build/load/error/finished 事件和 TUI 更新 |

## 19. 运行证据索引

### 19.1 Analysis 产物

| 文件 | 用途 |
|---|---|
| `/root/plandex-study/analysis/report.md` | 恢复研究报告和结论 |
| `/root/plandex-study/analysis/evidence-index.md` | Case、request、源码索引 |
| `/root/plandex-study/analysis/runtime-summary.json` | 146 个 request 的 case/stage/roles/params/usage/cache |
| `/root/plandex-study/analysis/wire-requests-full.jsonl` | 完整 request body、可得 SSE、content/reasoning/usage |
| `/root/plandex-study/analysis/persistent-state.json` | Postgres、plan data、subtasks、git ref、CLI state、workspace |
| `/root/plandex-study/analysis/runtime-log-evidence.json` | 成功、失败、BrokenPipe、bookkeeping 计数 |
| `/root/plandex-study/analysis/server-log-sanitized.txt` | stage、subtask、status、builder、stream 生命周期 |
| `/root/plandex-study/analysis/wire-proxy-log-sanitized.txt` | upstream 200 与 BrokenPipe |
| `/root/plandex-study/analysis/sign-in-log-sanitized.txt` | 无 TTY sign-in 根因 |
| `/root/plandex-study/analysis/secret-scan.json` | 最终密钥扫描 |
| `/root/plandex-study/analysis/process-cleanup.json` | 精确 PID/容器清理与残留检查 |

### 19.2 原始 Case

| Case | 终端/输入 | Wire | Plan |
|---|---|---|---|
| A | `traces/case-a/user-input.txt`, `run.raw`, `final-state.txt` | `traces/case-a/wire.jsonl` | `875e867d-...` |
| B | `prompt-1.txt`, `prompt-2.txt`, `run-1.raw`, `run-2.raw` | `traces/case-b/wire.jsonl` | `47cb2688-...` |
| C 无效 | `user-input.txt`, `run.raw` | `traces/case-c/wire.jsonl` | `8067a165-...` |
| C-real | `prompt-1.txt`, `prompt-2.txt`, `run-1.raw`, `run-1.exit-code` | `traces/case-c-real/wire.jsonl` | `eaef972d-...` |

聚合 wire：`/root/plandex-study/traces/wire/wire.jsonl`。不重叠区间：

- A：1–30；
- B：31–92；
- C：93–229；
- C-real：230–283。

## 20. 与 Pi、Marix 的对比

### 20.1 架构表

| 维度 | Plandex | Pi | Marix |
|---|---|---|---|
| 核心编排 | 固定 Context→Planner→Subtask Executor→Builder→Apply | 单一 native-tool agent loop；plan 多由扩展提供 | hierarchical Intent + workflow native calls |
| 模型 tool list | **本次完全没有 native tool list** | 每轮重发 read/bash/write/edit schema | workflow 与普通 tools 都是 typed calls |
| 计划协议 | `### Tasks` / `Uses:` 文本 | 默认核心无固定 task planner | `workflow_plan` 结构化 schema |
| 文件操作 | `<PlandexBlock>` → server parser/builder | native write/edit 直接执行 | ordinary tools；workflow 负责路由 |
| Observation | context/pending；失败由 CLI 新 Tell 回注 | `assistant.tool_calls` / `tool` 原生配对 | Current Task 的 typed Completed Calls |
| 状态 | flat ordered subtasks | tree-shaped transcript / active branch | `IntentSignature.parent` 层级 |
| 完成 | marker 或 Status LLM；四条后强制 done | 模型停止/终结回复 | `workflow_complete` + task-scoped evidence |
| 终态 | plan status，难表达 infeasible/canceled 的任务语义 | session/tool error | Succeed/Infeasible/Canceled/Failed |
| 角色 replay | 历史常统一为 user | 保持 user/assistant/tool 和 call id | 应保持 provider-neutral transcript 与投影分层 |
| Reasoning replay | 不 replay | tool-call assistant reasoning 可 replay | provider adapter 应按协议保留 |
| Context | UsesFiles + pending overlay + summary | 活跃 branch + compaction | Current Task + Completed Calls + policy |
| Cache | 大而动态的 stage system | 稳定 system/tools + append tail | 稳定 policy + 动态 task tail |
| Safety | approval/auto/rollback；无强制 sandbox | 默认继承本机权限 | 应把权限、sandbox、确认、审计分开 |

### 20.2 Plandex 相对 Pi

Plandex 值得肯定：

- Planner/Executor 是核心模块，不依赖可选扩展；
- task、UsesFiles、pending files、build 和 apply 都有独立持久状态；
-文件生成有 server-side builder/diff/validation；
-命令失败会 rollback 并进入明确的 debug/replan。

Pi 更强之处：

- provider wire 保持原生 assistant/tool 配对；
- call id、tool result 和 reasoning provenance 更清晰；
- Session transcript 与 provider projection 分离；
-稳定工具 schema 和 append-only history 更利于 cache。

Plandex 的文本 parser 名称（`didFinishSubtask`、`PlandexBlock`、`Task`）不能当成 native tool；Pi 的工具则确实出现在 provider request 的 `tools` 中。

### 20.3 Plandex 相对 Marix

可借鉴：

1. Planner、current task、Builder、Apply/debug 分为可观察模块；
2. task 的 `UsesFiles` 持久化，使 context selection 可审计；
3. pending file 覆盖旧 context，而不是让模型读到过期版本；
4. 文件构建有独立 diff/validation/fix；
5. command failure 先 rollback，再作为明确的新 observation；
6. stage、build、stream、apply 都能单独记录。

不可照搬：

1. 不要把自由文本当作 observation；
2. 不要把历史 assistant 重新标成 user；
3. 不要信任 completion marker 绕过 side-effect evidence；
4. 不要以四条消息强制 done 代替 requirement coverage；
5. 不要用 flat task list 表达 canceled/failed/infeasible；
6. 不要让 prompt 的同名 task 修改契约与 store merge 逻辑不一致；
7. 不要因 host parser 里有“function/tool”符号，就声称模型有 native tools。

Marix 的 hierarchical Intent、Current-Task-scoped Completed Calls 和四种 terminal result 比 Plandex 的文本 marker/flat task 更适合隔离父子任务与失败语义。Marix 最值得吸收的是 **可见 Builder/diff/apply-debug pipeline**，而不是 Plandex 的文本完成协议。

## 21. Safety、限制与反模式

### 21.1 Safety

Plandex 本次可见的安全措施：

- auto/semi 模式配置；
- Apply 和执行确认；
-失败 rollback；
-分 plan/branch 的持久状态；
-stream error/aborted 状态。

未观察到：

-强制 filesystem sandbox；
-进程隔离；
-网络隔离；
-命令 capability allowlist；
-把模型 observation 与真实 host observation 做类型隔离。

确认和 rollback 不能等价为 sandbox。

### 21.2 已知限制

1. 九个 `builder.validation-fix` response 因 BrokenPipe 未落盘：  
   `0036,0046,0061,0070,0083,0089,0101,0131,0142`。
2. 它们的 request、upstream HTTP 200、后续持久结果仍在，但不能声称拥有完整 response。
3. 短 Case 没覆盖 200 次 auto-continue、5 次 auto-debug、全部 3 次 validation failure 后的最终退化行为。
4. 没有运行真实 task 删除和同名 unfinished replacement。
5. 没有触发 conversation overflow 后“任何 summary 仍压不下去”的错误。
6. Case A 的 apply metadata 缺失，不能据此统计为完整 apply record。
7. Case B 最终硬编码验收值，证明 pipeline 能完成验收，不代表实现具有通用业务正确性。
8. Provider 自动 cache 的切分和淘汰策略不可见。
9. 本地 proxy 证据没有密码学不可否认性。

### 21.3 主要反模式

-自由文本混合指令、proposal、observation 和 completion；
-模型可以在执行前“预测”命令输出；
-精确字符串 marker 被直接信任；
-四轮强制完成以正确性换终止性；
-历史角色扁平化；
-复杂、超长 stage prompt；
-prompt/store 契约可能漂移；
-把 host parser/function 名误称为 model-facing tool。

## 22. 清理状态与密钥扫描

### 22.1 清理

`process-cleanup.json` 记录：

-停止 Plandex server container；
-停止 Postgres container；
-停止 wire proxy 精确 PID；
-停止私有 `dockerd`，其 `containerd` 随之正常退出；
-遗留 sign-in PID 均不再存活；
-监听 `15432`、`18080`、`18099` 均为 false；
-已知 PID 均不存活；
-`/root/plandex-study` 下无残留研究进程；
-`clean=true`。

源码、Docker/Postgres data、server data、日志、traces、workspace 和 analysis 均保留，便于长期复核。

### 22.2 密钥扫描

`secret-scan.json`：

-扫描范围：`logs`、`traces`、`analysis`；
-扫描文件：47；
-剩余精确密钥命中：**0**；
-剩余命中文件：空数组；
-扫描后唯一新增的 `process-cleanup.json` 只含 PID、container id、状态和固定日志尾，不含环境或凭据。

## 23. 最终判定

1. **Plandex 是真正的内置 Planner/Executor 系统，但不是 native-tool agent。**
2. **146 个真实 request 均没有模型面对的 tool list。**
3. **它的强项是 stage、Subtask、Builder、Apply/debug 的显式分层和持久化。**
4. **它的主要风险是自由文本 observation、角色扁平化、marker completion 与四轮强制完成。**
5. **C-real 完整证明了真实失败经 CLI 回注、重新 Context/Plan、添加 task、修正和再次 Apply 的闭环。**
6. **`case-c` 和 Case B 同时证明：模型写出的“像 traceback 的文本”绝不能自动视为真实执行证据。**
7. **对 Marix，应借鉴可观察的 build/diff/apply-debug pipeline，保留 typed workflow、role provenance、Current Task evidence 与四态终结模型。**

