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
