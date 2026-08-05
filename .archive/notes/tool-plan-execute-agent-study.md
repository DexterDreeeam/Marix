# LangGraph Plan-and-Execute 外部 Agent 深研归档

**日期：** 2026-08-05  
**运行位置：** Ubuntu `/root/tool-plan-execute-study/`（与 Marix 现有服务、端口隔离）  
**模型：** 配置中的 `deepseek-v4-flash`；实际 key 只在透明代理进程环境中，不在本归档、trace、stdout、headers 或报告中。

## 选择与证据边界

|层面|结论|
|---|---|
|固定官方架构|`langchain-ai/langgraph@23961cff61a42b52525f3b20b4094d8d2fba1744` 的 `docs/docs/tutorials/plan-and-execute/plan-and-execute.ipynb` 是真正外层 `Planner → ExecuteOne → Replanner → Complete/ExecuteOne` 图。|
|当前源码|快照 `b2926a0ff9589c28c7e01fe7cdbb337b86d5a4b4`；当前示例位于 `examples/plan-and-execute/plan-and-execute.ipynb`。|
|实际 runtime|Python 3.14.4、LangGraph 1.2.10、LangChain 1.3.14、langchain-openai 1.4.1、OpenAI SDK 2.53.0。旧 notebook 未直接运行；改以当前 `StateGraph` + `langgraph.prebuilt.create_react_agent` 构建**忠实 harness**。|
|A/B 结论|A：真实 Plan-and-Execute。B：Planner、Replanner、Executor 都用 DeepSeek OpenAI-compatible API 的原生 `tools`/function calling；并非文本 JSON 约定。|

官方源码定位：

- 历史 notebook：cell 9 L3–8 `create_react_agent`；cell 12 L6–10 `PlanExecute(input, plan, past_steps, response)`；cell 15–16 Planner structured output；cell 19 Replanner `Act/Response` structured output；cell 21 L5–36 三节点与结束函数；cell 22 L6–32 节点、边和条件边。  
  <https://github.com/langchain-ai/langgraph/blob/23961cff61a42b52525f3b20b4094d8d2fba1744/docs/docs/tutorials/plan-and-execute/plan-and-execute.ipynb>
- 当前 prebuilt ReAct：`libs/prebuilt/langgraph/prebuilt/chat_agent_executor.py` L173–195、L278、L958–986。  
  <https://github.com/langchain-ai/langgraph/blob/b2926a0ff9589c28c7e01fe7cdbb337b86d5a4b4/libs/prebuilt/langgraph/prebuilt/chat_agent_executor.py>
- 当前 `StateGraph.add_conditional_edges`：`libs/langgraph/langgraph/graph/state.py` L969；Pregel/checkpointer：`pregel/main.py` L450、L839–840、L1396–1493；checkpoint contract：`checkpoint/base/__init__.py` L176。  
- 已安装 OpenAI adapter 的本地证据：`venv/.../langchain_openai/chat_models/base.py` 的 `_convert_message_to_dict` L388、`_get_request_payload` L1775–1801、`bind_tools` L2212–2259。

## 四阶段与真实 wire 模板

1. **Planner**：`system + user task + tools:[submit_plan]` → DeepSeek streaming SSE → assistant `tool_calls[submit_plan]` → `plan[]`。
2. **ExecuteOne**：只取 outer `plan[0]`；prebuilt ReAct 发 `system/current step + user + tools:[list_directory, read_file, search_text, write_file, run_command]`。每个循环是 `assistant tool_calls → host result → 下一请求 role:"tool", tool_call_id`。
3. **Replanner**：输入 original task、`remaining_plan_before`、`past_steps`；带 `tools:[submit_replan]`，输出 revised remaining plan 或 final response。
4. **Complete/loop**：`StateGraph.add_conditional_edges` 路由 END 或下一次 ExecuteOne。实测 outer recursion limit=30，inner ReAct=24，另设 audited outer iteration limit=6。

透明代理记录了 27 个实际 request 的完整 messages、完整 tool schema、model、tool_choice、response_format、temperature/top_p/max tokens、stream/thinking 和完整 SSE；不记录 headers，Authorization 明确 redacted。Planner/Replanner 未触发文本 fallback，显式 `tool_choice` 未发送。

## Tool List / Output 与宿主边界

|工具|模型可见契约|宿主限制|
|---|---|---|
|`list_directory`|相对目录列举|canonical path 必须在 case workspace 内|
|`read_file`|读取 UTF-8 文本|越界/不存在返回 `error:true`|
|`search_text`|相对路径文字搜索|只扫描 workspace|
|`write_file`|写入 UTF-8 文件|只允许 workspace|
|`run_command`|只读本地命令|仅 `pwd/ls/find/wc/sha256sum/grep/sed/cat` allowlist；无 shell、网络或写入|

