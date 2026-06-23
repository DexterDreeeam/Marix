# Aider Coding Agent Research

## 1. Source, activity, technical stack, and nature

Primary material: the completed external `coding-agent-research` output and the upstream repository/source files listed in this document.

| Item | Details |
|---|---|
| Repository | https://github.com/Aider-AI/aider |
| Main language | Python |
| Stack | Python CLI, LiteLLM, GitPython, tree-sitter/grep-ast, prompt_toolkit, rich, pytest |
| Activity evidence | GitHub API showed a recent push on 2026-05-22; latest release `v0.86.0` on 2025-08-09 |
| License | Apache-2.0 |
| Package | `aider-chat` on PyPI |

Aider is a Git-first CLI coding assistant rather than a general tool-calling autonomous runtime. It is strongest at conversational file editing, repo-aware prompts, diff review, automatic lint/test repair loops, and Git commit/undo workflows.

## 2. Entry points and modules

`pyproject.toml` exposes the CLI as:

```toml
[project.scripts]
aider = "aider.main:main"
```

Core structure:

```text
aider/
  main.py                    # CLI entry
  coders/
    base_coder.py             # Coder base class and main loop
    architect_coder.py        # architect mode
    ask_coder.py              # ask-only mode
    editblock_coder.py        # SEARCH/REPLACE edit format
    wholefile_coder.py        # whole-file replacement
    patch_coder.py / udiff*   # patch and unified diff formats
    search_replace.py         # exact/fuzzy replacement engine
  models.py                  # model config, aliases, LiteLLM metadata
  llm.py                     # lazy LiteLLM wrapper
  repo.py                    # Git integration
  repomap.py                 # tree-sitter repo map
  commands.py                # /add, /drop, /model, and other REPL commands
  run_cmd.py                 # shell command execution
  history.py                 # chat summarization
  linter.py                  # lint/test repair loop
  watch.py                   # file watching
tests/
  basic/
  browser/
  fixtures/
benchmark/
```

## 3. Agent loop

Aider's loop is a REPL plus a selected `Coder` mode, not a generic tool-calling loop.

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
       - continue reflection/fix loop if reflected_message exists
Coder.send_message()
  -> cur_messages += user
  -> format_messages()
  -> check_tokens()
  -> send() through LiteLLM
  -> parse response
  -> apply_updates()
  -> run_shell_commands()
  -> auto_lint / auto_test
  -> auto_commit
```

Key runtime state includes `cur_messages`, `done_messages`, `abs_fnames`, `abs_read_only_fnames`, `repo_map`, `aider_commit_hashes`, `num_reflections`, `max_reflections=3`, `auto_lint`, `auto_test`, and `auto_commits`.

## 4. Planner / executor

The closest planner/executor split is architect mode:

- `ArchitectCoder` inherits from `AskCoder`.
- The architect phase produces a plan or approach.
- After user confirmation, Aider creates an editor coder.
- The editor uses `main_model.editor_model` and `editor_edit_format` to perform actual modifications.
- Shell command suggestions are disabled during the architect phase.

Other modes are edit-format executors rather than planners:

| Edit format | Behavior |
|---|---|
| `ask` | Answer only |
| `architect` | Plan first, then edit |
| `whole` | Output complete files |
| `diff` / `editblock` | SEARCH/REPLACE blocks |
| `udiff` / `patch` | Unified diff / patch style |
| function variants | Some formats use function-call style outputs |

## 5. Tool abstraction

Aider does not use MCP or a unified tool registry as its central abstraction. Its tool-like capabilities are embedded in the coder workflow.

| Capability | Implementation |
|---|---|
| File editing | The model emits an edit format; `Coder` parses and applies it |
| Repo map | Automatically injected context, not a model-callable tool |
| Shell command | Model suggests a command; user confirms execution |
| Lint/test | `Coder` runs after edits and can reflect on failures |
| Git commit | Successful edits can be auto-committed |
| Slash commands | User REPL commands, not LLM tools |

This makes Aider direct and robust for Git-centric editing, but less extensible for third-party tools than MCP-oriented agents.

## 6. Model / provider adaptation

Main paths:

- `aider/models.py`
- `aider/llm.py`

Aider uses LiteLLM, model aliases, `ModelSettings`, per-model edit formats, weak models, editor models, reasoning tags, prompt cache controls, and cached model metadata.

Important `ModelSettings` fields:

| Field | Meaning |
|---|---|
| `edit_format` | Default edit format for a model |
| `weak_model_name` | Lightweight model for summaries and cheap tasks |
| `editor_model_name` | Editing model used by architect mode |
| `use_repo_map` | Whether to enable repo map context |
| `cache_control` | Prompt cache behavior |
| `reasoning_tag` | Reasoning-tag handling |
| `accepts_settings` | Model-specific thinking/reasoning parameters |

## 7. Context construction

`Coder.format_messages()` and `format_chat_chunks()` build the model prompt from system prompt, edit-format instructions, chat files added with `/add`, read-only files, repo map, `done_messages`, `cur_messages`, reminders, examples, images, and URLs.

`repomap.py` is a central differentiator:

- Uses `grep_ast` and `tree_sitter`.
- Extracts tags with relative filename, absolute filename, line, symbol name, and kind.
- Captures definitions and references.
- Uses a `networkx` PageRank-like ranking to choose relevant symbols.
- Caches tags through diskcache/SQLite files such as `.aider.tags.cache.v*`.
- Large repositories can be slow on the first scan.

## 8. File editing and diff

Important paths:

| Path | Role |
|---|---|
| `aider/coders/editblock_coder.py` | SEARCH/REPLACE blocks |
| `aider/coders/search_replace.py` | exact/fuzzy/relative-indent/diff-match-patch replacement |
| `aider/coders/wholefile_coder.py` | whole-file replacement |
| `aider/coders/patch_coder.py` | patch mode |
| `aider/diffs.py` | diff presentation |
| `aider/repo.py` | git diff, commit, and undo |

Editing flow:

```text
LLM response
  -> get_edits()
  -> apply_edits_dry_run()
  -> prepare_to_edit()
  -> apply_edits()
  -> lint/test
  -> commit
