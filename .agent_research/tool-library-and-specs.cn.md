# 工具库与工具调用规范调研报告

## 1. 主流 Agent 核心工具列表与分类

通过分析 OpenHands, Cline, Aider, AutoGPT, CrewAI, LangGraph 以及 Claude Code 等项目，我们可以将目前 Agent 最常用且最核心的工具归纳为以下几大类。

### 1.1 文件与工作区操作类 (File & Workspace Operations)
这是代码和本地自动化 Agent 的基石。
- **读取/查看**：`read_file` (读取文件), `view_directory` (查看目录), `list_files` (列出文件)。通常包含行号范围支持或长文件分页。
- **编辑/写入**：`write_file` (全量覆盖), `edit_file` / `str_replace` (局部搜索替换), `apply_patch` (应用 Unified Diff 补丁)。
- **搜索**：`grep_search` (正则表达式内容搜索), `glob_search` (文件路径模式匹配)。
- **管理**：`create_file`, `delete_file`, `move_file`。

### 1.2 系统与终端执行类 (System & Terminal Execution)
用于编译、测试、运行脚本及系统级交互。
- **同步执行**：`execute_command`, `run_bash_script` (运行完毕返回结果)。
- **后台进程**：`start_background_process`, `read_process_output`, `kill_process`。(对于运行 Web 服务器、监听器或耗时较长的测试用例至关重要)。
- **沙盒执行**：如 OpenHands 采用隔离的 Docker 容器，确保执行安全。

### 1.3 AST与代码分析/IDE 集成类 (AST & IDE Integration)
Aider, Continue 等与 IDE 紧密结合的 Agent 大量使用此类工具。
- **代码上下文**：`get_repository_map` (通常底层利用 `ctags` 提取全局符号树)。
- **LSP 语义化**：`find_references` (查找引用), `go_to_definition` (跳转定义), `get_diagnostics` (获取编译或 Lint 报错)。
- **编辑器状态**：`read_active_tab` (读取当前活跃标签页), `get_selected_text` (获取选中代码)。

### 1.4 网络与搜索类 (Network, Web & Search)
用于查阅最新文档、报错方案或处理外部数据。
- **网页搜索**：`web_search` (如 Tavily, Google, DuckDuckGo)，返回带链接的摘要。
- **网页抓取**：`fetch_url`, `read_webpage` (通常会自动将 HTML 转为 Markdown 以节省 Token)。
- **API 请求**：`send_http_request`。

### 1.5 浏览器自动化类 (Browser Automation)
主要在 Browser-Use, MultiOn 以及 Claude 的 Computer Use 中体现。
- **视觉/行为**：`click_element` (点击), `type_text` (输入), `scroll` (滚动), `navigate` (导航)。
- **状态获取**：`get_dom_snapshot` (获取简化版 DOM 树), `take_screenshot` (截图并结合视觉模型)。

### 1.6 Agent 编排与元操作类 (Agent Orchestration & Meta)
- **委派与多智能体**：`delegate_task_to_subagent` (LangGraph, CrewAI 常用)。
- **人机交互**：`ask_user_for_clarification` (向用户提问), `request_permission` (请求敏感操作权限)。
- **状态流转**：`finish_task` (标记任务完成), `submit_result` (提交结果), `report_error` (报错并让渡控制权)。

### 💡 对 Marix 的工具扩展建议：
1. **引入 AST/符号分析工具**：除了 grep 之外，建议引入基于 ctags 或 LSP 的工具，帮助模型更高效地理解大型代码库的全局结构。
2. **细化补丁应用机制**：增强 `edit` 工具对大文件局部更新的鲁棒性，例如支持标准的 diff 格式，避免大文件全量覆写。
3. **增加浏览器 UI 测试模块**：集成 Playwright 或类似工具，使 Marix 具备全栈项目的端到端跑通并验证的能力。
4. **子智能体委派规范化**：扩展 `task` 工具的功能，支持唤起专职的 Reviewer 或 Researcher 智能体处理支线任务。

---

## 2. 主流工具调用规范 (Tool Calling Specifications)

### 2.1 OpenAI Function Calling (基于 JSON Schema)
- **行业基准**：目前大多数模型支持的底层规范。所有工具以 JSON Schema 的格式定义。
- **调用逻辑**：在请求时传入 `tools` 数组。当模型决定调用时，返回带有 `tool_calls` 结构的消息；客户端执行后，将结果封装为 `role: "tool"` 且附带 `tool_call_id` 的消息返回给模型。
- **优缺点**：结构化良好，但在生成极长文本（如覆写几百行代码）时，JSON 的字符串转义极易引发 JSON 格式错误。

### 2.2 Anthropic Tool Use 规范
- **特性**：与 OpenAI 类似，同样接受 JSON Schema 定义，但在消息体中体现为特定的内容块：`tool_use` 和客户端返回的 `tool_result`。
- **原生预设工具**：Claude 3.5 引入了内置的 `computer_use`, `bash`, `text_editor`。这些工具不再需要开发者手写复杂的 Schema，而是由模型原生级优化和支持。

### 2.3 MCP (Model Context Protocol) 架构
- **核心理念**：Anthropic 推出的一种开源协议，旨在实现“模型”与“数据源/工具”之间的解耦。
- **架构机制**：采用 Client-Server 架构（底层为基于 stdio 或 SSE 的 JSON-RPC 2.0）。
  - **Server**：暴露资源 (`resources`)、提示词模板 (`prompts`) 和工具 (`tools`)。
  - **Client**：Agent（如 Cline, Claude Desktop）连接到这些 Server 进行交互。
- **对 Marix 的意义**：Marix 不应再硬编码所有的三方 API（如 GitHub、数据库、Slack），而应该**实现一个标准的 MCP Client**。这样可以直接接入社区海量的 MCP Servers，瞬间扩展出百种能力。

### 2.4 XML 标签 / 自定义文本解析 (Custom Text-based Calling)
- **核心理念**：不依赖模型原生的 JSON Function Calling 机制，而是要求模型在纯文本中输出特定的 XML 标签格式。
- **应用案例**：Cline 极度依赖此模式（如 `<read_file>path</read_file>`），Aider 在许多模型上也使用特殊标记的文本块。
- **优势**：完美避开了 JSON 序列化时对引号、换行符的转义问题，特别适合“长代码生成”、“文件替换”等场景。同时，某些对 Function Calling 微调不佳的开源模型，反而能很好地遵循 XML 标签。

### 💡 对 Marix 底层架构的建议：
1. **全面拥抱 MCP 协议**：将 Marix 现有的外部集成逐步改造成连接外部 MCP Server，让工具库无限可扩展且易于社区贡献。
2. **混合调用策略 (Hybrid Strategy)**：针对结构化的短查询（如搜索文件、查天气），使用 OpenAI/Anthropic 原生的 JSON Function Calling；对于**大量代码写入和文件修改**（如 `edit` 或 `create`），考虑采用基于 XML 标签解析的纯文本机制，以大幅提升大段代码输出的稳定性并减少 Token 消耗。