# Aider Coding Agent 研究

## 1. 来源、活跃度、技术栈与性质

| 项目 | 内容 |
|---|---|
| 仓库 | https://github.com/Aider-AI/aider |
| 主要语言 | Python |
| 技术栈 | Python CLI、LiteLLM、GitPython、tree-sitter/grep-ast、prompt_toolkit/rich、pytest |
| 近期活跃证据 | GitHub API 显示最近 push：2026-05-22；最新 release：`v0.86.0`，2025-08-09 |
| 许可 | Apache-2.0 |
| PyPI 包 | `aider-chat` |

主要来源：

- https://github.com/Aider-AI/aider
- https://raw.githubusercontent.com/Aider-AI/aider/main/pyproject.toml
- https://raw.githubusercontent.com/Aider-AI/aider/main/aider/main.py
- https://raw.githubusercontent.com/Aider-AI/aider/main/aider/coders/base_coder.py
- https://raw.githubusercontent.com/Aider-AI/aider/main/aider/coders/architect_coder.py
- https://raw.githubusercontent.com/Aider-AI/aider/main/aider/models.py
- https://raw.githubusercontent.com/Aider-AI/aider/main/aider/repomap.py
- https://raw.githubusercontent.com/Aider-AI/aider/main/aider/run_cmd.py
- https://raw.githubusercontent.com/Aider-AI/aider/main/aider/coders/search_replace.py

## 2. 入口与包结构

`pyproject.toml`：

```toml
[project.scripts]
aider = "aider.main:main"
```

核心结构：

```text
aider/
  main.py                    # CLI 入口
  coders/
    base_coder.py             # Coder 基类和主循环
    architect_coder.py        # architect 模式
    ask_coder.py              # 只问不改
    editblock_coder.py        # SEARCH/REPLACE edit format
    wholefile_coder.py        # whole file replace
    patch_coder.py / udiff*   # patch/unified diff 格式
    search_replace.py         # 模糊/精确替换逻辑
  models.py                  # 模型配置、alias、LiteLLM metadata
  llm.py                     # LiteLLM lazy wrapper
  repo.py                    # Git 集成
  repomap.py                 # tree-sitter repo map
  commands.py                # /add /drop /model 等交互命令
  run_cmd.py                 # shell command 执行
  history.py                 # chat summary
  linter.py                  # lint/test 修复循环
  watch.py                   # 文件监听
tests/
  basic/
  browser/
  fixtures/
benchmark/
```

## 3. Agent loop

Aider 的 loop 是 REPL + Coder 模式，而非通用 tool-calling agent。

核心在 `aider/coders/base_coder.py`：

```text
Coder.run()
  -> while True:
       - get_input()
       - run_one(user_message)
Coder.run_one()
  -> init_before_message()
  -> preproc_user_input()
  -> while message:
       - send_message(message)
       - 如果 reflected_message 存在，继续反思/修复
Coder.send_message()
  -> cur_messages += user
  -> format_messages()
  -> check_tokens()
  -> send() 调 LiteLLM
  -> parse response
  -> apply_updates()
  -> run_shell_commands()
  -> auto_lint / auto_test
  -> auto_commit
```

显著字段：

- `cur_messages`
- `done_messages`
- `abs_fnames`
- `abs_read_only_fnames`
- `repo_map`
- `aider_commit_hashes`
- `num_reflections`
- `max_reflections=3`
- `auto_lint`
- `auto_test`
- `auto_commits`

## 4. Planner / executor

Aider 最接近 planner/executor 的是 `architect` 模式：

- `ArchitectCoder` 继承 `AskCoder`；
- 先生成计划/方案；
- 用户确认后创建 editor coder；
- editor coder 使用 `main_model.editor_model` 和 `editor_edit_format` 执行实际修改；
- architect 阶段禁用 shell command 建议。

路径：

- `aider/coders/architect_coder.py`

其他模式不是 planner/executor，而是 edit format executor：