```

`search_replace.py` uses exact `original.replace(search, replace)`, diff-match-patch, relative indentation handling, line-level patching, git cherry-pick-assisted strategies, and multiple fallback levels.

## 9. Command execution, sandbox, and permissions

Command execution is in `aider/run_cmd.py`.

| Item | Behavior |
|---|---|
| Unix/macOS interactive | `pexpect.spawn(shell, ["-i", "-c", command])` |
| Fallback | `subprocess.Popen(..., shell=True)` |
| Windows | Detects parent PowerShell/cmd behavior |
| Output | Prints and collects output live |
| Sandbox | None; commands run on the host |
| Permission | User confirmation for model-suggested commands; shell suggestions can be disabled |

Aider's safety boundary is mostly user confirmation plus Git rollback. It does not provide process isolation by default.

## 10. Memory and state persistence

Aider persistence is lightweight:

| Area | Notes |
|---|---|
| Chat history | Recoverable `.aider.chat.history.md` |
| Summarized history | `history.py` generates summaries |
| Repo map cache | `.aider.tags.cache.v*` |
| Model metadata cache | `~/.aider/caches/model_prices_and_context_window.json` |
| Git commits | `aider_commit_hashes` supports `/undo` |
| Config | `.aider.conf.yml`, model settings, model metadata |

There is no OpenHands/OpenCode-style event-sourced session store. The durable audit trail is mostly Git plus optional chat history.

## 11. Event stream, logging, and audit

Aider primarily uses CLI output and optional analytics. `aider/io.py` uses rich and prompt_toolkit, `aider/analytics.py` can record events, `repo.py` creates Git commits for audit, LLM history can be logged, and `show_diffs` can display changes. Events are not modeled as a first-class event bus.

## 12. Testing strategy

The test stack is pytest-based with mock LLMs, temporary Git repositories, basic/browser/scrape/help tests, fixtures, and benchmark suites.

Key areas:

| Path | Focus |
|---|---|
| `tests/basic/test_coder.py` | Coder core behavior |
| `tests/basic/test_commands.py` | slash commands |
| `tests/basic/test_history.py` | summarization |
| `tests/basic/test_sendchat.py` | message role sanity |
| `tests/basic/test_io.py` | IO and completion |
| `benchmark/` | editing quality and performance benchmarks |

## 13. Plugins, MCP, and extension model

Aider has no native MCP and no general plugin system. Extension is mostly source-level:

| Extension style | Notes |
|---|---|
| New `Coder` | Inherit `Coder` and implement `get_edits()` / `apply_edits()` |
| New edit format | Add implementation under `aider/coders/` and register it |
| New command | Add methods in `Commands` |
| Custom linter | Use `Linter.set_linter()` |
| Custom model | Use `.aider.model.settings.yml` or `.aider.model.metadata.json` |

## 14. Core source file paths

Recommended paths for architecture review:

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

## 15. Lessons for `{{proj}}`

1. Make diff, commit, and undo first-class if the product edits source code.
2. Choose edit formats per model instead of assuming one patch protocol works for every provider.
3. Use tree-sitter repo maps for cheap large-repository context.
4. Separate planning and editing models for high-risk refactors.
5. Add multiple search/replace fallbacks to tolerate imperfect model formatting.
6. Keep a simple CLI path available even if the full product has richer runtimes.
7. Use Git as an audit and rollback layer, but do not treat it as a full event store.

## 16. Risks and anti-patterns

| Risk / anti-pattern | Why it matters |
|---|---|
| No sandbox | Shell commands run with host authority |
| Non-MCP architecture | Third-party tool ecosystem integration is harder |
| Format fragility | SEARCH/REPLACE blocks depend on strict model compliance |
| Large file token cost | Chat files are often injected as full text |
| Weak replay model | Audit depends on Git and logs, not structured events |
| Limited concurrency | Not designed for multiple agents or sessions editing together |
| Auto-commit overreach | Automatic commits are convenient but can hide unreviewed changes if defaults are too aggressive |
