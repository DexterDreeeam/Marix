# Web Fetch/Search Pagination

## Purpose

This note documents the pagination design added to the `web_fetch` and
`web_search` native tools (`src/tool/native/web/web_fetch.rs`,
`src/tool/native/web/web_search.rs`), and the architectural reasoning behind
it, for future reference and further tuning.

## Problem

`web_fetch` returned the entire fetched and cleaned page with no size bound.
This caused a real production failure: an 849KB Rust release-notes page blew
out a model's context window (`model_context_window_exceeded`) during an E2E
smoke test, because the full page text was embedded directly into the next
relay Prompt.

`web_search` only exposed a flat `max_results` (1-10) with no way to reach
results beyond the first page.

## Architectural constraints that shaped the design

1. **Every tool invocation is a brand-new, stateless OS process.**
   `src/tool/tool_main.rs` reads one JSON payload from stdin, prints one JSON
   result, and exits. `src/host/executor/tool.rs` spawns each tool with only
   stdin/stdout/stderr pipes — no task id, session id, or any other
   correlating context is passed in. There is no way for one invocation to
   remember anything from a previous invocation without writing to disk.
2. **Marix already has an internal pagination convention.**
   `read_process_output` (`src/tool/native/process/{read_process_output,
   output,process_registry}.rs`) already uses:
   - input: `offset` (position) + `max_bytes` (page size),
   - output: `next_offset` (position) + `truncated` (bool),
   - purely structured JSON fields — no natural-language hint embedded in the
     content itself.
   This convention was reused for consistency rather than inventing a new one.
3. **`clean_html()` is a whole-document algorithm.** It matches anchor tags
   through a global stack across the entire page and cannot safely run on an
   arbitrary HTML fragment. This means the full page must always be fetched
   and fully cleaned before any slicing — pagination can only happen on the
   final, already-cleaned text.

## External research summary

A dedicated research pass (`researcher-of-agents`) compared how other
agent CLIs handle this:

