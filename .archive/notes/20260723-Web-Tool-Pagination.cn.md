# Web Fetch/Search 分页改造

## 目的

本文记录 `web_fetch` 和 `web_search` 两个原生工具
（`src/tool/native/web/web_fetch.rs`、`src/tool/native/web/web_search.rs`）
新增分页能力的设计方案与背后的架构考量，供后续参考和调优。

## 问题

`web_fetch` 之前会返回抓取并清洗后的整页内容，没有任何大小上限。这直接导致了一次真实的生产故障：
一次 E2E smoke test 中，抓取一个 849KB 的 Rust release-notes 页面把模型的上下文窗口撑爆
（`model_context_window_exceeded`），因为整页文本被原样拼进了下一轮 relay 的 Prompt。

`web_search` 只有一个扁平的 `max_results`（1-10），没有办法拿到第一页之后的结果。

## 影响设计的架构约束

1. **每次工具调用都是全新的、无状态的独立进程。**
   `src/tool/tool_main.rs` 只读一次 stdin 的 JSON payload、打印一次 JSON 结果就退出。
   `src/host/executor/tool.rs` 启动每个工具时只给了 stdin/stdout/stderr 三个管道——不传
   task id、session id 或任何其他关联上下文。一次调用没有任何办法记住上次调用发生过什么，
   除非落盘。
2. **Marix 内部已经有一套分页约定。**
   `read_process_output`（`src/tool/native/process/{read_process_output,output,
   process_registry}.rs`）已经在用：
   - 输入：`offset`（位置）+ `max_bytes`（单页大小）
   - 输出：`next_offset`（位置）+ `truncated`（布尔值）
   - 纯结构化 JSON 字段——不在内容文本里嵌入任何自然语言提示。
   这次改动沿用了这套既有约定，而不是另起一套。
3. **`clean_html()` 是整篇文档级别的算法。** 它靠一个贯穿全文的栈来匹配 `<a>` 标签，
   没法安全地处理任意一段 HTML 片段。这意味着必须先抓取并清洗完整页面，才能对最终的
   清洗结果做切片——分页只能发生在"已清洗完的文本"这一层。

## 外部调研摘要

专门委派了一次 `researcher-of-agents` 调研其他 agent CLI 的做法：

- **输入形态**：offset/length（`start_index`/`max_length`）是复用最广的约定
  （MCP 官方 `fetch` server、OpenHands SDK、以及本环境自己的 `web_fetch` 工具都是这个形态）。
  页码式、不透明 cursor 式的做法也存在（Windsurf，来自泄露 schema），但很少见且缺乏文档。
  没有任何被调研的 agent 在同一次 fetch 调用里同时支持 offset 分页和"关键词定位"（query）参数。
- **输出形态**：所有被调研的 agent 都是靠在内容文本里嵌一句自然语言提示（比如"call again
  with start_index=X"）来告知截断，而不是结构化字段。Marix 这次刻意选择偏离这个主流做法，
  转而对齐自己已有的 `read_process_output` 约定。
- **是否要缓存**：不缓存、每次重新抓取重新切片是绝大多数系统的默认做法，直接读了 5 个以上
  独立代码库源码确认（MCP 官方 server、Cline、Continue，以及本环境自己的工具实质上也是如此）。
  真正做到"只抓一次、缓存后分页读"的设计只在 Anthropic API 的 `web_fetch`（官方确认存在，
  但官方文档明确说这是不透明、可能随时改变的内部细节，不是稳定契约）和 Claude Code CLI
  （仅有社区推测，非官方）里见到过。没有任何系统公开记录过抓取内容的缓存 key/TTL/淘汰策略。
- **`web_search` 分页**：现代基于语义检索的搜索工具（Tavily、Exa、Perplexity、Anthropic 和
  OpenAI 各自的托管搜索）大多完全没有 offset/分页机制——面对"我需要更多结果"，主流答案是
  "换一个更具体的 query"，而不是"翻到第 2 页"。只有传统倒排索引引擎（Brave）支持真正的
  offset/页码分页。DuckDuckGo 和 Yahoo 的经典 HTML 搜索端点——正是 Marix 现在抓取的这两个
  引擎——经过一次实时 web search 验证，确认支持"跳过 N 条结果"式的原始参数（分别是 `s=`
  和 `b=`），所以给 `web_search` 加 offset 分页对 Marix 现有的引擎是直接可行的。

