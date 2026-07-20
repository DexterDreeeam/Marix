# Tool Library and Tool Calling Specifications Research

## 1. Mainstream Agents' Core Tool List & Categorization

By analyzing agents like OpenHands, Cline, Aider, AutoGPT, CrewAI, LangGraph, and Claude Code, we can categorize the most common and effective tools into several distinct domains. 

### 1.1 File & Workspace Operations
The foundation of coding and local automation agents.
- **Read/View**: `read_file`, `view_directory`, `list_files`. (Often includes line-range support or pagination for large files).
- **Edit/Write**: `write_file` (full overwrite), `edit_file` / `str_replace` (search and replace), `apply_patch` (unified diff format).
- **Search**: `grep_search` (regex in files), `glob_search` (file path pattern matching).
- **File Management**: `create_file`, `delete_file`, `move_file`.

### 1.2 System & Terminal Execution
Crucial for compiling, testing, and system interaction.
- **Synchronous Execution**: `execute_command`, `run_bash_script`.
- **Background Processes**: `start_background_process`, `read_process_output`, `kill_process`. (Essential for running servers, watchers, or long test suites).
- **Sandboxed Execution**: Systems like OpenHands use isolated Docker containers for safe execution.

### 1.3 AST, Code Analysis & IDE Integration
Used heavily by Aider, Continue, and other IDE-native agents.
- **Code Context**: `get_repository_map` (often using `ctags` to list symbols).
- **Language Server Protocol (LSP)**: `find_references`, `go_to_definition`, `get_diagnostics` (lint/compile errors).
- **Editor State**: `read_active_tab`, `get_selected_text`.

### 1.4 Network, Web & Search
For research and fetching external documentation.
- **Web Search**: `web_search` (Tavily, Google, DuckDuckGo) returning snippets and URLs.
- **Page Fetching**: `fetch_url`, `read_webpage` (often converts HTML to Markdown to save tokens).
- **API Interaction**: `send_http_request`.

### 1.5 Browser Automation (Visual/DOM)
Used by Browser-Use, MultiOn, and Claude's Computer Use.
- **Actions**: `click_element`, `type_text`, `scroll`, `navigate`.
- **State Retrieval**: `get_dom_snapshot`, `take_screenshot`.

### 1.6 Agent Orchestration & User Interaction
- **Delegation**: `delegate_task_to_subagent` (used in LangGraph, CrewAI).
- **Human-in-the-loop**: `ask_user_for_clarification`, `request_permission`.
- **Task Management**: `finish_task`, `submit_result`, `report_error`.

### 💡 Suggestions for Marix Tool Expansion
1. **Adopt AST/Symbol tools**: Introduce `ctags` or LSP-based semantic search to help Marix understand large codebases without grep.
2. **Standardize Patch/Edit**: Ensure the `edit` tool supports robust line replacement or unified diffs to avoid full-file rewrites.
3. **Browser Automation Module**: Integrate a Playwright-based or MCP-based browser tool for end-to-end testing of web apps.
4. **Agent Delegation**: Formalize the `task` tool to spawn specialized sub-agents (e.g., `researcher`, `reviewer`).

---

## 2. Mainstream Tool Calling Specifications

### 2.1 OpenAI Function Calling (JSON Schema)
- **Standard**: The industry baseline. Tools are defined using JSON Schema.
- **Format**: Passed in the `tools` array of the API request.
  ```json
  "tools": [{ "type": "function", "function": { "name": "get_weather", "description": "...", "parameters": { "type": "object", "properties": {...} } } }]
  ```
- **Execution**: The model returns `tool_calls` with a JSON payload. The client executes the function and appends a `tool` role message with the `tool_call_id` and result string.

### 2.2 Anthropic Tool Use
- **Standard**: Similar to OpenAI but tightly integrated into Claude's message structure (`tool_use` and `tool_result` content blocks).
- **Format**: Also uses JSON Schema for tool definitions.
- **Beta Tools**: Claude 3.5 provides native concepts like `computer_use`, `bash`, and `text_editor` which bypass standard schema definitions for predefined, highly optimized workflows.

### 2.3 Model Context Protocol (MCP)
- **Concept**: An open standard introduced by Anthropic to decouple tools/data from the agent implementation.
- **Architecture**: A Client-Server model (JSON-RPC over stdio or SSE).
  - **Servers**: Expose `resources`, `prompts`, and `tools`.
  - **Clients**: Agents (like Cline or Claude Desktop) connect to MCP servers.
- **Impact**: Instead of writing custom API integrations, Marix can act as an MCP Client, instantly gaining access to hundreds of community-built MCP Servers (e.g., GitHub MCP, Postgres MCP, Slack MCP).

### 2.4 XML Tags / Custom Text-based Calling
- **Concept**: Before native JSON function calling, or to save tokens, agents instruct the LLM to output specific XML tags.
- **Usage**: Prominent in Cline (`<read_file>path</read_file>`) and Aider's patch formats.
- **Pros/Cons**: Excellent for models that struggle with strict JSON formatting or when tool arguments are very large (like rewriting a 500-line code block), as it avoids JSON string escaping issues.

### 💡 Suggestions for Marix Tool Architecture
1. **MCP Client Integration**: Make Marix fully compatible with the Model Context Protocol. This will infinitely expand Marix's tool library without maintaining custom code for each tool.
2. **Hybrid Output Parsing**: Use OpenAI/Anthropic native function calling for structured, short queries (e.g., grep, search), but use XML/Text-block parsing for code-writing tools to avoid JSON escaping issues on large file edits.