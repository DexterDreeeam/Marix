# source-programmer experience — Marix

## Validation gotchas

- If Cargo fails at the repo root, rerun from `src/`; the repository root intentionally has no `Cargo.toml`.
- The workspace is not fully rustfmt-clean. Avoid formatting unrelated files; for one Rust file use `rustfmt --edition 2024 --check <file>`.
- Known baseline warnings: all native tool bins share `tool/tool_main.rs`, and several worker/state handle fields are currently `dead_code`.

## Current source shape

- Model-facing plan drafts are flat serde data: `PlanDraft { description, run_steps, pending_steps, expected_result }` and `StepDraft { name, kind, description, input }`; `input` defaults for intent steps, and `kind` is `tool`, `model`, or `intent`.
- Protocol `Answer` and `PlanDraft` have no inherent parse helpers; call sites deserialize with `serde_json::from_str`.
- `src/server/plan/draft.rs` is intentionally absent. Model completion handles answer JSON before plan JSON; `Plan::from_draft` builds runtime plans and `Step::from_draft` maps flat drafts to `StepKind`.
- Execution signatures are injected only when a tool draft becomes a runtime step. `StepSignature` owns `StepId`, `PlanSignature` owns `PlanId`, and there are no numeric step numbers or `TaskState::step_count`.
- Alias placeholders were removed repo-wide. Config now reads literal TOML; credential refs are separate and must remain.
- 2026-07-07: Config endpoints now centralize node IP under `ServerConfig.ip`; client/host connections and telemetry clients derive addresses from `config.server.ip` plus `client_port`, `host_port`, or `telemetry_port`. The current config contract has no separate host or telemetry sections.
- 2026-07-08: `accept_channel`/`connect_channel` in `src/common/structure/channel.rs` take `_auth: ChannelAuth` by value (handshake consumes the token). The 4 call sites (client/session.rs, host/session.rs, and two in server/session/session.rs) construct `ChannelAuth { token: String::new() }` inline behind a `TODO(feature-implement)` to source the token; keep the `ChannelAuth` import in each file. Bodies are still `panic!("not implemented")` design stubs.
- 2026-07-08 (superseding above): `accept_channel`/`connect_channel` now take a single `endpoint: ChannelEndpoint` (`Client`|`Host`, `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`); `ChannelAuth` was removed entirely. Address (ip + client_port/host_port by role) AND handshake token are resolved from `Config` INSIDE the functions (design phase reads nothing yet — bodies stay `panic!("not implemented")`). Re-exports in `common/structure/mod.rs` and `common/lib.rs` now list `ChannelEndpoint` instead of `ChannelAuth`. The 4 call sites now pass only `ChannelEndpoint::Client`/`::Host`; they no longer build a `SocketAddr` or call `Config::load()` for the address, so `Config`/`SocketAddr` imports were dropped there and `Session::bind_address` in server/session/session.rs was removed. IMPLEMENT-phase deferral: add a config source for the handshake token (new `ServerConfig` field + credential + deploy `config.toml` entry) before wiring the real handshake.
- 2026-07-08 (IMPLEMENTED the above): `channel.rs` `accept_channel`/`connect_channel` now spawn a `std::thread` + current-thread tokio runtime, do TCP bind+accept / connect, and run a shared `connect_socket` remoc helper. Handshake wire type is `remoc::rch::base::Sender/Receiver<(String, NetReceiver<T>)>`: each side sends `(own_token, net_rx)`; `accept_channel` passes `Some(expected_token)` and rejects a mismatch with `ChannelError::Auth("channel token mismatch")`; `connect_channel` passes `None` (server side is the only gate). The runtime MUST keep the `connection_task = tokio::spawn(connection)` handle and `connection_task.await` after sending the setup result — otherwise `block_on` returns, the runtime drops, and the remoc connection future never drives the channel. Config: added `ServerConfig.auth_token: String` + `RawServerConfig.auth_token: CredentialRef` (reads `.credential/SERVER_AUTH_TOKEN.txt` via `read_credential`, mirroring `ip`). Address resolved inline via `.parse::<SocketAddr>()` (turbofish REQUIRED — a `let x: SocketAddr = ...parse()` annotation alone does NOT let the `map_err(|error| ...)` closure infer `AddrParseError`, giving E0282). channel.rs deliberately does NOT `use crate::external::*`: that glob shadows the extern `remoc` crate and would break the file's own `remoc::rch::mpsc`/`remoc::RemoteSend` type aliases; the style-guide wrapper-routing rule is already unmet by those pre-existing aliases, and `external/remoc.rs` doesn't re-export `Connect`/`Cfg`/`rch::mpsc`/`RemoteSend` anyway. Runtime needs `.credential/SERVER_AUTH_TOKEN.txt` and `config.toml` `[server] auth_token` (deploy-side, outside src/).
- 2026-07-08 (supersedes the two CredentialRef notes above): runtime config no longer reads `.credential/*.txt`. `config.toml` is a deploy template where credential values are `{{NAME}}` placeholders substituted before the app runs, so the loader just parses plain strings. Removed from `src/common/config/config.rs`: private `struct CredentialRef`, `fn read_credential`, `pub struct CredentialConfig`, `Config.credential` field, `RawConfig.credential` field, and `ConfigError::EmptyCredential` (+ its Display arm). `RawServerConfig.ip/auth_token` and `RawDeepseekConfig.api_key` are now `String` (raw == resolved). `load_config` builds `ServerConfig`/`DeepseekConfig` directly from raw strings; `resolve_config_path` is still used (tool.directory) and stays. Dropped `CredentialConfig` from re-exports in `common/config/mod.rs` and `common/lib.rs`. Only those 3 files referenced it; no other crate consumed it. cargo check clean.
- 2026-07-08: `Config` cache (`src/common/config/config.rs`) is now `static CONFIG_CACHE: RwLock<Option<Result<Config, String>>> = RwLock::new(None)` (was `OnceLock`). `load()` keeps get-or-init/first-wins: read-lock fast path clones a `Some`, else compute `load_config(&config_path())` and `write().get_or_insert(computed).clone()` (only stores if still `None`, guarding the race). Lock poisoning uses the repo convention `.unwrap_or_else(|error| error.into_inner())`, never `.unwrap()`. `load_config` was split into `load_config` (reads file → `repository_root_for_config` → delegates) + `build_config(content: &str, repo_root: &Path)` (pure `toml::from_str::<RawConfig>` → `resolve_runtime_paths` → construct `Config`; behavior-identical extraction). New `pub fn mock(overrides: &[&str]) -> Result<Self, String>` for tests: honors load-first (`Self::load()?`), re-reads the on-disk `config_path()` into a `toml::Table`, deep-merges each TOML fragment via private recursive `merge_tables(&mut toml::Table, toml::Table)` (only recurses when BOTH sides are `toml::Value::Table`, else overlay replaces), re-serializes with `toml::to_string`, rebuilds via `build_config`, then UNCONDITIONALLY installs `Some(Ok(config))` into the write-locked cache so later `Config::load()` return the mock (used by channel/logger which call `Config::load()` internally). toml crate is 0.9 (default features `serde`+`parse`+`display`, NO `preserve_order` → `Table` is `BTreeMap`-backed, so re-serialization sorts keys — fine for reparsing). Added `pub use ::toml::{to_string, Table, Value}` to the wrapper `src/common/external/toml.rs`; access as `toml::Table`/`toml::Value`/`toml::to_string` via the existing `use crate::external::*`. cargo check clean (only pre-existing baseline warnings; marix-common itself warning-free).
- 2026-07-08: Telemetry `Logger` (`src/common/logging/logger.rs`) now rides the remoc channel transport instead of hand-rolled framed-JSON-over-TcpStream, so it gets the same token handshake as sessions. Added `ChannelEndpoint::Telemetry` (→ `config.server.telemetry_port`) and wired it in BOTH `accept_channel`/`connect_channel` match arms. `Logger::host()` (was `host(port)`) sets `Sink::Local(store)` then spawns a thread running an accept loop: `accept_channel::<LogMessage>(Telemetry)` per connection, each `Ok` spawns a detached worker owning `net_rx` with a current-thread tokio runtime `block_on(while let Ok(Some(m)) = net_rx.recv().await { LOGGER.record(m); })`; `Err` sleeps 200ms and retries (best-effort, brief rebind race accepted). `Logger::connect()` (was `connect(SocketAddr)`) retries `connect_channel::<LogMessage>(Telemetry)` up to 5x with 200ms backoff, stores the sender in `Sink::Remote(Mutex<NetSender<LogMessage>>)` (was `Mutex<TcpStream>`). `telemetry()` locks the mutex (immutable guard suffices — remoc `try_send(&self, T) -> Result<Sending<T>, TrySendError<T>>`) and `try_send`s. `TrySendError<T>` impls `Display` with NO `T: Display` bound, so `error.to_string()` is safe. `ChannelError` has NO `Display` impl (only Debug), so map it to string via `format!("{error:?}")`, not `.to_string()`. Added `LoggingError::Channel(String)` (+ Display arm) for both send and connect failures. Removed the 4 raw-TCP helpers (`spawn_worker`/old `run_worker`/`send_message`/`read_message`) and the now-dead `std::io::{Read,Write}` + `std::net::{SocketAddr,TcpListener,TcpStream}` imports. Call sites: `server/main.rs` `Logger::host()`, `host/main.rs` + `client/cli/main.rs` drop the `telemetry_address`/`SocketAddr::parse` dance and their `std::net::SocketAddr` import, calling `Logger::connect()` directly. `Store` + redb logic left verbatim. cargo check clean (only pre-existing baseline warnings).