| edit_format | 说明 |
|---|---|
| `ask` | 只回答 |
| `architect` | 先计划再编辑 |
| `whole` | 输出完整文件 |
| `diff` / `editblock` | SEARCH/REPLACE |
| `udiff` / `patch` | unified diff / patch 风格 |
| function variants | 部分格式支持 function-call 风格 |

## 5. Tool abstraction

Aider 不采用 MCP 或统一 Tool registry 作为核心抽象。它的“工具”更接近：

| 类型 | 实现方式 |
|---|---|
| 文件编辑 | LLM 按 edit format 输出文本，Coder 解析 |
| repo map | 自动注入上下文，不是模型调用工具 |
| shell command | LLM 生成命令建议，用户确认执行 |
| lint/test | 编辑后由 Coder 自动运行 |
| Git commit | 编辑成功后自动提交 |
| `/commands` | 用户 REPL 命令，不是 LLM tool |

这使 Aider 非常直接、Git-first，但扩展第三方工具不如 MCP agent 灵活。

## 6. 模型/provider 适配

路径：

- `aider/models.py`
- `aider/llm.py`

核心：

- LiteLLM；
- model aliases；
- `ModelSettings`；
- per-model edit format；
- weak model；
- editor model；
- reasoning tag；
- prompt cache；
- model info cache。

`models.py` 中 `ModelSettings` 包括：

| 字段 | 说明 |
|---|---|
| `edit_format` | 默认编辑格式 |
| `weak_model_name` | 摘要等轻量任务 |
| `editor_model_name` | architect 执行阶段 |
| `use_repo_map` | 是否启用 repo map |
| `cache_control` | prompt cache |
| `reasoning_tag` | 推理标签处理 |
| `accepts_settings` | thinking/reasoning 等参数 |

## 7. 上下文构建

Aider 上下文由 `Coder.format_messages()` / `format_chat_chunks()` 构造，主要包括：

| 来源 | 说明 |
|---|---|
| system prompt | edit format 说明、行为规则 |
| chat files | 用户 `/add` 到 chat 的文件全文 |
| read-only files | 只读上下文 |
| repo map | tree-sitter 符号图 |
| chat history | `done_messages` + `cur_messages` |
| reminders/examples | per-model/prompt 注入 |
| images/URLs | 支持 image 和 URL 处理 |

`repomap.py`：

- 使用 `grep_ast`、`tree_sitter`；
- `Tag = rel_fname/fname/line/name/kind`；
- 提取 definitions/references；
- 使用 `networkx` PageRank 风格相关性排序；
- 用 diskcache/SQLite 缓存 `.aider.tags.cache.v*`；
- 大仓库首次扫描可能较慢。

## 8. 文件编辑 / diff

主要实现：

| 路径 | 说明 |
|---|---|
| `aider/coders/editblock_coder.py` | SEARCH/REPLACE |
| `aider/coders/search_replace.py` | 精确/模糊/relative indent/diff_match_patch |
| `aider/coders/wholefile_coder.py` | 整文件替换 |
| `aider/coders/patch_coder.py` | patch |
| `aider/diffs.py` | diff 展示 |
| `aider/repo.py` | git diff/commit/undo |

编辑流程：

```text
LLM response
  -> get_edits()
  -> apply_edits_dry_run()
  -> prepare_to_edit()
  -> apply_edits()
  -> lint/test
  -> commit
```

`search_replace.py` 的特点：

- 精确 `original.replace(search, replace)`；
- diff-match-patch；
- relative indentation 处理缩进变化；
- line-level patch；
- git cherry-pick 辅助策略；
- 多级 fallback。

## 9. 命令执行 / 沙箱 / 权限

路径：

- `aider/run_cmd.py`

行为：

| 项 | 说明 |
|---|---|
| Unix/macOS interactive | `pexpect.spawn(shell, ["-i", "-c", command])` |
| fallback | `subprocess.Popen(..., shell=True)` |
| Windows | 检测 parent PowerShell/cmd |
| 输出 | 实时打印并收集 |
| 沙箱 | 无沙箱，直接宿主执行 |
| 权限 | 用户确认命令建议；可禁用 `suggest_shell_commands` |