每个真实结果都形如 `{call_id, error, text}`；事件另记 arguments、current step、trace tag。模型输入 schema 和宿主输出 schema 是不同边界；后者不会自动改善模型的工具选择。

## 三个 Case

|Case|实际多轮结果|核心证明|
|---|---|---|
|A 文件任务|计划 3 步；读取 `alpha/beta`、写 `overview.md`、read-back。|多真实 API 工具调用与写入后验证。|
|B 依赖任务|第 1 个 current step：list → search `release` → read；Replanner 更新剩余计划；第 2 个 current step：write → read → `wc`。|外层每次只调度一个 current step；同 step 的内层 ReAct 可以多工具回合；tool output 已回注后续 request。|
|C 失败恢复|首 call `read_file(docs/missing.txt)` 返回真实 `FileNotFoundError/error:true`；同一 ReAct step 随后 list/search/read 修复；下一 outer step 写并 read-back `recovered.txt`。|同 step ReAct retry/recovery 与外层 replan/complete 的清晰边界。|

## State、Context、Cache

- `state-transitions.jsonl` 保存 input、plan、past_steps、current step、Executor messages、Replanner output、新 plan/response、transition、iteration 和 recursion limit。
- 每个 proxy request 使用 trace tag 映射到 node/step；`analysis/wire-map.json` 提供 request ID → graph node → assistant calls → tool output injection 的索引。
- 历史 notebook 以 `past_steps` 追加作为 outer memory。该 harness 的 inner ReAct transcript 只在一次 ExecuteOne 内累积；跨 step 向 Replanner 提供的是 `past_steps`，不是长期 coding-agent 全 transcript。
- executor 工具 schema 前缀稳定；task/current step/history/output 是动态后缀。DeepSeek usage 实测：16/27 请求有 cache hit，合计 hit=16000、miss=20475 tokens；没有显式 cache key。
- 源码确认 LangGraph 有 `BaseCheckpointSaver`，但本次 `compile()` 未配置 checkpointer；JSON snapshots 是审计记录，不是可 resume 的 LangGraph checkpoint。
- 未实测并明确不夸大：context compression、context overflow、rate limit、用户取消、checkpoint resume、内置 UI。State/stream event 可供 UI 显示 Planner/current step/tool/replan/END，但这不是本次 UI 实测。

## Pi、Plandex、Marix 对比

|系统|差异|应取/不应取|
|---|---|---|
|Pi|持久 transcript tree 与 provider projection、native tool replay、主动 compaction；默认无工具 sandbox。|取 call-ID replay、投影分离、稳定 schema；不取无权限边界。|
|Plandex|planner/executor 是文本 task/parser/apply-debug；既有实测 146 个请求没有 API tools。|取确定性当前 subtask、build/diff 审计、失败注入；不把文本 parser 当 native function calling。|
|Marix|四态终止、层级 Intent、current-task scoped Completed Calls。|保留 typed calls、完成证据与四终态；可增加 plan/replan state 和 wire→transition 审计。|

## 对 Marix 的借鉴与限制

1. 分开保存 `plan`、`current step`、`past evidence`、terminal result；精确保留 provider call ID、顺序、error 与 tool output replay。
2. 将 iteration/stall/verification guard 放进代码层，且把 guard 的拒绝或重试写进 state/audit。
3. prebuilt ReAct 是 orchestration，不是 sandbox；继续由 Marix enforce workspace、process、network、permission 和 cancellation。
4. 不把工具输出协议误作模型工具选择机制；质量主要来自模型侧 tool schema/description 和真实 observation replay。
5. 此结论限于短任务与 DeepSeek 运行；长上下文、持久 checkpoint/UI、取消/限流需要独立验证。

## Ubuntu 证据与清理

- 根目录：`/root/tool-plan-execute-study/`
- 核心：`report.md`、`evidence-index.md`、`runtime-summary.json`、`wire-requests-full.jsonl`、`state-transitions.jsonl`、`observed-tools.json`。
- Case：`workspaces/<case>/manifest.json`、任务、before/after、tool events、state snapshots、最终工作区。
- 安全：`secret-scan.json` 实际配置 key 精确匹配 **0**；headers 未持久化。
- 清理：`process-cleanup.json` 记录 proxy/harness PID、日志、timeout、退出码；完成后 PID 已不存在。