- 2026-07-08 (supersedes the symmetric-token handshake notes above): `channel.rs` handshake is now an asymmetric private enum `Handshake<T> { Connect { token, rx: NetReceiver<T> }, Accept { rx: NetReceiver<T> }, Reject }` carried over `remoc::rch::base::Sender/Receiver<Handshake<T>>` (needs `#[derive(serde::Serialize, serde::Deserialize)]` + `#[serde(bound(serialize/deserialize = "T: remoc::RemoteSend"))]` + `where T: remoc::RemoteSend`; remoc `Receiver<T>` is Serialize/DeserializeOwned when `T: RemoteSend`). `accept_channel` NO LONGER returns `ChannelError::Auth` — it LOOPS (bind once via `TcpListener`, `accept()` in a loop) until ONE connection both establishes AND passes the token check; per-connection failures (accept err, transport err, token mismatch, or 5s handshake timeout) `continue` and send NOTHING on the setup mpsc, so the caller's `setup_rx.recv()` stays blocked. Only server-side setup failures send an `Err` and return: `Config::load()`/address-parse → `Setup`, `TcpListener::bind` → `Bind`, runtime build → `Runtime`. On success `accept_loop` `drop(listener)` IMMEDIATELY (frees the port so a later `accept_channel` on the same endpoint can bind — this is what makes tests 4/5 work) then `connection_handle.await` for keep-alive. The 5s cap is `tokio::time::timeout(HANDSHAKE_TIMEOUT=5s, server_handshake(...))`. Now the CONNECTER surfaces auth failure: on token mismatch the server `base_tx.send(Handshake::Reject)`, then must keep driving the connection briefly for the reject to flush (via `ConnectionGuard::flush(REJECT_FLUSH_GRACE=2s)`) before returning `Err(Auth)`; `connect_channel` maps `Reject` → `ChannelError::Auth("channel token rejected")`, `None`/`Err`/unexpected variant → `Transport`. LEAK PREVENTION: the remoc `connection` future MUST be `tokio::spawn`ed to drive the base channel, but a timed-out/rejected connection must not leak a detached task — wrap the spawned `JoinHandle<()>` in `struct ConnectionGuard { handle: Option<JoinHandle<()>> }` whose `Drop` calls `.abort()`; `server_handshake` `disarm()`s it (returns the handle) only on success, so every early-return path (incl. the outer `timeout` dropping the future) aborts the task. Wrap the connection as `async move { let _ = connection.await; }` to get `JoinHandle<()>`.
- 2026-07-08 (test pattern for Config-internal callers): Test pattern for code that calls `Config::load()` internally (channel/logger): tests live INSIDE `marix_common` as `#[cfg(test)] mod tests;` in `structure/mod.rs` → `structure/tests/mod.rs` (`mod channel;`) → `structure/tests/channel.rs`, using crate-internal paths (`crate::config::Config`, `crate::structure::{...}`). `Config::mock` writes the PROCESS-GLOBAL RwLock cache, so tests MUST serialize: `static TEST_GUARD: Mutex<()>` locked for each test body (poison-tolerant `.lock().unwrap_or_else(|e| e.into_inner())`), plus a DISTINCT localhost port per test (34110/20/30/40/50) to dodge TIME_WAIT/rebind. The on-disk root `config.toml` is a `{{PLACEHOLDER}}` template (unparseable), so write a concrete base fixture to `std::env::temp_dir()` and point `MARIX_CONFIG` at it ONCE via `static BASE_CONFIG: OnceLock<()>` under the guard; `std::env::set_var` is `unsafe` in edition 2024 (wrap in `unsafe {}`, justified because only called under TEST_GUARD). The base fixture must satisfy `RawConfig` `deny_unknown_fields` (name, [runtime] environment/mode/marix_path, [client], [server] with ALL of enabled/ip/auth_token/client_port/host_port/telemetry_port/max_turns, [model]+[model.deepseek], [tool]); `install_config(token, port)` overlays only `[server] ip/auth_token/client_port` via `Config::mock(&[fragment])`. Run `accept_channel` on a `std::thread` (it blocks) with a ~300-400ms `sleep` barrier before the single-shot `connect_channel` (connect does NOT retry, so it can hit connection-refused if it races the bind). Send with `net_tx.try_send(v)`; recv needs an async ctx — build a fresh current-thread tokio runtime and `block_on(tokio::time::timeout(t, rx.recv()))` (cross-thread wakers are fine — each side's connection is driven by its own accept/connect runtime thread). Wrong-token test: start accept (reads "correct" at call time), sleep barrier, THEN re-mock "wrong" and connect → assert `Err(Auth(_))` AND `!accept.is_finished()`, then re-mock "correct" + valid connect to unblock/join. Timeout test: raw `std::net::TcpStream::connect` that sends nothing → server 5s-timeouts and closes → raw client's blocking `read` returns `Ok(0)` at ~5s; assert closed && elapsed in [4s,8s] && `!accept.is_finished()`. All 5 tests pass, ~13s total (the 5s-timeout test dominates), stable across repeated runs.
- 2026-07-08 (step is the parent kind for execution/relay): `Execution`/`ExecutionHub` moved from `src/server/execution/` to `src/server/step/execution/` — execution is a KIND of step. `crate::step::mod` now re-exports both `execution::{Execution, ExecutionHub}` and `relay::{Relay, RelayHub}`; server `lib.rs` surfaces all four at crate root via `pub use step::{Execution, ExecutionHub, Relay, RelayHub, Step};` (there is NO top-level `crate::execution` module anymore). Inside `step/execution/hub.rs` the self-import is `use crate::step::execution::Execution;` (mirror the original absolute-path style, NOT `super::`). `TaskState` holds BOTH `execution_hub: ExecutionHub` and `relay_hub: RelayHub`. New `relay` protocol module (`src/protocol/relay/`) mirrors `execution/` field-for-field but is the LOCAL model-request kind: `RelayId`/`RelaySignature{task,relay_id,name}`/`RelayRequest{signature,input:String}`/`RelayStatus{Started,Running,Succeed(usize),Failed{reason}}`/`RelayEvent{Evoke,Update,Status}`+`RelayUpdate{seq,content}`. Registered in BOTH `protocol/lib.rs` AND `protocol/mod.rs` (the crate has two parallel entry files that must stay in sync). `SessionEvent` gained a `Relay(RelaySignature, RelayEvent)` variant (between `Execution` and `Plan`) — the ONLY exhaustive `match`es that needed a new arm are server `task.rs::run_worker` and `session.rs::route_session_event`; client `to_client_event` and host `execution.rs`/`executor.rs` all use `_ =>`/`let-else` so they were unaffected. Relay routing (unlike execution's host-forwarding) routes back to the owning task locally: `route_session_event` does `Self::route_task_event(state, &signature.task.id, event.clone())` (parallel to `Plan`); relay events are NOT forwarded to the client (the `send_client_event` `matches!` filter was left as-is by design). DESIGN-PHASE deferral: `Step::run`'s `StepKind::Model(_)` arm still calls the inline `run_model`; the implement phase will rewire model steps through `relay_hub.run_relay_step` + `SessionEvent::Relay` streaming, and may embed a `RelayRequest` in `ModelStepKind` (currently model steps carry no request, unlike `ExecutionStepKind::Invocation(ExecutionRequest)`).
- 2026-07-08 (RelayRequest field rename): `RelayRequest`'s data field is now `prompt: String` (was `input: String`); `RelayRequest { signature, prompt }`. Safe single-file design-phase edit — the field is never accessed anywhere: `RelayRequest` only appears as re-exports (`protocol/lib.rs`, `protocol/mod.rs`, `relay/mod.rs`) and as the `RelayEvent::Evoke(RelayRequest)` payload in `relay/event.rs`, none of which read the field. Unrelated `.input` fields on `ToolInputSchema`/`StepDraft` are distinct types — do not touch.
- 2026-07-08 (task-worker termination decoupled from routers): `server/task/task.rs` `run_worker` no longer uses router `bool` returns as a keep-alive side-channel. It now top-level-matches the two TERMINAL task events FIRST — `SessionEvent::Task(_, TaskEvent::Cancel)` (→ `send_status_event(Canceled)` + break) and `SessionEvent::Task(_, TaskEvent::Status(TaskStatus::Succeed(result)))` (→ `send_status_event(Succeed(result))` + break, DISTINCT client messages) — and routes everything else through `other => Self::route_task_event(&state, other)`. `route_task_event` is now a side-effect-only `-> ()` dispatcher over `Step`/`Execution`/`Relay`/`Plan` (with `Task(_,_) => {}` for non-terminal task events). All routers dropped their `-> bool`: `Step::route_step_event`, `Step::on_complete`, `Step::on_model_complete`, `ExecutionHub::route_event`, and `RelayHub::route_event` (relay stayed a `panic!("not implemented")` design stub — only its signature changed). KEY WIRING: the session forwards Task `Status` events to the CLIENT ONLY (`session.rs` `route_session_event` ignores `Task(_, Status(_))` and `send_client_event` only forwards them outward — never back to the worker), so the model final-answer path (`on_model_complete`) must self-post the Succeed terminal to the worker's OWN inbox. Added `TaskState.task_tx: Sender<SessionEvent>` (the worker inbox sender, `task_tx.clone()` passed into `TaskState::new` from `Task::new`); `on_model_complete` now does `state.task_tx.send(SessionEvent::Task(sig, TaskEvent::Status(TaskStatus::Succeed(TaskResult{content: answer.answer}))))` instead of `session_tx.send(...)`. `run_worker`/`route_task_event` take `state: &Arc<TaskState>` (deref-coerces fine for `send_status_event(&TaskState)` and passes straight through to `execution_hub.route_event(state, ...)` which wants `&Arc<TaskState>`). NOTE: `TaskState::new` calls `RelayHub::new()` which `panic!`s — the whole relay path is still a design stub, so any live `create_task` would panic at runtime, but cargo check is clean (panic bodies compile). Refactor was warning-neutral (19 marix-server baseline warnings unchanged, incl. the relay unused-param warnings which pre-date this change).
- 2026-07-09: Server task ownership now has `src/server/task/access.rs::TaskAccess` as the single cloneable packet for Task-level access (`session_context`, `session_tx`, `signature`, `user_request`, `rt`). `TaskState` stores `access` plus Task-owned hubs/queues/channels only; Plan/Step/Invocation/Relay states each store `TaskAccess` plus their own signatures, queues, hubs, and local data. Constructor chains pass `TaskAccess` clones (`Plan::from_draft`, `Step::{new,from_draft,trigger_initial_plan}`, `InvocationHub::create`/`Invocation::new`, `RelayHub::create`/`Relay::new`) instead of separately threading runtime/session fields. Relay remains the only owner of `model_backend`. `cargo check --quiet` from `src/` is clean.
- 2026-07-08 (supersedes the note directly above — that variant over-collapsed the loop): the intended `server/task/task.rs` shape keeps per-event-variant match arms in `run_worker` and makes `route_task_event` a `TaskEvent`-ONLY `-> bool` handler. `run_worker(state: Arc<TaskState>, ...)` has 5 arms: `SessionEvent::Task(_, _) => if !Self::route_task_event(&state, event) { break; }` (ONLY the Task arm consults the bool to break); `Step`/`Execution`/`Relay`/`Plan` arms call their routers DIRECTLY with no bool and no break. `route_task_event(state: &TaskState, event: SessionEvent) -> bool` returns `false` for the two terminals with DISTINCT client messages — `Task(_, Cancel)` → `send_status_event(Canceled)` + `false`; `Task(_, Status(Succeed(result)))` → `send_status_event(Succeed(result))` + `false` — and `_ => true` for everything else; it NO LONGER dispatches Step/Execution/Relay/Plan. Router arg forms: `Step::route_step_event` takes owned `Arc` → `Arc::clone(&state)`; hub `route_event`s take `&Arc<TaskState>` → `&state`; `route_task_event` takes `&TaskState` and is called `Self::route_task_event(&state, event)` (`&Arc<TaskState>` deref-coerces to `&TaskState`), then forwards `state: &TaskState` straight into `send_status_event(&TaskState)`. KEPT from the prior refactor: the `-> ()` returns on hub routers / `Step::route_step_event`/`on_complete`/`on_model_complete`/`plan_hub.route_event`, the `TaskState.task_tx` self-sender field, and `on_model_complete` self-posting the Succeed terminal to `task_tx` (so the worker's Task arm receives it → `route_task_event` matches Succeed → notifies client → `false` → break). cargo check clean from `src/` (19 marix-server + 1 marix-host baseline warnings, unchanged).

- 2026-07-08 (task-worker terminals inlined into run_worker; supersedes the two variants above): `server/task/task.rs` `run_worker` now matches the two TERMINAL task events as their OWN top-level arms and breaks there, so exit is visible in the main loop: arm1 `SessionEvent::Task(_, TaskEvent::Cancel)` -> log + `send_status_event(&state, Canceled)` + `break`; arm2 `SessionEvent::Task(_, TaskEvent::Status(TaskStatus::Succeed(result)))` -> log + `send_status_event(&state, Succeed(result))` + `break`. arm3 `SessionEvent::Task(_, event)` binds a TaskEvent (Cancel/Succeed already peeled off) -> `Self::route_task_event(&state, event);` (NO bool, NO break). `Step`/`Execution`/`Relay`/`Plan` arms unchanged. `route_task_event` is now `fn route_task_event(_state: &TaskState, event: TaskEvent)` (was `(&TaskState, SessionEvent) -> bool`): body is an EXHAUSTIVE no-op match over all 6 TaskEvent variants `=> {}` to consume `event` (avoids unused_variables); `_state` prefixed since worker has no per-event handling yet (placeholder). Must keep the exhaustive listing (Create/CreateFailed/Query/Preview/Cancel/Status(_)) not `_ => {}` so adding a TaskEvent variant later forces a compile touch. `send_status_event` unchanged. cargo check clean from src/ (19 marix-server + 1 marix-host baseline warnings unchanged; the two terminal messages stay DISTINCT client notifications).
- 2026-07-08 (PlanEvent Complete/Fail + PlanResult): Added the symmetric plan result type + upward-event variants. New `src/protocol/plan/result.rs` mirrors `step/result.rs` and `task/result.rs` exactly — `pub struct PlanResult { pub content: String }`, same `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`, `use crate::external::*;` only. `plan/mod.rs` gained `pub mod result;` + `pub use result::PlanResult;` in ALPHABETICAL slot (answer, draft, event, id, result, signature). `PlanEvent` (plan/event.rs) grew from a single `Trigger(PlanDraft)` to `Trigger(PlanDraft)` + `Complete { result: PlanResult }` + `Fail { result: PlanResult }` — struct-variant `result` field mirrors `StepEvent::Complete/Fail` (StepEvent::Complete also carries seq_count; PlanEvent::Complete does NOT). Import merged to `use crate::{PlanDraft, PlanResult};`. Re-export invariant CONFIRMED again: `PlanResult` had to be added to BOTH `protocol/lib.rs` AND `protocol/mod.rs` plan lines (`{Answer, PlanDraft, PlanEvent, PlanId, PlanResult, PlanSignature}`) — the crate keeps two parallel entry files in sync; only `lib.rs` is the actual `[lib] path`, `mod.rs` is a non-compiled mirror but is maintained identically for the export block. The ONLY exhaustive match over `PlanEvent` is `server/plan/hub.rs::route_event` — added `PlanEvent::Complete { result: _ }`/`Fail { result: _ }` arms as `panic!(\"not implemented\")` design stubs (`result: _` to avoid unused-binding warnings). Design-phase: no Step->Plan trigger wiring and no Plan->Task upward reporting were touched (deferred to implement phase); did NOT run cargo check.

## 2026-07-08 — protocol/plan re-export duality
- src/protocol/ has TWO parallel module roots that must stay in sync for every plan-level symbol: lib.rs and mod.rs. Both carry an identical pub use plan::{...} line. Any add/remove of a plan re-export must edit both.
- PlanEvent (src/protocol/plan/event.rs) is Trigger(PlanDraft), Complete, Fail — Complete/Fail are unit variants (no payload). Trigger is routed by the task worker into `PlanHub::run_plan`; existing Plan objects log unsupported Complete/Fail in `Plan::route_event`.
- Plan submodules live under src/protocol/plan/{answer,draft,event,id,signature}.rs, each re-exported via plan/mod.rs.

## 2026-07-08 — task event routing ownership
- `src/server/task/task.rs` now routes Step/Execution/Relay/Plan events by finding the concrete stored object and then calling that object's `route_event`; hub-level `route_event` dispatchers are intentionally absent.
- Step lookup must check both `TaskState.steps.with(&StepId, ...)` and `PlanHub::step(&StepSignature)`: run steps live only inside `Plan.run_steps` before `StepEvent::Trigger`, then in the WorkQueue after running.
- `ExecutionHub`/`RelayHub` are storage helpers with `with_mut` accessors; object behavior lives in `Execution::route_event` and `Relay::route_event`. Unsupported or missing routes should log via `Logger::error`/`warning` without payload content.
- `PlanEvent::Trigger` is the special plan-creation entry through `PlanHub::run_plan`; existing Plan objects handle later object events themselves and currently log unsupported Complete/Fail.

## 2026-07-08 — invocation protocol design split
- Protocol now distinguishes server-side `Invocation*` from host-side `Execution*`: `src/protocol/invocation/` owns `InvocationId`, signature/request/status/event, while `src/protocol/execution/` owns host `ExecutionId`, `ExecutionSignature`, `ExecutionStatus`, and `ExecutionEvent::Cancel`; host execution creation has no draft payload.
- Public event shapes are hierarchical definitions only: `SessionEvent` carries task create/update and nested task events, `TaskEvent` carries plan create/update/cancel, and `StepEvent` carries invocation/relay create/update/cancel. The old implementation routers still need an implement pass to target that hierarchy.
- Server subjects moved to top-level modules: `src/server/invocation/` exports `Invocation`/`InvocationHub`, and `src/server/relay/` exports `Relay`/`RelayHub`; `src/server/step/mod.rs` only re-exports those top-level subjects for compatibility.
- Host runtime type is now `host::executor::execution::Execution` (not `ExecutionRuntime`) and takes a `Tool` plus server sender; its event loop is intentionally a `panic!("not implemented")` design stub until host execution routing is implemented.

## 2026-07-08 — host execution draft removal
- Host execution no longer exposes a draft payload/type: `src/protocol/execution/draft.rs` is absent, `src/protocol/execution/mod.rs` exports only event/id/signature/status, and `InvocationEvent::ExecutionCreate` is payloadless.
- Host runtime state is now `src/host/executor/execution/state.rs`; the public `ExecutionState` name is unchanged and carries only the tool plus server sender in this design layer.
- `PlanStatus` has both `Success` and `Fail`; `PlanEvent` carries `PlanUpdate(PlanStatus)` independently from `TaskEvent::PlanUpdate`. `SessionEvent` has no top-level cancel variant.

## 2026-07-08 — host execution dispatch implementation
- Superseded by the correction below: `dispatch` is receiver-side handling and must not enqueue on `execution_tx`.
- Host `Executor::dispatch` is only a thin extractor for the current `SessionEvent -> TaskEvent -> PlanEvent -> StepEvent -> InvocationEvent::Execution` path. `ExecutionCreate`/`ExecutionUpdate` remain logged as incomplete because the current protocol has no host execution create payload/signature path and no complete status/output route back to the server.

## 2026-07-08 — host execution dispatch correction (supersedes above)
- `src/host/executor/execution/execution.rs` uses `Execution::sender()` as the only external entry to enqueue `ExecutionEvent`s. Private `Execution::dispatch(&ExecutionState, ExecutionEvent)` is receiver-side handling called by `event_loop` after `execution_rx.recv()`; it must not send on `execution_tx`.
- `src/host/executor/executor.rs` forwards nested `InvocationEvent::Execution` payloads with `execution.sender().send(event)` and logs send/forward failures separately from object dispatch failures.

## 2026-07-08 — executor protocol envelope design
- Protocol now has `src/protocol/executor/` for host-executor envelope events:
  `ExecutorEvent::{Execution, ExecutionCreate, ExecutionUpdate, ExecutionCancel}`.
  The create payload is `ExecutionRequest`, currently a public type alias to
  `InvocationRequest` in `src/protocol/execution/request.rs`.
- Keep `ExecutionRequest` and `ExecutorEvent` exported from both parallel
  protocol roots (`src/protocol/lib.rs` and `src/protocol/mod.rs`).
- `ExecutionStatus` is intentionally `Started`, `Processing(String)`,
  `Canceled`, `Killed`, `Succeed(usize)`, and payloadless `Failed`;
  do not restore `Running` or `Failed { reason }` for host execution status.

## 2026-07-08 — InvocationEvent execution payload
- `InvocationEvent::Execution` now carries only `ExecutionEvent` (no `ExecutionSignature`). The host executor cannot recover an `ExecutionId` from the surrounding `InvocationSignature`, so it records a routing warning instead of inventing an invocation-to-execution id mapping; `ExecutorEvent::Execution` remains the signed host-executor envelope.

## 2026-07-08 — protocol signature hierarchy
- `src/protocol/plan/signature.rs` is intentionally non-recursive: `PlanSignature` is `{ task, id, name }` only. `StepSignature` adds `{ task, plan, id, name }`, and `InvocationSignature`/`RelaySignature` add `{ task, plan, step, *_id, name }`.
- `StepSignature` no longer carries `description` or `kind`; server runtime `Step` keeps the draft `description`/`kind` separately for legacy routing/stringification while protocol signatures remain identity-only.

## 2026-07-08 — invocation/execution status protocol shape
- `InvocationStatus` and `ExecutionStatus` now share the same public variants:
  `Created`, `Started`, `Processing { seq, content }`, `Canceled`,
  `Killed`, `Succeed { seq_count }`, and payloadless `Failed`. Do not restore
  `Running`, tuple `Processing(String)`, tuple `Succeed(usize)`, or
  `Failed { reason }` in either protocol status.
- `ExecutionRequest` is a concrete protocol struct
  `{ signature: ExecutionSignature, input: ToolInputSchema }`, not an alias to
  `InvocationRequest`. `InvocationEvent::Execution` remains unsigned
  (`Execution(ExecutionEvent)`); use `ExecutorEvent::Execution` for the signed
  host-executor envelope.
- 2026-07-08: `ExecutorEvent::ExecutionUpdate` is now signed as
  `ExecutionUpdate(ExecutionSignature, ExecutionStatus)`, while
  `InvocationEvent::ExecutionUpdate` remains status-only. Keep executor and
  invocation envelopes distinct when updating direct matches/constructors.

## 2026-07-08 — signature key API
- `src/protocol/signature.rs` owns the shared `SignatureKey` UUID wrapper for
  WorkQueue-friendly signature keys. It derives `Ord`, `Hash`, and serde via
  `use crate::external::*`, and `Signature::key()` defaults to
  `SignatureKey(self.id())`, so concrete signature impls only need `id()`.
- Keep `SignatureKey` re-exported next to `Signature` from both protocol roots:
  `src/protocol/lib.rs` and the mirrored `src/protocol/mod.rs`.

## 2026-07-09 — host executor event boundary
- Host `Executor::dispatch` is an `ExecutorEvent` boundary only. `HostSession`
  must unwrap `SessionEvent::Executor(event)` before dispatching; other
  session events are unsupported on the host side and should be logged.
- Host execution status is a two-hop path: `Execution` sends
  `ExecutorEvent::ExecutionUpdate(signature, status)` on the private executor
  channel, and the executor worker wraps it back into
  `SessionEvent::Task(...Plan(...Step(...Invocation(...ExecutionUpdate))))`
  before sending to the server.
- 2026-07-09: `src/host/session.rs` no longer calls
  `Executor::dispatch` directly; it enqueues `SessionEvent::Executor(event)`
  through `executor.sender().send(event)` and warns if the worker channel is
  closed. `src/host/executor/executor.rs` keeps `executor_tx` on public
  `Executor`, shares registry/executions through private `ExecutorState`, and
  the worker calls private `dispatch`; `ExecutionUpdate` forwarding is inlined
  in that dispatch branch. Execution event route logs snapshot names with
  `format!("{event:?}")` rather than a helper.
- 2026-07-09: `src/host/session.rs` keeps its private host event loop helper
  named `worker` (not `run_worker`) to match the `HostSession.worker` field,
  and unsupported host-side `SessionEvent`s are warning-logged with direct
  Debug formatting (`{event:?}`); do not reintroduce a `session_event_name`
  helper for those warnings.
- 2026-07-09: `ExecutorEvent` now has only `Execution`,
  `ExecutionCreate`, and signed `ExecutionUpdate`; the unused envelope-level
  `ExecutionCancel` variant and host executor unsupported-cancel dispatch arm
  were removed. Use `ExecutionEvent::Cancel` for per-execution cancellation.
- 2026-07-09: Host executor shared state lives in
  `src/host/executor/state.rs` as sibling-private `ExecutorState`; it owns the
  `ToolRegistry`, `WorkQueue<ExecutionSignature, Execution>`, and private
  executor sender. `src/host/executor/mod.rs` declares `mod state;` without a
  public re-export, and `executor.rs` imports `super::state::ExecutorState` so
  worker/dispatch behavior stays local to the executor module.


## 2026-07-09 — server event-chain workers
- `src/server/session/session.rs` treats client/host network receives as enqueue-only: both connection workers send `SessionEvent` into the session worker channel, and private `dispatch` either creates a task, forwards `SessionEvent::Task(..)` to `Task::sender()`, forwards `TaskUpdate` to the client, or sends `SessionEvent::Executor(..)` to the host channel.
- `src/server/task/task.rs`, `src/server/plan/plan.rs`, `src/server/step/step.rs`, `src/server/invocation/invocation.rs`, and `src/server/relay/relay.rs` each own an mpsc worker loop plus `sender()`; callers send protocol events to the child sender and never call child dispatch directly.
- Upward server status now uses the task worker channel: invocation/relay workers wrap status as `TaskEvent::Plan(PlanEvent::Step(StepEvent::*Update))`, step/model code emits `TaskEvent::PlanCreate` or `SessionEvent::TaskUpdate`, plan workers emit `TaskEvent::PlanUpdate`, and the task worker is the only layer that forwards `TaskUpdate` to the session worker.
- Server invocation bridges to host execution with `SessionEvent::Executor(ExecutorEvent::ExecutionCreate(..))`; host `ExecutionUpdate` returns through the nested Task/Plan/Step/Invocation event path before invocation maps it to `InvocationStatus` and emits a step update.
- `src/protocol/relay/status.rs` mirrors `InvocationStatus` (`Created`, `Started`, `Processing`, `Canceled`, `Killed`, `Succeed { seq_count }`, `Failed`) so relay and invocation updates have the same status shape.

- 2026-07-09: In `src/server/session/session.rs`, private receive-loop helpers are named `client_worker`/`host_worker`, and the session worker spawner is `task_worker` (not `spawn_task_worker`). Client/host receive loops inline `state.task_tx.send(message.event)` with the existing enqueue warning; task routing uses `dispatch_task`.
- 2026-07-09: Remaining source cleanup keeps runtime worker bodies named
  `worker` (not `run_worker`) in client telemetry/server task-plan-step
  workers, and nested event forwarders named `dispatch_task` /
  `dispatch_plan` / `dispatch_step` / `dispatch_invocation` /
  `dispatch_relay`. Event logs in task/plan/step routing use
  `format!("{event:?}")`; do not reintroduce `*_event_name` helpers there.
- 2026-07-09: `src/server/task/task.rs` intentionally has no
  `plan_error_name` helper. `PlanError` diagnostics in plan creation/insertion
  use direct Debug formatting (`{error:?}`); remaining `*_name` helpers under
  `src/` are behavior/path helpers (`parse_model_step_name`,
  `input_model_name`, `database_file_name`), not log stringifiers.

## 2026-07-09 — typed dispatch stop reasons
- `src/server/task/task.rs`, `src/server/plan/plan.rs`, `src/server/step/step.rs`, `src/server/invocation/invocation.rs`, `src/server/relay/relay.rs`, and `src/host/executor/execution/execution.rs` now use private `*DispatchError` enums behind `Result<(), _>` dispatch methods; worker loops log `{error:?}` before breaking on cancel/terminal statuses while unsupported task-level events still warn and continue.
- `src/server/step/step.rs` maps terminal `InvocationStatus` and `RelayStatus` values directly to `StepDispatchError` variants; the old private `is_terminal_invocation` / `is_terminal_relay` bool helpers in `src/server/step/child.rs` were removed to keep step worker exit reasons typed.

## 2026-07-09 — protocol error relocation
- Public protocol stop/error enums now live in
  `src/protocol/{task,plan,step,invocation,relay,execution}/error.rs`,
  derive serde through `use crate::external::*`, and are re-exported from each
  leaf module plus both protocol roots (`lib.rs` and the mirrored `mod.rs`).
- The private server/host `*DispatchError` enums were removed; worker dispatch
  methods use `TaskError`, `PlanError`, `StepError`, `InvocationError`,
  `RelayError`, or `ExecutionError` from `marix_protocol`. The old
  `src/server/plan/error.rs` is gone; plan hub/build/step draft validation use
  `marix_protocol::PlanError`.

## 2026-07-09 — status cleanup
- Protocol statuses no longer have any `Killed` variants; keep stop handling on
  `Canceled`. This also removes the matching protocol errors
  `ExecutionKilled`, `InvocationKilled`, and `RelayKilled`; server routing now
  treats only canceled/succeed/failed as terminal child outcomes.
- `TaskStatus` is now `Created`, `Started`, `Canceled`, `Succeed`, `Failed`.
  There is no task-level streaming/update status. Model/invocation/relay
  processing chunks stay in the lower-level event chain/logs until a terminal
  task status is produced.

## 2026-07-09 — plan step updates
- `PlanEvent` no longer carries `PlanUpdate(PlanStatus)`. Step outcomes flow
  through `PlanEvent::StepUpdate(StepStatus)`, where `StepStatus` mirrors the
  invocation/relay lifecycle shape without `Killed`. The plan worker converts
  terminal step status to `TaskEvent::PlanUpdate(PlanStatus::{Success, Fail})`.

- 2026-07-09: `Logger::log`/`warning`/`error`/`debug` are fire-and-forget `-> ()` APIs. They route through private `emit`, while `telemetry`/`record` keep returning `Result<(), LoggingError>` internally. Logging failures are reported with non-recursive `eprintln!("marix logger failed: {error}")`; call sites should invoke `Logger::...(...)` directly without `let _ =`.
- 2026-07-09: Protocol signature Display support lives beside each signature type in `src/protocol/*/signature.rs` and intentionally prints only the owned UUID field (`Task/Plan/Step.id.0`, `Invocation.invocation_id.0`, `Relay.relay_id.0`, `Execution.execution_id.0`). Server/host logs can now use `{signature}`/`{signature.task}` without cloning IDs or adding type prefixes.
- 2026-07-09: `TaskRequest` now mirrors the other protocol request payloads with `signature: TaskSignature` plus `content`; client task creation owns `TaskSignature::new("task".to_owned())`, and server session task creation destructures the request and must not mint a replacement signature.

- 2026-07-09: `src/server/session/session.rs` session context reset helper is named `reset_context`; host connect and disconnect paths call this private helper to clear `SessionContext` after host lifecycle changes.
eset_context; host connect and disconnect paths call this private helper to clear SessionContext after host lifecycle changes.

- 2026-07-09: Logger call sites for protocol signatures should prefer Rust captured format strings ({task_signature}, {plan_signature}, {step_signature}, {invocation_signature}, {relay_signature}, {execution_signature}) over extra {} args. Field expressions such as state.signature or self.signature.task need clear local refs first so signature Display implementations are used directly without cloning IDs.

- 2026-07-09: `src/server/task/task.rs` task workers now have a private close channel (`close_tx`/`close_rx`) and use `marix_common::select!` to wait on close and task events without polling. `Task::sender()` intentionally clones `TaskState.task_tx`; `Task` no longer stores a duplicate task sender. `Task::dispatch` only accepts `TaskEvent`; `SessionEvent::TaskUpdate` is handled by `dispatch_status`. Supporting change: `src/common/structure/channel.rs` aliases local `Sender`/`Receiver` to `crossbeam_channel` and keeps remoc setup plumbing on `std_mpsc`.
- 2026-07-09 (supersedes the TaskState sender ownership note above): `src/server/task/task.rs` owns the worker inbox sender on `Task { task_tx }`; `Task::sender()` returns `self.task_tx.clone()`. `TaskState` only carries `session_tx`, so Plan/Step/Invocation/Relay helpers that already wrap `SessionEvent::Task(...)` or `SessionEvent::TaskUpdate(...)` send through `state.session_tx` and log "session worker stopped" on failure. `SessionState` names its own queue `session_tx`/`session_rx`, so `state.task_tx` is reserved for Task-owned code.

- 2026-07-09: src/server/task/task.rs keeps the close channel for explicit future shutdown, but Task must not implement Drop to auto-send close_tx.send(()); store the unused sender as _close_tx: Sender<()> to preserve the channel without dead-code warnings.
- 2026-07-09: `src/server/task/task.rs` task worker inbox is typed as
  `Sender<TaskEvent>`/`Receiver<TaskEvent>`; the session layer unwraps
  `SessionEvent::Task(signature, task_event)` in `dispatch` and sends only the
  inner `TaskEvent` through `dispatch_task`. `SessionEvent::TaskUpdate(_)`
  remains session-to-client only and must not enter the task worker; do not
  reintroduce an `on_worker_event` SessionEvent adapter in `Task`.

- 2026-07-09: Logger signature-only temporary refs for display (for example `let task_signature = &state.signature; Logger::log(format!(...))`) are now inlined as named `format!` arguments at the `Logger::log`/`debug`/`warning`/`error` call site, e.g. `task_signature = &state.signature`. Keep one-off signature Display refs inside the Logger statement; only keep a local ref when it is reused by more than one log or other logic.
- 2026-07-09: `Task::new` now constructs channels/state and immediately emits `TaskStatus::Created`, but does not spawn. `Task::run(&mut self)` consumes the stored task/close receivers, stores `Option<JoinHandle<()>>`, warns and returns on repeat/missing-channel calls, and the worker emits `TaskStatus::Started` before logging and triggering the initial plan. Server session task creation must insert the `Arc<Mutex<Task>>` into `SessionState.tasks` before locking it to call `run()`, so initial worker events can route back to the task.

- 2026-07-09: Runtime objects with child workers now follow construct-then-run ordering. `Task`, `Plan`, `Step`, `Invocation`, `Relay`, host `Execution`, host `Executor`, `server::Session`, and `host::HostSession` keep receivers/handles in `Option` storage and warn on repeated `run` instead of panicking. Insert children into their hub/queue before `run()` so Created/Started updates can route; Plan/Executor/Session types do not emit Created/Started because their public statuses have no such variants.
- 2026-07-09: Worker-backed runtime State must stay flat across layers. `ClientSessionState`/`HostSessionState` own their loop handles, `PlanState` carries only task session routing, `StepState` transfers task fields it uses (`session_context`, task signature, user request, model backend handle, session sender, hubs, step queue), and `InvocationState`/`RelayState` carry only their session sender plus private runtime inner data. Do not pass `Arc<TaskState>` into Plan/Step/Invocation/Relay objects or hubs; pass concrete handles such as `session_tx` instead.

- 2026-07-09: `src/server/step/step.rs` `Step::trigger_initial_plan` is intentionally flat: it takes `TaskSignature`, `user_request`, and `Sender<SessionEvent>` directly and must not import or accept `TaskState`; `src/server/task/task.rs` clones those fields from its own worker state at the call site.

- 2026-07-09: Host `Execution` now keeps only `state: Arc<ExecutionState>` plus its `_worker` handle on the outer object; `ExecutionState` owns the execution event sender/receiver with tool/request/executor sender. `Execution::sender()` clones `state.execution_tx`, and `Execution::run()` takes `state.execution_rx` before spawning so Created still emits in `new()` and Started emits inside the worker after executor insertion.
- 2026-07-09: `src/host/executor/execution/state.rs` keeps
  `ExecutionState.execution_rx` as `StdMutex<Option<Receiver<ExecutionEvent>>>`
  rather than `Arc<StdMutex<_>>`; the whole state is already inside
  `Arc<ExecutionState>`, and `Execution::run(&self)` only needs mutex interior
  mutability to take the one-shot receiver before spawning the worker.

## 2026-07-09 — host execution receiver ownership cleanup
- `src/host/executor/execution/state.rs` now stores `ExecutionState.execution_rx` as a bare `Receiver<ExecutionEvent>`. Because `marix_common::Receiver` is the local crossbeam receiver alias, `recv()` only needs `&self`; the worker can read through `Arc<ExecutionState>` without `Mutex<Option<_>>` ownership transfer. `Execution::run()` still uses `_worker` as the single-run guard before spawning `Execution::worker(state)`.

## 2026-07-09 — worker runtime state ownership cleanup
- Worker-backed runtime outers now keep only `state: Arc<...State>` and worker join handles. `ExecutorState`, `PlanState`, `StepState`, `InvocationState`, `RelayState`, `TaskState`, `ClientSessionState`, and `SessionState` own their senders/receivers and runtime data; workers read bare `marix_common::Receiver` values through the shared state because the local channel is crossbeam-backed. Keep these states flat: pass concrete handles (`session_tx`, hubs, queues), not nested parent State objects.

- 2026-07-09: Host executor runtime sender ownership: `Executor::new(server_tx)` stores the shared server sender handle in `ExecutorState.server_tx`; `Executor::run()` and the worker no longer receive sender parameters, and server events are emitted through `state.server_tx`. `HostSession::spawn_worker` still owns the shared handle that gets populated after `connect_channel`, so constructing the executor before connection is valid.

- 2026-07-09: Host `Execution` sends status updates directly through `ExecutionState.server_tx` as packaged `SessionEvent::Task(...InvocationEvent::ExecutionUpdate(...))` messages. `ExecutorEvent::ExecutionUpdate` remains in the public protocol for compatibility but should only be treated as a legacy/non-normal path in the host executor.
- 2026-07-09: Worker-owned channels are now created inside the owning State constructors rather than outer runtime constructors: `ExecutorState::new(server_tx)`, `TaskState::new(...)`, inline `PlanState::new(...)`, `StepState::new(...)`, `InvocationState::new(...)`, and `RelayState::new(...)` each allocate their own crossbeam sender/receiver pairs. Keep outer `Executor`/`Task`/`Plan`/`Step`/`Invocation`/`Relay` constructors limited to building `Arc<State>` plus worker handles, and keep sender accessors cloning from state.
- 2026-07-09: Worker-backed outers whose handles are only used by `run`
  keep bare `Option<JoinHandle<()>>`, not `Arc<Mutex<Option<_>>>`.
  `Execution`, `Plan`, `Step`, `Invocation`, and `Relay` now expose
  `run(&mut self)`. Their `Clone` impls (where needed for preview/routing)
  clone only the shared `Arc<State>` and reset the handle to `None`, so
  callers must insert into the owning hub/queue before calling `run` on the
  inserted object. Runtime insertion helpers are `WorkQueue::with_mut` and
  `PlanHub::with_mut`; invocation/relay hubs run via `HashMap::get_mut`
  immediately after insertion.
- 2026-07-09: `marix_protocol::Actor<T, E>` is the shared actor facade:
  `run(&mut T)`, `sender(&T) -> marix_common::Sender<E>`, and
  `close(&T)`. Host `Execution`/`Executor` implement it with exactly
  `state: Arc<State> + _worker: Option<JoinHandle<()>> + close_tx:
  Sender<()>` on the outer struct. Their State constructors create both
  event and close channels and return `(State, close_tx)`; workers use
  `marix_common::select!` over close receiver and event receiver.
- 2026-07-09 (supersedes Actor facade signature above):
  `marix_protocol::Actor<T, E>` keeps the generic name shape but uses
  natural self methods: `run(&mut self)`, `sender(&self) -> Sender<E>`,
  and `close(&self)`. Host `Execution`/`Executor` implement the worker
  logic directly in the trait impl; do not add inherent UFCS wrappers like
  `<Self as Actor<...>>::run(self)`. Call sites outside the defining module
  need `marix_protocol::Actor` in scope for `execution.run()` /
  `executor.sender()` method syntax.
- 2026-07-09: Execution is the first host object split into Actor +
  Runtime. `marix_protocol::Actor<T, E>` now only has `run(&mut self)` and
  `sender(&self) -> Sender<E>`; close signaling is an inherent method on
  host outers (`Execution::close`, `Executor::close`) instead of the Actor
  trait. `marix_protocol::Runtime<E>` owns `close(&self)` and
  `dispatch(&self, E) -> Result<(), Self::Error>`. Host
  `ExecutionRuntime` (file `host/executor/execution/runtime.rs`) replaces
  `ExecutionState` and owns tool/request/server sender/event+close
  receivers; `Execution` keeps only `runtime: Arc<ExecutionRuntime>`,
  `_worker`, and `close_tx`. Execution status sending and event handling now
  live on the runtime; the worker calls `runtime.close()` only after receiving
  the outer close signal.
- 2026-07-09: Host Execution now follows the Actor-starts-Runtime shape. `Actor<T,E>` is `start(&mut self)` + `sender(&self)`, and `Runtime<E>` adds `run(&self)` beside `close`/`dispatch`. `Execution` no longer holds `Arc<ExecutionRuntime>`; `start` creates event/close channels, stores only the tx halves, moves rx halves plus tool/request/server_tx into the thread, constructs `ExecutionRuntime`, then calls `runtime.run()`. `sender()` is intentionally fail-fast with `expect("execution sender requested before start")`; current executor flow inserts the execution, immediately calls `start`, and only later routes events.

- 2026-07-09: Host `Execution` outer is now only `state: ExecutionState`; it has no `close()`, no close sender, and no stored join handle. `ExecutionState` owns the one-shot startup data (`tool`, `request`, `server_tx`) plus `execution_tx` and a `started` guard; `start()` takes runtime parts, creates the execution channel, and drops the spawned `JoinHandle` to detach. `ExecutionRuntime::new` creates its private close channel and stores `_close_tx` so `close_rx` stays alive instead of disconnecting immediately; external execution shutdown is not exposed on the actor.
- 2026-07-09: Host `ExecutionState` (`src/host/executor/execution/state.rs`) is intentionally data-only: no inherent `impl`, no runtime-parts helper. `Execution::new` initializes the option fields directly, while `Actor::start` owns duplicate-start checks, option `take()`s, channel creation, sender storage, and spawning `ExecutionRuntime::run`; `sender()` reads `state.execution_tx` directly with the existing fail-fast expect.

- 2026-07-09: Host `Execution::new` now creates the execution channel immediately and stores `execution_tx` directly so `Execution::sender()` is usable before `start()`. `ExecutionState` stores cloneable startup data (`Tool`, `ExecutionRequest`, `SharedNetSender`) as direct values. Only `execution_rx` remains `Option<Receiver<ExecutionEvent>>` because cloning a crossbeam receiver creates a second live, competing consumer; leaving an unconsumed receiver in state would keep sends successful after the runtime receiver exits and could hide shutdown/backpressure semantics. `Execution::start` clones the direct values, takes the receiver once, and constructs `ExecutionRuntime::new(...)` inside the spawned thread.

- 2026-07-09 (host execution start idempotence): `src/host/executor/execution/state.rs` keeps `ExecutionState` as field-only storage without a `started` flag; `src/host/executor/execution/execution.rs` treats `execution_rx: Option<_>` as the one-shot start guard, so repeat `Execution::start` calls are detected by `execution_rx.take() == None` and log `execution start ignored: event receiver was already moved`.

- 2026-07-09: Host execution actor/runtime now share `Arc<ExecutionState>` across `src/host/executor/execution/{execution.rs,runtime.rs,state.rs}`. State owns `Tool`, `ExecutionRequest`, shared server sender, and both crossbeam channel halves directly (no `Option`, no `started`, no `impl ExecutionState`); `Execution::start` clones the Arc into a detached runtime thread and `Execution::sender` clones `state.execution_tx`. `ExecutionRuntime` keeps only the shared state plus its private close channel pair; `_close_tx` is intentionally retained so `close_rx` does not disconnect while `run` selects on it.

- 2026-07-09: `Runtime` in `src/protocol/runtime.rs` uses an error type parameter (`Runtime<E, Error>`) instead of an associated `type Error`; execution runtime implements it as `Runtime<ExecutionEvent, ExecutionError>` and dispatch returns `Result<(), ExecutionError>`.
- 2026-07-09: Host `ExecutionState::new(tool, request, server_tx)` now owns creation of the execution event channel (`build_channel`) and stores both halves in state. `Execution::new` should only wrap that state in `Arc`, while `ExecutionRuntime` keeps its own private `close_tx`/`close_rx`; runtime `close()` sends on `close_tx`, and the `run()` close branch must only log/exit instead of re-sending the close signal.

- 2026-07-09: `marix_protocol::Actor<T, E>` now sends through `dispatch(&self, E)` instead of exposing worker channel senders. Host `Execution` and `Executor` log warning inside their Actor dispatch implementations on send failure, so callers should route with `actor.dispatch(event)` and only handle lookup/not-found cases locally.

- 2026-07-09: Host execution now lives at src/host/execution/ and is re-exported from host::execution/host::lib, while src/host/executor/ owns only Executor, ToolRegistry, and Tool. Executor runtime logic belongs in src/host/executor/runtime.rs; create executions through crate::execution::Execution and route child events with Actor::dispatch, not sender accessors.

- 2026-07-09: Server Session/Task now mirror the host Actor+State+Runtime standard. `src/server/session/session.rs` and `src/server/task/task.rs` actors each hold only `Arc<State>`, detach `Runtime::run()` in `Actor::start`, and route inbound events through `Actor::dispatch` with warning-on-send-failure. `SessionRuntime` uses `Runtime<SessionEvent, Infallible>` and owns the former client/host/session worker loops plus task creation/routing; `TaskRuntime` uses `Runtime<TaskEvent, TaskError>` and owns the former task worker loop, plan creation/routing, cancellation, and runtime-private close channel.

- 2026-07-09: Server Task now owns a per-task Arc<tokio::runtime::Runtime> in TaskState (built with Builder::new_multi_thread().enable_all()). TaskRuntime, Plan, Step, Invocation, Relay, and Step model workers can be scheduled through this runtime with spawn_blocking because their current worker loops block on crossbeam 
ecv(). Deepseek streaming still uses its backend-local 	hread::spawn because the ModelBackend::request(&mut self, ModelRequest) contract has no runtime/handle parameter and ModelRequest exposes no public runtime access; wiring it into the task runtime needs an explicit model interface change.

- 2026-07-09: src/server/task/task.rs no longer emits TaskStatus::Created from Task::new; runtime status emission remains owned by TaskRuntime::send_session_status for Started/Failed/Canceled and any other lifecycle statuses.

- 2026-07-09: Server task-owned Tokio runtime handles are named \\
t\\ in TaskState and downstream worker states (Plan/Step/Invocation/Relay) to avoid confusing runtime fields with the protocol Runtime trait and TaskRuntime/SessionRuntime types; pass Arc<tokio::Runtime> parameters as \\
t\\ through these constructors.

- 2026-07-09: Server model backend ownership now terminates at RelayState: TaskState/Plan/Step no longer carry model_backend; model steps build a RelayRequest and relay workers create their own backend via config (Relay::build_model_backend). Relay streams ModelResponse chunks as RelayStatus::Processing, finalizes with Succeed, and Step recovers model output through RelayHub::content_for_step. ModelRequest.step is now a StepSignature, not a Step, so Relay does not need to hold a Step/StepState clone just to call the backend.

- 2026-07-09: Task child objects must not receive Task-owned orchestration resources: `TaskAccess` stays limited to `session_context`, `session_tx`, `signature`, `user_request`, and `rt`. `TaskRuntime` owns hub/queue orchestration for plan creation, step queue start/complete, invocation/relay create/dispatch, terminal content lookup, and Analysis prompt plan stringify; Plan/Step/Invocation/Relay states keep only access plus their own local fields.

- 2026-07-09 (superseded): The former task async runtime design extended sync `Runtime` and used a boxed-future async entrypoint. Do not restore that shape; use the peer `RuntimeAsync` trait described below instead. Task/Plan/Step/Invocation/Relay worker inboxes use Tokio `mpsc::UnboundedSender` with `StdMutex<Option<UnboundedReceiver<_>>>` so actor dispatch remains synchronous and each async worker takes the receiver exactly once. Relay still bridges model backend work through `tokio::task::spawn_blocking` because `ModelBackend::request` is blocking and returns a crossbeam receiver.

- 2026-07-09: `marix_protocol::RuntimeAsync<E, Error>` is the async peer of `Runtime<E, Error>`, not a subtype. It exposes `async fn run(&self)`, `close`, and `dispatch` directly, avoiding boxed futures and a separate async-run method. Server `TaskRuntime` implements only `RuntimeAsync<TaskEvent, TaskError>` and is started from `Task::start` with `state.access.rt.spawn(async move { task_runtime.run().await; })`, so do not add `block_on` or `spawn_blocking` for the task loop.


- 2026-07-09: Relay/Invocation now mirror Task's `RuntimeAsync + State` split. `src/server/relay/{state.rs,runtime.rs}` and `src/server/invocation/{state.rs,runtime.rs}` keep event receivers in runtime-owned `StdMutex<Option<UnboundedReceiver<_>>>` after taking them from state; outer `Relay`/`Invocation` only hold `Arc<State>` and spawn `Runtime::run()` on `TaskAccess.rt`. Created status is still emitted by outer `new`; Started and runtime business transitions are emitted by runtime/state helpers. Module privacy uses `pub(super) mod state/runtime` plus sibling imports through `super::state::...`/`super::runtime::...`, not re-export aliases, because `pub(super) use` of `pub(super)` structs hit E0365. `cargo check --quiet` from `src/` was clean.

- 2026-07-09: Invocation has been tightened to the Actor/State/RuntimeAsync shape in `src/server/invocation`: `Invocation` implements `Actor<Invocation, InvocationEvent>` and no longer exposes `run`, `sender`, `push`, or `finalize`; `InvocationHub::create` starts it through `Actor::start`, and `TaskRuntime` dispatches invocation events through `Actor::dispatch`. `InvocationState`/`InvocationInner` now only carry fields plus their `new` constructors; status forwarding, executor forwarding, output accumulation, finalization, content assembly, and completion checks live in `InvocationRuntime`. `Invocation::new` still emits Created through `InvocationRuntime::send_step_update`.
- 2026-07-09: Async actor mailboxes are now created through `build_async_channel()` in `src/common/structure/channel.rs` and re-exported by `marix_common`; task/plan/step/invocation/relay runtimes should use that helper instead of direct `tokio::mpsc::unbounded_channel()` calls. `InvocationState` stores the canonical `InvocationSignature` only; use `state.signature.step` for step routing and avoid persisted invocation status.

- 2026-07-09: Invocation output content is no longer exposed through `Invocation::content` or `InvocationHub::content`; task-side `invocation_content` intentionally returns empty content, while relay/model content still uses their own paths.
- 2026-07-09: Invocation runtime no longer has `send_step_update`, receiver-take helpers, execution forwarding, start/status mapping/terminal/push/finalize helpers, or startup step status emission; `RuntimeAsync::run` takes both receivers inline and `dispatch` only handles create/update/cancel. `InvocationEvent` no longer carries `Execution(ExecutionEvent)`; direct execution forwarding should use another route if needed.
- 2026-07-09: Relay now mirrors Invocation actor layering: `Relay` only owns `Arc<RelayState>` and implements `Actor`, `RelayState` is data-only (`signature.step` is the single step source), and `RelayRuntime::request_worker` runs the blocking model request on a std thread that self-notifies the runtime with `RelayEvent::Update(RelayStatus)` before runtime forwards step events. RelayHub now creates/starts/inserts and dispatches through `Actor`; TaskRuntime no longer reads relay content after relay success.

- 2026-07-09: Relay self-notification no longer uses protocol `RelayEvent::Update`. `src/protocol/relay/event.rs` is external-control only (currently `Cancel`); `src/server/relay/runtime.rs` keeps a runtime-private async `ModelResponse` channel for backend stream chunks. `ModelResponse` is now a struct `{ content, seq, complete }`: content chunks carry `complete=false` and complete frames carry empty content with `seq` equal to the chunk count.`
- 2026-07-09: Model backend async stream API is designed in `src/server/model/backend.rs` without using `async fn` on `ModelBackend`, preserving `Box<dyn ModelBackend>` dyn-compatibility for RelayState. Public aliases are `ModelResponseReceiver` for the existing crossbeam receiver and `ModelResponseAsyncReceiver` for the Tokio unbounded receiver; the blanket `ModelBackend` impl forwards `request_async` to `ModelBackendImpl::request_async`, whose default body is a design stub `panic!("not implemented")`.
- 2026-07-09: Deepseek async streaming in `src/server/model/backend_deepseek.rs` uses `build_async_channel()` and a spawned Tokio task around async `reqwest::Client`; shared SSE parsing emits the same `ModelResponse { content, seq, complete }` sequence as the blocking path and logs async stream failures instead of inventing failed responses. Relay now consumes the backend async receiver directly in `src/server/relay/runtime.rs`, so it should not keep a relay-local model response channel or request worker thread.
- 2026-07-10: `build_async_channel()` in
  `src/common/structure/channel.rs` must use `tokio::sync::mpsc`
  (`UnboundedSender`, `UnboundedReceiver`, `unbounded_channel`);
  Tokio does not expose `mpsc` at `tokio::mpsc` with the current
  dependency features. `RuntimeAsync` implementors, including
  `src/server/task/runtime.rs`, must provide `dispatch` even when
  `run` owns the receive loop. `src/server/invocation/runtime.rs`
  keeps a `pub(super)` `send_step_update` helper because
  `Invocation::new` emits Created status through the runtime helper.

- 2026-07-10: Invocation Created status is emitted directly through
  `InvocationRuntime::send_step_event(..., StepEvent::Update(...))`;
  `send_step_update` is intentionally absent. `send_step_event` is
  `pub(super)` so `invocation.rs` can call the shared event path. Relay's
  `send_step_update` helper remains separate and unchanged.
2026-07-10: `accept_channel` must bind its server endpoints to the IPv4
wildcard address independently of `config.server.ip`; `connect_channel`
continues to resolve the configured server IP. The channel tests use a
remote reserved address for the listener and then remock localhost for the
connecter to protect this distinction.
- 2026-07-10: Logging mode is owned by `Config.logging.remote`.
  `Logger::connect()` uses the telemetry channel only when true and otherwise
  opens a local redb store under `<runtime.marix_path>/log`. `Logger::host()`
  always records locally under
  `<runtime.marix_path_server-or-marix_path>/log`, and starts the telemetry
  accept loop only when remote logging is enabled. Local database filenames
  include the process ID to avoid concurrent process collisions.
- 2026-07-10: Client CLI connection and task-response waits both use
  `Config.client.request_timeout_ms`; connection readiness is polled in 25ms
  slices and zero means immediate timeout. `ClientSession::send_to_server`
  reports missing/broken transport through both `Logger::error` and a
  `ClientEvent::Done`, after releasing the sender mutex. Its worker retries
  failed connects with 200ms backoff and logs only the first and each 25th
  failure; task submission is logged only after `try_send` succeeds.
- 2026-07-11: `TaskAccess.rt` remains a Tokio current-thread runtime, so
  `Task::start` must move its cloned `Arc<Runtime>` and `Arc<TaskState>` to a
  dedicated std thread and call `rt.block_on(TaskRuntime::run())` there. This
  continuously drives task work plus nested Plan/Step/Invocation/Relay
  `rt.spawn` jobs without blocking the synchronous SessionRuntime dispatcher;
  the 2026-07-09 note prohibiting task-loop `block_on` is superseded.
- 2026-07-11: Plan child orchestration belongs to
  `server/plan/runtime.rs`: register each call/model Step through
  `TaskAccess::insert_step` before starting it, dispatch InvocationCreate for
  call Steps, then start the model exactly at `completed_steps == call.len()`.
  Empty-call plans start the model immediately. Model Relay prompts are built
  in PlanRuntime from the user request, ordered call outputs, PlanStringify,
  and a SessionContext snapshot; future Intent Steps remain prompt-only.
- 2026-07-11: Model output plan parsing belongs to
  `protocol/plan/draft.rs`: `PlanDraft::parse` deserializes the protocol
  structure directly, then normalizes call/model/future steps to
  `tool`/`model`/`intent` through `StepDraft::parse`. `StepDraft.kind`
  defaults during serde input and positional semantics overwrite any model
  value. The synthetic first model step is named `Initial` so PlanRuntime
  selects InitialPrompt; generated follow-up model steps remain `Analysis`.
- 2026-07-11: `Invocation::start` is the execution-create boundary: after
  spawning `InvocationRuntime`, it dispatches one buffered
  `InvocationEvent::ExecutionCreate`, guarded by private atomic start state.
  `StepRuntime` only inserts and starts the invocation. Host
  `ExecutionRuntime::execute` already emits `Created` then `Started`, which
  returns through invocation updates; no second startup event belongs in the
  step layer.
- 2026-07-11: Unknown tools in `host/executor/runtime.rs` must return nested
  `InvocationEvent::Update(StepletStatus::Failed)` through
  `request.signature.invocation`; logging and returning alone strands the
  server invocation.
- 2026-07-11: Host tool discovery is executor-owned. After each connection,
  `HostSession` dispatches `ExecutorEvent::ToolQuery`; `ExecutorRuntime`
  previews its registry and sends `SessionEvent::ExecutorTools`, which
  `SessionRuntime` installs into `SessionContext.tools`. Registration-state
  handling is superseded by the later server-session routing note.
- 2026-07-11: The preceding host tool-discovery startup detail is superseded:
  `HostSession` stores the first connected `net_tx` before starting its
  executor, and a worker-private guard prevents executor restarts on reconnect.
  `ExecutorRuntime::run` registers tools once at startup through
  `send_executor_tools`; `ExecutorEvent::ToolQuery` remains dispatchable for
  compatibility but has no active dispatch caller.
- 2026-07-11: `HostSession::worker` dispatches only
  `SessionEvent::Executor`; every other session event, including
  `ExecutorTools`, follows the common unsupported-event warning path.
- 2026-07-11: Server session uses `SessionContext.tools` itself as executor
  registration state, so an empty list rejects task creation. Client ingress
  handles `TaskCreate` and `Task` directly through the same private helpers as
  actor dispatch; all other client variants are unsupported. Host ingress
  continues to enqueue session events.
- 2026-07-11: Server session ingress routing is unified. `SessionRuntime` is
  cloneable through its shared `Arc<SessionState>` and cloned close channel
  endpoints; the client, host, and session receive loops all call that same
  runtime's `Runtime::dispatch` directly. Client/host workers must not match
  event variants or relay events through `SessionState.session_tx`; connection
  setup, receiver storage, context reset, and host disconnect cleanup remain
  worker lifecycle responsibilities.
- 2026-07-11: `server/task/runtime.rs` must retain
  `marix_common::external::*` after draft parsing moves to protocol because it
  still uses the released `tokio` and `serde_json` namespaces. It no longer
  needs a direct serde derive dependency.
- 2026-07-11: Relay status and processing notifications call
  `RelayRuntime::send_step_event` directly; the `send_step_update` and
  `send_step_processing` wrappers are intentionally absent. The event helper
  is `pub(super)` only so `Relay::new` can emit `Created` across sibling
  modules.
- 2026-07-11: `PlanEvent::Update` identifies its originating
  `StepSignature`; `PlanRuntime` resolves that signature before acting and
  ignores unknown updates. Call completion is status-derived with
  `call.iter().all(|step| step.status() == StepStatus::Succeed)`, so empty
  call sets are complete, invocation success can start the model, and only
  model success reports `TaskEvent::Update(..., PlanStatus::Success)`.
- 2026-07-11: Startup follows the ownership chain
  `PlanRuntime -> StepRuntime -> Invocation/Relay`. `PlanRuntime` only inserts
  and starts Steps. `StepRuntime::run` takes both runtime receivers before
  dispatching `StepKind` and creating its Invocation or Relay. Child `Created`
  events are buffered on `step_rx` until initialization enters the select
  loop; initialization failures fail the Step and return without starting the
  loop. Analysis prompt construction clones its current Plan through
  `TaskAccess::plan`.
- 2026-07-11: Model Relay request construction is owned by
  `src/server/step/helper.rs::model_request`; `StepRuntime` only calls it and
  creates the returned Relay. The helper preserves session-context snapshots,
  current-Plan lookup/stringification, prompt panic conversion, and Relay
  signature construction as one responsibility.
- 2026-07-11: `SessionRuntime` task creation, task dispatch, and client-event
  sending are instance helpers over its shared `self.state`; dispatch and
  task-creation failure paths call `self.create_task`, `self.dispatch_task`,
  and `self.send_client_event` directly rather than static state-parameter
  wrappers.
- 2026-07-11 (supersedes the Relay event-helper note above): Relay `Created`
  emission belongs to `RelayRuntime::new`, after the complete runtime instance
  is constructed. `Relay::new` only constructs the Relay, and the private
  instance helper `send_step_event(&self, event)` owns access to runtime state
  for all Relay status and processing notifications.
- 2026-07-11 (supersedes the Invocation startup-boundary note above):
  Invocation now mirrors Relay lifecycle ownership. `InvocationRuntime::new`
  emits `Created` through its private instance `send_step_event`, and
  `InvocationRuntime::run` creates the execution after taking both receivers
  and before entering its event loop. `Invocation` only owns shared state and
  starts the runtime; `InvocationEvent` contains only host updates,
  processing output, and cancellation.
- 2026-07-11: `PlanRuntime::run` owns Step startup. It takes both `plan_rx`
  and `close_rx` before calling its private `start_steps`, so startup-emitted
  Step updates and close signals remain buffered for the runtime loop.
- 2026-07-11: Analysis model input now crosses the Plan→Step boundary through
  clone-shared `StepState.input: Mutex<Option<String>>`. `PlanRuntime` renders
  ordered call output in `plan/helper.rs` as `- {step name}: {step.output()}`
  lines, calls `Step::set_input` before insertion/start, and
  `step/helper.rs::model_request` reads only that input. It no longer obtains
  Plans through `TaskAccess` or uses `PlanStringify`; the Analysis prompt's
  current-plan and pending-intentions strings are empty.
- 2026-07-11: `native_os_env` is owned by
  `src/tool/native/sys/os_env.rs`, selected by the `os_env` feature and
  `marix_tool_os_env` bin. It accepts only an empty object (or empty CLI
  input), returns a fixed nested system/user/path allowlist with nullable
  missing values, and never enumerates environment variables. Unix user
  folders only parse `user-dirs.dirs` with `$HOME`/`${HOME}` substitution;
  no shell syntax is executed and missing folders use HOME-based fallbacks.
- 2026-07-11: Native tool protocol names are defined only by each
  `src/tool/native/**` implementation's `NAME` constant and flow into
  `ToolPreview.name`. They intentionally omit the historical `native_`
  prefix, while feature names, module/file names, and `marix_tool_*` bin
  names remain unchanged.
- 2026-07-11: `Initial.prompt` and `Analysis.prompt` define every `call`
  array as parallel independent work: each tool input must be concrete when
  the Plan is emitted and cannot reference sibling output. Result-dependent
  actions remain `future` intents until a later Analysis Plan has real values;
  Initial resolves an unknown Desktop path with `os_env` before list/read.
  Analysis retries a failed action only when diagnostic output supplies a
  concrete correction, and never repeats the identical failed call.
- 2026-07-12: `src/prompt/step/Analysis.prompt` keeps each of its five render
  variables to one interpolation. Its compact decision contract permits an
  Answer only for fully satisfied requests; incomplete work and correctable
  failed calls produce a PlanDraft with independent, concrete `call` inputs
  and result-dependent actions deferred to `future` or a later Plan.
- 2026-07-12 (supersedes the Analysis-input note above): Plan-to-Analysis
  input is compact JSON containing `PlanState.background` and rendered
  `call_output`. `step/helper.rs` validates both as strings via
  `serde_json::Value`; `AnalysisPrompt` renders only Tools, Request,
  Background, and CallOutput, while `InitialPrompt` renders only Tools and
  Request.
- 2026-07-12: `Initial.prompt` and `Analysis.prompt` now share one four-part
  skeleton and each renders Tools, Request, Background, and CallOutput once;
  Initial supplies empty strings for the latter two. Both prompt structs
  serialize `SessionContext.tools` directly with `serde_json::to_string`, so
  the rendered value is an array and serialization panics are converted by
  `step/helper.rs::model_request` into explicit prompt-construction failures.
- 2026-07-12: `src/prompt/step/Initial.prompt` and `Analysis.prompt` keep
  identical literal skeletons: the single Rules block precedes the first of
  four separators, retains its eight behavioral rules, and ends with the
  strict two-schema rule. Tools, Request, Background, and CallOutput each
  occur once after the schema separators.
- 2026-07-12 (supersedes the prompt-opening note above): `Initial.prompt`
  and `Analysis.prompt` use the same continuous decision contract before the
  first separator. It selects exactly one answer or tool-call schema, keeps
  parallel inputs independently executable, defers dependencies to `future`,
  and flows directly into the unchanged shared schema/input skeleton.
- 2026-07-12: `src/prompt/step/Initial.prompt` and `Analysis.prompt` require
  tool `input` payloads to survive two JSON parses. Inner JSON special
  characters are escaped for the outer response, Windows backslashes use
  double-layer escaping, and forward-slash Windows paths are preferred when
  accepted. Correctable tool errors must produce a non-identical retry rather
  than an answer.
- 2026-07-12 (supersedes the Windows/error rules above): Both step prompts
  retain only the generic nested-JSON escaping contract; Windows path escaping
  and mandatory retry-on-error guidance are handled outside the prompt.
  `tool/native/mod.rs::parse_input` first preserves standard JSON semantics,
  then narrowly retries after doubling single backslashes only inside exact
  `path`/`cwd` string fields. Existing `\\`/`\"` escapes and every non-path
  field remain untouched; all six native tools share this parser.
- 2026-07-12: Telemetry collection is a separate workspace binary,
  `marix-server-telemetry`; `Logger::host()` always accepts authenticated
  telemetry connections, while `Logger::connect()` alone uses
  `logging.remote` to choose remote transport or a runtime-local redb store.
- 2026-07-12: A server owns one UUID for its process lifetime. It installs the
  UUID in `Logger`, stores it in `SessionState`, and sends
  `SessionEvent::SessionId` as the first post-handshake message. Host/client
  connection readiness is deferred until that first control message is
  received and installed in their logger.
- 2026-07-12: `Logger` keeps its optional session UUID in a poison-tolerant
  `RwLock`; every `LogMessage` snapshots it at emission, so local and remote
  sinks serialize the same session metadata.
- 2026-07-12: Server session UUID ownership lives in `SessionState::new()`,
  which generates the UUID internally. `Session::session_id()` exposes the
  copyable UUID so `server/main.rs` can configure `Logger` before emitting the
  first status and session-initialization logs.
- 2026-07-12 (supersedes the host/client readiness note above): Host and
  client consider the core channel connected as soon as `connect_channel`
  succeeds. `SessionEvent::SessionId` is optional logging correlation metadata
  handled in the normal receive loop; it need not be first or present, and
  pre-ID logs retain `session_id: None`.
- 2026-07-12 (telemetry query API + HTTP page): `ServerConfig`/`RawServerConfig`
  gained `telemetry_http_port: u16` (a new field alongside the existing
  `telemetry_port`), mapped 1:1 in `build_config`. It is deliberately NOT a
  `ChannelEndpoint` — the raw remoc telemetry channel (`telemetry_port`) is
  unrelated wire protocol from the new plain-HTTP status page
  (`telemetry_http_port`), bound directly in `server_telemetry` via
  `tokio::net::TcpListener`, not through `structure::channel`.
- 2026-07-12: Added `src/common/logging/query.rs` (`pub mod query;` in
  `logging/mod.rs`) with a SECOND `impl Logger` block providing
  `session_list()/session_log_list()/session_log_filter()`. This required
  exactly one new `pub(super) fn Logger::local_log()` (returns the local
  store's full message history) plus promoting the previously-fully-private
  `Store` struct/`open_at`/`read_all`/`record` to `pub(super)` in
  `logging/logger.rs` — `pub(super)` on an item defined in the `logging::logger`
  submodule is visible to `logging` AND every descendant module, including the
  sibling `logging::query`, which is the whole trick for cross-file access
  inside one Cargo package without widening the crate's real public API.
  `Store::open_at(path: &Path)` was extracted out of the pre-existing
  `Store::open(config, role)` so tests can build an isolated redb file
  directly, bypassing `Config`/the process-global `LOGGER` OnceLock entirely.
- 2026-07-12: redb 4.1 read access needs `ReadableDatabase` (for
  `Database::begin_read()`, a trait method — `begin_write()` stays inherent so
  it worked before without it) and `ReadableTable` (for `.iter()`, a trait
  default method; `ReadableTableMetadata` alone only gives `.len()`). Added
  both to `common/external/redb.rs`'s existing re-export line. Row shape is
  `Result<(AccessGuard<K>, AccessGuard<V>)>`; get bytes via
  `value.value()` (needs the `ReadableTable`-provided iterator item type, not
  a method on `ReadableTableMetadata`).
- 2026-07-12: The query semantics that matched the spec exactly: `session_list`
  puts `None` (unassigned) first when any unassigned message exists, then
  every `Some(uuid)` session ordered by that session's EARLIEST `emit_ts`
  descending (newest-first), ties broken by ascending UUID string.
  `session_log_list`/`session_log_filter` return one session's messages
  ascending by `emit_ts`; since `Store::read_all` yields rows in ascending
  redb key (= record-insertion) order and `Vec::sort_by_key` is stable, a
  plain `.sort_by_key(|m| m.emit_ts)` after filtering gives "ties keep
  insertion/record order" for free — no explicit record-id field needed on
  `LogMessage`. Keyword filtering trims + treats blank as "no filter", then
  does `.to_lowercase()` on both sides for Unicode-aware case-insensitive
  `contains` (good enough for e.g. "ÜBER"/"über"; no full Unicode
  case-folding library was added).
- 2026-07-12 (testing pattern, no `Config`/global `LOGGER` involved): Because
  `Store::open_at` takes a raw path with no `Config` dependency, unit tests in
  `logging/query.rs` open a FRESH `Store` per test at
  `std::env::temp_dir().join(format!("...{}.redb", uuid::Uuid::new_v4()))`
  (unique per test, no `TEST_GUARD`/serialization needed, unlike the
  `Config`-driven `structure/tests/channel.rs` pattern) and call
  `store.record(...)` + `store.read_all()` directly — never touching the
  process-global `LOGGER` `OnceLock`, so these tests are fully parallel-safe
  and don't interfere with any other test that might call `Logger::host()`.
- 2026-07-12 (new leaf package: `marix-server-telemetry` HTTP status page):
  Added `axum = "0.8"` (default features: `json`+`query`+`tokio`+`http1` are
  enough; no `tower-http`/CORS layer added on purpose — same-origin only) +
  `tokio`(`macros`,`net`,`rt-multi-thread`) + `serde`(`derive`) + `serde_json`
  + `uuid` directly to `server_telemetry/Cargo.toml`. Since `marix-common`
  itself has no reason to depend on axum, the "route third-party crates
  through wrappers" rule was satisfied the same way `marix-protocol` does it:
  a package-root `external/mod.rs` with whole-crate re-exports
  (`pub use axum;`, `pub use serde_json;`, `pub use tokio;`, `pub use uuid;`,
  plus `pub use serde::Deserialize;` since only the derive macro is named
  directly), NOT under `common/external/`. Every `http/*.rs` file does
  `use crate::external::*;` then references `axum::Json`,
  `axum::extract::Query`, `axum::http::StatusCode`,
  `axum::response::{IntoResponse, Response}`, `axum::routing::get`,
  `tokio::net::TcpListener`, `tokio::runtime::Builder`, `uuid::Uuid`,
  `serde_json::json!` fully-qualified — mirrors every `protocol/*.rs` file's
  `use crate::external::*;` + `uuid::Uuid`/`serde_json::from_str` style
  exactly. `IntoResponse` is imported directly (`use axum::response::
  IntoResponse;`) since it is a trait needed for `.into_response()` method
  resolution, same rationale as re-exporting `Serialize`/`Deserialize` by
  name instead of just the `serde` module.
- 2026-07-12: `server_telemetry/http/mod.rs` only has `mod
  error;/handlers;/router;/server;` + `pub(crate) use server::serve;` (and
  `error::HttpError` is NOT re-exported from `mod.rs` — nothing outside
  `http` needs to name it, `main.rs` only calls `http::serve(port)`).
  Sibling private submodules (`error`, `handlers`, `router`, `server`) can
  still reference each other via `crate::http::error::HttpError` etc. even
  though `mod error;` itself has no `pub` — module-privacy only blocks access
  from OUTSIDE the parent `http` module, not from other children of the same
  parent. `server_telemetry` has NO `[lib]` target (bin-only), so `pub(crate)`
  inside it is already the crate's entire "public API" surface — there is
  nothing to leak to another crate regardless of the visibility keyword used.
- 2026-07-12 (axum smoke-testing without a bound real port): `server.rs`
  splits `serve(port)` into `build_runtime` + `bind(port)` (real
  `TcpListener::bind`) + `serve_listener(listener)` (the actual
  `axum::serve(...).await` loop, which also logs "HTTP listening on port N"
  using the LISTENER's actual bound port, so `port=0` test binds still log a
  believable message). Tests call `bind(0)` (OS-assigned ephemeral port —
  this is what "random listener, no real/fixed port" means in this codebase)
  then `tokio::spawn(serve_listener(listener))` inside a
  `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` test, and
  drive real HTTP/1.1 requests with a ~40-line hand-rolled `TcpStream` GET
  helper (`Connection: close` + read-to-EOF) instead of adding an HTTP client
  dependency just for tests. GOTCHA (Windows): binding `0.0.0.0:0` and then
  trying to `TcpStream::connect` to the literal address from
  `listener.local_addr()` (`0.0.0.0:PORT`) fails with `AddrNotAvailable` (os
  error 10049) — connecting TO the wildcard address is invalid on Windows;
  the test must swap in `127.0.0.1` with the bound port
  (`SocketAddr::from(([127,0,0,1], local_addr.port()))`) before connecting,
  even though the server itself bound `0.0.0.0`.
- 2026-07-12 (hosting `Logger` inside `server_telemetry`'s own test binary):
  The HTTP handlers for `/api/sessions`/`/api/logs` need `Logger::host()` to
  have succeeded (else every request 500s with `NotHosting`). Test setup
  writes its own temp-dir TOML fixture (same `RawConfig` shape as
  `structure/tests/channel.rs`'s, now with `telemetry_http_port` too), points
  `MARIX_CONFIG` at it, and calls `Logger::host()` — all inside a
  `static INIT: std::sync::Once` guarded closure so it runs exactly once for
  the whole test binary (the `LOGGER` `OnceLock`/`Config` cache are
  process-global; a second `Logger::host()` call would only return
  `AlreadyConfigured`, harmless but wasteful). Set `[server]` ports to `0`
  in the fixture too (the raw telemetry accept-loop it spawns just binds an
  ephemeral port in the background and is otherwise irrelevant to the HTTP
  smoke tests).
- 2026-07-12 (HTTP error-body hygiene): `/api/logs`/`/api/sessions` map every
  `LoggingError` (which can embed real filesystem paths, e.g. `Database`
  variant messages) to a fixed, generic `{"error":"internal server error"}`
  500 body — the real error detail is only ever passed to
  `Logger::error(format!(...))` (recorded in the local telemetry store, never
  in the HTTP response). `400`s for bad `session_id`/`tag` use fixed static
  strings (`"invalid session_id"`/`"invalid tag"`/`"missing session_id"`)
  rather than echoing the caller's raw query value back, to avoid reflecting
  arbitrary client input into a response body.
- 2026-07-12 (contract fix: only `Logger::host()` is queryable): The prior
  `Sink::Local(Store)` was reused by BOTH `Logger::host()` and
  `Logger::connect()`'s non-remote branch, so `local_log()`'s
  `Some(Sink::Local(store)) => store.read_all()` wrongly let an ordinary
  `connect()`-only runtime answer session/log queries too. Split the enum
  into `Sink::Host(Store)` (set only by `host()`) and `Sink::Local(Store)`
  (set only by `connect()`'s non-remote branch); `record()`/`telemetry()`
  still accept `Host` and `Local` identically (both sides keep recording
  their own process's messages), but the new private free fn
  `host_store(sink: Option<&Sink>) -> Result<&Store, LoggingError>` matches
  ONLY `Sink::Host` and is the sole thing `local_log()` calls — `Local`,
  `Remote`, and unconfigured all fall through its `_` arm to `NotHosting`.
  Extracting the match into its own fn (rather than inlining it in
  `local_log`) let a `#[cfg(test)] mod tests` at the bottom of
  `logger.rs` unit-test the match table directly against
  `Sink::Host(temp_store())`/`Sink::Local(temp_store())`/`None` built via the
  already-`pub(super)` `Store::open_at`, with ZERO use of the process-global
  `LOGGER` `OnceLock` — same "own temp redb file per test, no shared state"
  trick as `logging/query.rs`'s tests. This is the general pattern for
  testing `OnceLock`/global-singleton match logic in this codebase: pull the
  match arms into a small private fn taking the enum by value/reference, and
  unit-test that fn directly instead of trying to reset or race the global.
- 2026-07-12 (visibility-narrowing gotcha in bin-only crates):
  `pub(super)` inside a bin crate submodule ONLY reaches that module's
  direct parent and the parent's OTHER descendants — it does NOT reach back
  up past the parent to the crate root/`main.rs`, even though `main.rs` is
  itself an ancestor of every module. Tried narrowing
  `server_telemetry/http`'s `pub(crate) fn serve`/`pub(crate) use
  server::serve` (in `http/server.rs`/`http/mod.rs`) to `pub(super)` since
  the task asked for narrowest visibility; `cargo check` failed with E0364
  (private and cannot be re-exported) because `main.rs` calls
  `http::serve(...)` from the crate root, which is an ANCESTOR of `http`,
  not a descendant of `http`'s parent. Also had to keep `HttpError`
  `pub(crate)` for the same reason — it appears in `serve`'s `Result<(),
  HttpError>` return type, and a function's visibility can never exceed the
  visibility of types in its own signature (E0603/"more private than" once
  attempted). Everything ELSE in that module narrowed cleanly to
  `pub(super)` (`router::build`, all three `handlers::{root,sessions,logs}`
  fns, `handlers::LogsQuery`) because their only callers (`server.rs`,
  `router.rs`) are siblings/descendants of `http`, never the crate root.
  Rule of thumb: only items reachable directly from `main.rs`/the crate root
  need `pub(crate)` in a bin-only crate; everything reachable solely from
  sibling submodules can go to `pub(super)`.
- 2026-07-12 (final polish, package-root `external/mod.rs` was wrong location):
  the earlier "new leaf package" entry above put the axum/serde/tokio/uuid
  re-export wrapper at `server_telemetry/external/mod.rs` (package root,
  mirroring `marix-protocol`'s pattern), but the actual requirement for THIS
  crate was narrower: "HTTP server and page code must all live under
  `server_telemetry/http/`". Moved the wrapper to
  `server_telemetry/http/external.rs` (private `mod external;` declared in
  `http/mod.rs`, alongside `error`/`handlers`/`router`/`server`), deleted the
  package-root `external/` dir entirely, and dropped `main.rs`'s `mod
  external;` (main.rs has zero axum/serde/tokio/uuid usage — only `Config`/
  `Logger` — so it needs no wrapper access at all). `handlers.rs`/`router.rs`/
  `server.rs` all switched their glob import from `use crate::external::*;`
  to `use crate::http::external::*;` — matches this package's existing
  sibling-reference style (`server.rs` already did `use
  crate::http::error::HttpError;`/`use crate::http::router;` rather than
  `use super::...`). General rule for a bin-only crate: when a third-party
  wrapper is used ONLY by one feature subtree (here, all axum/serde_json/
  tokio/uuid usage is HTTP-only), put `external.rs` inside that subtree's own
  module directory, not at the crate root — package-root placement is only
  right when multiple sibling subtrees would need it.
- 2026-07-12 (page.html empty-session-list bug): `/api/sessions` returning
  `[]` used to leave `#initial-loading` (`Loading…`) visible forever, because
  `loadSessions()` only ever reassigned `state.selectedSession` inside `if
  (... && state.sessions.length > 0)` branches — an empty list fell through
  every branch untouched and `loadLogs()` never got a `selectedSession` to
  build a URL from, so `renderLogs()` (the only code that hid
  `#initial-loading`) was never called. Fixed by adding an explicit
  `state.sessions.length === 0` branch at the TOP of `loadSessions()` that
  resets `state.selectedSession = undefined`, clears `#log-body`, and calls a
  new `setLogAreaState("no-sessions")` helper — added 4th HTML state div
  `#no-sessions-state` (`No sessions available.`, `display: none` inline,
  grouped into the existing `#empty-state, #initial-loading` CSS selector).
  `setLogAreaState(mode)` is a single function taking `"loading" |
  "no-sessions" | "empty" | "table"` that toggles ALL FOUR
  loading/no-sessions/empty/table elements' `display` together (replacing the
  old ad hoc two-line toggle inside `renderLogs`) — this is now the ONLY place
  that touches those four elements' visibility, so there is no way for two of
  them to be visible at once. Because this branch returns before the
  `stillPresent` check, the "selected session disappeared AND new list is
  empty" case the task called out is automatically covered by the same empty
  branch (no separate code path needed) — the old `!stillPresent &&
  state.sessions.length > 0` guard could also drop its `length > 0` half since
  by that point in the function the empty-list case has already returned.
- 2026-07-12 (HTTP root smoke test hardening): `root_route_serves_html_page`
  in `http/server.rs`'s `#[cfg(test)]` block previously only asserted
  `body.contains("<html")`; strengthened to also assert
  `body.contains(r#"id="session-list""#)` /
  `r#"id="tag-filter""#` / `r#"id="keyword-filter""#"` so the smoke test
  actually fails if `page.html`'s session list or tag/keyword filter markup
  ever regresses, not just if the file stops being HTML at all.
- 2026-07-12 (`/api/logs?tag=` contract fix — blank tag means "all tags", not
  400): `handlers::logs`'s `LogsQuery.tag` used to feed `query.tag.as_deref()`
  straight into `parse_tag` whenever `Some(_)`, so `tag=` (present-but-empty,
  which the query-string frontend/JS actually sends for "no tag filter") hit
  the `_ => Err("invalid tag")` arm and returned 400 — wrong per the outward
  contract `tag=<Info|Warning|Error|Debug|empty>`. Fixed by trimming first
  (`query.tag.as_deref().map(str::trim)`) and only calling `parse_tag` when
  the trimmed string is non-empty; missing key OR blank/whitespace-only value
  both collapse to `None` ("all tags"), matching the existing
  `keyword`-blank-means-no-filter precedent one line below it in the same fn.
  A non-empty-but-unrecognized tag still 400s via the same `parse_tag`
  "invalid tag" error. The `tag.is_none() && keyword.is_none()` branch
  (`Logger::session_log_list` vs `Logger::session_log_filter`) needed NO
  change — it already worked correctly once `tag` collapses to `None` for
  blank input. Added a `server.rs` smoke-test assertion
  (`/api/logs?session_id=unassigned&tag=` → 200 + valid JSON array) right next
  to the existing bad-tag-400 case in `logs_route_rejects_invalid_session_and_tag`
  (kept in the same test fn since it's the same query-family, just didn't
  rename the fn). No public API surface changed — `LogsQuery`'s fields and
  `logs`'s signature are untouched; this was purely internal parsing logic.