**决策：不做缓存。** 综合考虑无状态子进程的硬约束（要做缓存就得像
`process_registry.rs` 那样上一整套磁盘锁+TTL+清理机制）、`clean_html()`
没法处理半截 HTML（缓存只能省网络往返，省不了重新清洗的 CPU 开销，除非专门缓存
"清洗后的文本"——这是个从未被要求过、且会显著增加复杂度的设计）、以及"重新抓取重新切片"
本身就是业界主流默认做法这三点，两个工具的分页都实现为**纯粹的、无状态的、每次调用都
重新抓取重新切片**，不做任何磁盘缓存，不做任何跨调用的状态持久化。

## 设计：`web_fetch`

输入 schema（新增两个可选字段，`url` 不变、仍为必填）：

```json
{
  "type": "object",
  "properties": {
    "url": {"type": "string"},
    "max_length": {"type": "integer", "minimum": 1, "maximum": 15000},
    "start_index": {"type": "integer", "minimum": 0}
  },
  "required": ["url"],
  "additionalProperties": false
}
```

- `max_length`：可选，默认 `5000`，上限 `15000` 字符。这个上限相对保守，是因为
  Marix 接的是 DeepSeek/GLM 后端（相比 Claude 级别的模型，实际可用上下文预算小得多），
  而且分页后的 `content` 还会流经新加的 tool-call-summarize relay
  （`workflow_call_summary`，参见 tag `marix_tag_20260723_010223_tool_call_summarize_relay`）——
  这个 relay 自己的 prompt 会把原始的、未经总结的整段页面文本嵌进去，所以它同样会受到
  这个功能本来就是为了解决的那种上下文预算风险；设定这个上限也顺带保证了那个 relay
  自身的输入是有界的，不管底层页面本身有多大。
- `start_index`：可选，默认 `0`。

输出形态：

```json
{
  "content": "...",
  "start_index": 0,
  "next_start_index": 5000,
  "truncated": true,
  "total_length": 849213
}
```

`truncated` 为 `false` 时 `next_start_index` 为 `null`/不存在。
`total_length` 在 Marix 的架构下是零额外成本的，因为 `curl` 在做任何切片之前已经把整个
响应体下载完了（不像流式抓取那样,算总长度本身就有代价）。

实现细节（`fn paginate`，私有函数，位于 `web_fetch.rs`）：

- 现有的 curl → `looks_like_html` → `clean_html` 管线完全没有改动，依然总是对整页内容运行。
- 切片按**字符索引**而非裸字节偏移操作，避免切断多字节 UTF-8 字符
  （`content.char_indices()` 先把字符位置映射到字节偏移，再对 `String` 切片）。
- `start_index > total_length` 是一个错误。`start_index == total_length` 是合法的
  "已读到末尾"情况，返回一个空内容的页面，`truncated: false`。这与
  `src/tool/native/process/output.rs` 里 `read_process_output` 已有的
  `OutputSnapshot::from_path` 语义完全一致，保持全仓库风格统一。

`content` 里不嵌入任何"请用 X 重新调用"式的自然语言提示——契约纯粹是结构化字段，
对齐 `read_process_output`。

## 设计：`web_search`

输入 schema（新增可选 `offset`，`query`/`max_results` 不变）：

```json
{
  "type": "object",
  "properties": {
    "query": {"type": "string", "minLength": 1},
    "max_results": {"type": "integer", "minimum": 1, "maximum": 10},
    "offset": {"type": "integer", "minimum": 0}
  },
  "required": ["query"],
  "additionalProperties": false
}
```

`offset` 是"要跳过的原始结果条数"（对齐 Marix 自己 `read_process_output` 的字节/计数式
offset 约定），而不是页码式的乘数（Brave 自己的 `offset` 语义），因为它能 1:1 直接映射到
DuckDuckGo 和 Yahoo 各自查询参数的真实工作方式。

输出形态：

```json
{
  "results": [...],
  "offset": 0,
  "next_offset": 10,
  "has_more": true
}
```

`has_more` 是一个尽力而为的启发式判断（`results.len() >= max_results`）——HTML 抓取
拿不到引擎本身可靠的总结果数信号，所以没法做到精确。

实现细节（`web_search.rs`）：

- `search(query, max_results, offset)` 只把 `offset` 传给
  `search_duckduckgo`/`search_yahoo`。
- `search_duckduckgo`：`offset > 0` 时在请求 URL 后加 `&s={offset}`
  （DuckDuckGo 原生的"跳过结果数"参数）。`offset == 0` 时完全不加这个参数，
  保证默认调用的请求 URL 和改动前字节级一致。