Aider 的安全边界主要是用户确认和 Git 回滚，不是进程隔离。

## 10. 记忆 / 状态持久化

Aider 的状态持久化偏轻量：

| 类型 | 说明 |
|---|---|
| chat history | 可恢复 `.aider.chat.history.md` |
| summarized history | `history.py` 自动摘要 |
| repo map cache | `.aider.tags.cache.v*` |
| model metadata cache | `~/.aider/caches/model_prices_and_context_window.json` |
| git commits | `aider_commit_hashes` 支持 `/undo` |
| config | `.aider.conf.yml`、model settings/metadata |

它没有类似 OpenHands/OpenCode 的 event-sourcing session store。

## 11. 事件流 / 日志 / 审计

Aider 主要是 CLI 输出 + optional analytics：

- `aider/io.py` 用 rich/prompt_toolkit 输出；
- `aider/analytics.py` 可记录事件；
- `repo.py` 通过 Git commit 形成审计；
- LLM history 可 log；
- `show_diffs` 可显示变更。

事件不是一等结构化 event bus。

## 12. 测试策略

测试栈：

- pytest；
- mock LLM；
- 临时 git repo；
- basic/browser/scrape/help tests；
- benchmark 目录。

关键测试方向：

| 路径 | 说明 |
|---|---|
| `tests/basic/test_coder.py` | Coder 核心 |
| `tests/basic/test_commands.py` | slash commands |
| `tests/basic/test_history.py` | 摘要 |
| `tests/basic/test_sendchat.py` | message role sanity |
| `tests/basic/test_io.py` | IO/补全 |
| `benchmark/` | 代码编辑效果/性能基准 |

## 13. 插件 / MCP / 扩展机制

Aider 没有原生 MCP，也没有通用插件系统。扩展方式主要是源码级：

| 扩展方式 | 说明 |
|---|---|
| 新 Coder | 继承 `Coder`，实现 `get_edits/apply_edits` |
| 新 edit format | 加入 `aider/coders/__init__.py` |
| 新 command | 在 `Commands` 中添加方法 |
| 自定义 linter | `Linter.set_linter()` |
| 自定义模型 | `.aider.model.settings.yml` / `.aider.model.metadata.json` |

## 14. 对 `{{proj}}` 的借鉴

以下经验可作为 `{{proj}}` 设计 agent runtime、工具边界和工程治理时的参考：

1. **Git-first UX**：diff、commit、undo 是核心流程，不是附属功能。
2. **edit format 策略化**：不同模型使用不同 edit format，提升稳定性。
3. **RepoMap**：tree-sitter 低成本构建大型仓库符号上下文。
4. **Architect mode**：规划模型与编辑模型分离，非常适合复杂重构。
5. **多级 search/replace fallback**：比简单 patch apply 更抗模型格式误差。
6. **CLI 简洁性**：单进程、低服务依赖，用户上手快。

## 15. 核心源码文件路径

建议优先阅读以下源码路径来复核架构判断：

- `aider/main.py`
- `aider/coders/base_coder.py`
- `aider/coders/architect_coder.py`
- `aider/coders/editblock_coder.py`
- `aider/coders/search_replace.py`
- `aider/coders/wholefile_coder.py`
- `aider/coders/patch_coder.py`
- `aider/models.py`
- `aider/repomap.py`
- `aider/run_cmd.py`
- `aider/repo.py`

## 16. 风险 / 反模式

| 风险 | 说明 |
|---|---|
| 无沙箱 | shell 命令宿主执行 |
| 非 MCP 化 | 难以接入第三方工具生态 |
| 格式脆弱 | SEARCH/REPLACE 要求模型遵守格式 |
| 大文件上下文成本 | chat files 全文注入可能爆 token |
| 状态不可重放 | 没有 event log，审计依赖 Git/日志 |
| 并发弱 | 不适合多 agent、多会话协同编辑 |

---