- **Input shape**: offset/length (`start_index`/`max_length`) is the most
  replicated convention (MCP reference `fetch` server, OpenHands SDK, and
  this environment's own `web_fetch` tool all use this shape). Page-number
  and opaque-cursor styles exist (Windsurf, leaked schemas) but are rare and
  under-documented. No agent studied combines offset pagination with a
  query/substring-locate parameter on the same fetch call.
- **Output shape**: every agent studied signals truncation via an embedded
  natural-language sentence in the content (e.g. "call again with
  start_index=X"), not a structured field. Marix deliberately diverges here
  to stay consistent with its own `read_process_output` convention instead.
- **Caching**: re-fetch-and-reslice with no cache is the majority default,
  confirmed by direct source reads of 5+ independent codebases (MCP
  reference server, Cline, Continue, and effectively this environment's own
  tool). True fetch-once/cache-then-page designs exist only in Anthropic's
  API `web_fetch` (officially confirmed to exist, but explicitly documented
  as an undisclosed, revisable internal detail, not a stable contract) and
  Claude Code CLI (community-reported only, unofficial). No system
  documents a transparent cache key/TTL/eviction policy for fetched content.
- **`web_search` pagination**: modern semantic-search-based tools (Tavily,
  Exa, Perplexity, Anthropic's and OpenAI's hosted search) mostly have no
  offset/page mechanism at all — the common answer to "I need more" is
  "issue a more specific query," not "page 2." Only traditional
  inverted-index engines (Brave) support real offset/page pagination.
  DuckDuckGo's and Yahoo's classic HTML search endpoints — the two engines
  Marix already scrapes — were confirmed (via a live web search) to support
  raw result-count-skip parameters (`s=` and `b=` respectively), so adding
  offset pagination to `web_search` is directly feasible against Marix's
  existing engines.

**Decision: no cache.** Given the stateless-process constraint (any cache
would require the same disk-based lock/TTL/cleanup machinery as
`process_registry.rs`), the fact that `clean_html()` cannot process partial
HTML slices (so caching would only save the network round-trip, not the
re-cleaning cost, unless it cached the cleaned text specifically — a design
that was never asked for and adds real complexity), and the fact that
re-fetch-and-reslice is the industry-majority default — pagination for both
tools is implemented as pure, stateless re-fetch-and-reslice on every call.
No disk cache, no cross-invocation persistence of any kind.

## Design: `web_fetch`

Input schema (new optional fields, `url` unchanged and still required):

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

- `max_length`: optional, default `5000`, capped at `15000` characters. The
  cap was chosen conservatively relative to Marix's DeepSeek/GLM backends
  (much smaller effective context budget than Claude-scale models), and
  because the paginated `content` also flows through the newly-added
  tool-call-summarize relay (`workflow_call_summary`, see
  `20260723_010223_tool_call_summarize_relay` tag) — that relay's own prompt
  embeds the raw, un-summarized page text, so it is subject to the exact
  same context-budget risk this feature exists to solve; the cap keeps that
  relay's own input bounded too, regardless of how large the underlying page
  is.
- `start_index`: optional, default `0`.

Output shape:

```json
{
  "content": "...",
  "start_index": 0,
  "next_start_index": 5000,
  "truncated": true,
  "total_length": 849213
}
```

`next_start_index` is `null`/absent when `truncated` is `false`.
`total_length` is reported at zero extra cost in Marix's architecture, since
`curl` already downloads the full body before any slicing happens (unlike a
streaming fetch, where computing total length would itself be expensive).

Implementation (`fn paginate`, private, in `web_fetch.rs`):

- The existing curl → `looks_like_html` → `clean_html` pipeline is
  completely unchanged and still always runs on the full page.
- Slicing operates on **character indices**, not raw byte offsets, to avoid
  ever splitting a multi-byte UTF-8 codepoint (`content.char_indices()` maps
  character position to byte offset before slicing the `String`).
- `start_index > total_length` is an error. `start_index == total_length` is
  a valid "end of content" case, returning an empty page with
  `truncated: false`. This exactly mirrors
  `src/tool/native/process/output.rs`'s existing
  `OutputSnapshot::from_path` semantics for `read_process_output`, for
  repo-wide consistency.

No natural-language "call again with X" hint is embedded in `content` — the
contract is purely structured fields, matching `read_process_output`.

## Design: `web_search`

Input schema (new optional `offset`, `query`/`max_results` unchanged):

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

`offset` is a raw result-count to skip (matching Marix's own byte/count-based
`read_process_output` convention), not a page-number multiplier (Brave's own
`offset` semantics), since it maps 1:1 onto how DuckDuckGo's and Yahoo's own
query parameters actually work.

Output shape:

```json
{
  "results": [...],
  "offset": 0,
  "next_offset": 10,
  "has_more": true
}
```

`has_more` is a best-effort heuristic (`results.len() >= max_results`) —
HTML scraping gives no reliable total-result-count signal from the engine
itself, so this cannot be exact.

Implementation (`web_search.rs`):

- `search(query, max_results, offset)` threads `offset` into
  `search_duckduckgo`/`search_yahoo` only.
- `search_duckduckgo`: appends `&s={offset}` to the request URL when
  `offset > 0` (DuckDuckGo's raw result-skip parameter). Omitted entirely
  when `offset == 0`, so the default-call request URL is byte-identical to
  pre-change behavior.
- `search_yahoo`: appends `&b={offset + 1}` when `offset > 0` (Yahoo's
  classic `b=` parameter is 1-indexed "begin at result number N"; `+1`
  adjusts Marix's 0-indexed `offset` to Yahoo's convention). Same
  omit-when-zero behavior.
- `search_wikipedia` is deliberately left unchanged and does not receive
  `offset` at all. Wikipedia's `opensearch` API is a relevance/prefix search
  with no result-count pagination concept; it always searches from the top
  regardless of the caller's requested `offset`. This is a documented,
  accepted limitation of the lowest-priority fallback engine, not an
  oversight.
- `parse_duckduckgo_results`/`parse_yahoo_results` parsing logic is
  unchanged — pagination only changes which page is requested, not how a
  page's HTML is parsed.

### Known limitation: engine consistency across a pagination sequence

The existing fallback chain (DuckDuckGo → Yahoo → Wikipedia on empty
results) is unchanged. Because each `web_search` call is stateless, a
sequence of paginated calls with growing `offset` will keep returning
DuckDuckGo-sourced pages as long as DuckDuckGo keeps returning non-empty
results at each offset (the fallback order is not conditioned on `offset`).
If DuckDuckGo's results run out exactly at some offset, subsequent pages
degrade to Yahoo's own from-scratch pagination — which is a reasonable
degradation at the natural "end of results" boundary, not a broken
continuation, but the model has no explicit signal that the underlying
engine changed mid-sequence. This was accepted as a pre-existing property of
the fallback design, not something this change attempts to solve.

## Files changed

- `src/tool/native/web/web_fetch.rs`: new `DEFAULT_MAX_LENGTH`/
  `MAX_MAX_LENGTH` consts, extended `INPUT_SCHEMA`/`DESCRIPTION`, new
  `struct Page` + `fn paginate`, `invoke()` parses the two new optional
  fields and calls `paginate` instead of returning unbounded content.
- `src/tool/native/web/web_search.rs`: extended `INPUT_SCHEMA`/
  `DESCRIPTION`, `invoke()` parses `offset` and adds `offset`/`next_offset`/
  `has_more` to the response, `search`/`search_duckduckgo`/`search_yahoo`
  signatures gained an `offset: usize` parameter, `search_wikipedia` left
  untouched.
- No changes to `ToolProgram`, `ToolPreview`, or any other protocol-layer
  Rust type — both tools' `invoke(&self, call: &str) -> String` signature is
  unchanged; only the JSON schema string constants and the JSON content
  built inside `invoke()` changed. This is why the change was implemented
  directly (`feature-implement`-style) without a separate `feature-design`
  pass.

## Explicitly out of scope for this change

- No disk cache, temp files, or any cross-invocation persistence for either
  tool.
- No query/substring-locate ("grep-in-page") parameter on `web_fetch` — no
  agent studied combines this with offset pagination on the same call, and
  it was not requested.
- No fix to the engine-consistency-across-pagination limitation described
  above.
- No changes to `clean_html`/`looks_like_html`/tag-parsing helpers in
  `web_fetch.rs`, or to `parse_duckduckgo_results`/`parse_yahoo_results`/
  `search_wikipedia`'s parsing logic in `web_search.rs`.

## Verification performed

- `cargo check -p marix-tool`: clean (only the pre-existing, unrelated
  multi-bin-target warning documented in
  `.github/skills/project-build/SKILL.md`).
- `cargo clippy -p marix-tool --lib --all-features`: no new warnings
  (verified by diffing against a `git stash`-ed baseline run).
- Manual `--preview` and stdin sanity tests against both
  `marix_tool_web_fetch` and `marix_tool_web_search` binaries, built and run
  one at a time (never together, to avoid the documented Cargo
  feature-unification hazard across this crate's 15 shared-source `[[bin]]`
  targets): confirmed contiguous pagination, `start_index == total_length`
  boundary behavior, out-of-range errors, invalid `max_length`/`start_index`/
  `offset` validation errors, and that `offset` genuinely changes the live
  DuckDuckGo results returned.