- `search_yahoo`：`offset > 0` 时加 `&b={offset + 1}`（Yahoo 经典的 `b=` 参数是
  1-based 的"从第 N 条结果开始"；`+1` 是把 Marix 0-based 的 `offset` 换算成 Yahoo
  的约定）。同样在 offset 为 0 时不加参数。
- `search_wikipedia` 刻意保持不变，完全不接收 `offset`。Wikipedia 的 `opensearch`
  API 是相关性/前缀匹配搜索，没有"结果条数分页"这个概念；不管调用方要求的 `offset`
  是多少，它始终从头开始搜索。这是这个优先级最低的兜底引擎一个明确记录、可接受的
  限制，不是遗漏。
- `parse_duckduckgo_results`/`parse_yahoo_results` 的解析逻辑没有变——分页只改变
  "请求哪一页"，不改变"怎么解析一页的 HTML"。

### 已知限制：分页序列中的引擎一致性

现有的兜底链路（DuckDuckGo → Yahoo → Wikipedia，结果为空时依次尝试）没有变。由于每次
`web_search` 调用都是无状态的，只要 DuckDuckGo 在每个 offset 都还能返回非空结果，
一串 offset 递增的分页调用就会一直拿到 DuckDuckGo 的结果（兜底顺序不受 `offset` 影响）。
如果 DuckDuckGo 的结果恰好在某个 offset 用尽，后续页面会退化成 Yahoo 自己从头开始的分页——
这在"结果自然耗尽"的边界上是合理的降级,不是分页链路坏掉了,但模型没有任何显式信号能
知道底层引擎在分页序列中途换了。这是兜底设计本身就有的既有特性，本次改动接受这一点，
不尝试解决它。

## 改动文件

- `src/tool/native/web/web_fetch.rs`：新增 `DEFAULT_MAX_LENGTH`/`MAX_MAX_LENGTH`
  常量，扩展了 `INPUT_SCHEMA`/`DESCRIPTION`，新增 `struct Page` + `fn paginate`，
  `invoke()` 解析两个新的可选字段并调用 `paginate`，不再返回无界内容。
- `src/tool/native/web/web_search.rs`：扩展了 `INPUT_SCHEMA`/`DESCRIPTION`，
  `invoke()` 解析 `offset` 并在响应里加上 `offset`/`next_offset`/`has_more`，
  `search`/`search_duckduckgo`/`search_yahoo` 的函数签名都加了 `offset: usize`
  参数，`search_wikipedia` 保持不变。
- `ToolProgram`、`ToolPreview` 或任何其他 protocol 层的 Rust 类型都没有改动——
  两个工具的 `invoke(&self, call: &str) -> String` 签名完全不变，只改了 JSON
  schema 字符串常量和 `invoke()` 内部拼装的 JSON 内容。这也是这次改动直接走
  `feature-implement` 风格实现、不需要先过一遍 `feature-design` 的原因。

## 本次明确不做的事

- 两个工具都不做磁盘缓存、临时文件，或任何形式的跨调用状态持久化。
- `web_fetch` 不加"关键词定位"（grep-in-page）式的 query 参数——没有任何被调研的
  agent 在同一次调用里把这个和 offset 分页放在一起，本次也没人要求这个功能。
- 不修复上面提到的"分页序列中引擎一致性"这个已知限制。
- `web_fetch.rs` 里的 `clean_html`/`looks_like_html`/标签解析辅助函数，以及
  `web_search.rs` 里的 `parse_duckduckgo_results`/`parse_yahoo_results`/
  `search_wikipedia` 的解析逻辑，全部没有改动。

## 已完成的验证

- `cargo check -p marix-tool`：干净（只有 `.github/skills/project-build/SKILL.md`
  里记录过的、与本次改动无关的既有多 bin target 警告）。
- `cargo clippy -p marix-tool --lib --all-features`：没有新增警告（通过对比
  `git stash` 后的基线运行结果确认）。
- 对 `marix_tool_web_fetch` 和 `marix_tool_web_search` 两个二进制分别做了手动
  `--preview` 和 stdin 验证，一次只编译运行一个（避免这个 crate 15 个共享源码的
  `[[bin]]` target 之间已知的 Cargo feature unification 坍缩风险）：确认了连续
  分页、`start_index == total_length` 边界行为、越界错误、`max_length`/
  `start_index`/`offset` 的非法值校验错误，以及 `offset` 确实能让 DuckDuckGo
  返回真实不同的结果。
