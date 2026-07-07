---
name: experience-compact
description: Compact Marix agent experience files, remove overlap with agent definitions, and propose up to five definition upgrades.
---

## Purpose

Compact every Marix agent experience file under `.github/experience/` into a much shorter, high-signal addendum to its paired agent definition under `.github/agents/`.

Experience files must contain only extra operational lessons that are not already present in the corresponding agent definition. After compaction, identify a small set of important lessons that may belong in agent definition files and ask the user which, if any, to promote.

## Trigger

Use this skill when the user asks to compact, compress, summarize, prune, or reduce agent experience files, especially when they mention `experience-compact`.

## Pairing Rules

- Pair `.github/experience/<name>.md` with `.github/agents/<name>.agent.md`.
- If an experience file has no paired agent definition, compact it only against the global agent instructions known in the session and report the missing pair.
- If an agent definition has no experience file, do not create one unless the user explicitly asks.
- Ignore non-Markdown files and dot-prefixed folders.

## Compaction Workflow

1. **Read all pairs** — Read every paired agent definition and experience file before editing so duplicated content can be identified consistently.
2. **Extract durable lessons** — Keep only lessons that are concrete, reusable, and specific to that agent's work. Remove narrative history, one-off task notes, stale decisions, implementation logs, and content already captured in the agent definition.
3. **Remove definition overlap** — Do not repeat responsibilities, tool boundaries, triggers, repository paths, or policies already stated in the paired `.agent.md` file. Experience content must be additive.
4. **Rewrite compactly** — Rewrite each experience file as concise Markdown with short headings and dense bullets. Prefer imperative rules and precise gotchas over prose.
5. **Enforce size limit** — For each paired file, the remaining experience content must be less than half the paired agent definition length. Measure by non-whitespace character count after compaction. If a file cannot meet the limit without losing essential information, keep only the most critical lessons and report the omission.
6. **Collect promotion candidates** — After compaction, identify lessons that are too important to stay only in experience and may belong in the corresponding agent definition.
7. **Ask before promotion** — Present at most five promotion candidates and ask the user which items to add to agent definitions. Do not edit agent definition files until the user confirms specific items.
8. **Apply confirmed promotions** — If the user approves items, update only the relevant `.github/agents/*.agent.md` files and remove or further shorten the promoted content from the corresponding experience file so duplication is not introduced.

## Promotion Candidate Rules

- Suggest no more than five items total across all agents.
- Each candidate must include:
  - the target agent file,
  - the proposed concise definition text,
  - why it belongs in the definition rather than experience.
- Prefer candidates that affect safety, routing, ownership boundaries, or recurring correctness failures.
- Do not suggest content that already exists in the target agent definition.
- If no lesson deserves promotion, report that no promotion candidates were found and do not ask a confirmation question.

## Editing Rules

- Preserve the existing language and style of each edited file unless the file is inconsistent.
- Keep Markdown valid and concise.
- Do not edit files under `src/` or `overview/`.
- Do not modify unrelated skills, workflows, or source design metadata.
- Do not run git commands.
- Use precise file edits; do not rewrite agent definitions unless applying user-approved promotion candidates.

## Validation

After compaction:

1. Confirm every edited experience file is valid Markdown text.
2. Confirm every paired experience file is less than half the non-whitespace character count of its paired agent definition.
3. Confirm no compacted experience file repeats material already present in the paired agent definition.
4. Confirm the promotion candidate list has at most five items.
5. If promotion candidates were approved, confirm promoted content is not duplicated between agent definition and experience.

## Reporting

Report:

- experience files compacted,
- before/after non-whitespace character counts for each compacted file,
- paired agent definition count used for the half-size limit,
- any omitted or ambiguous lessons,
- promotion candidates, if any, and the user's promotion choices,
- files changed after any approved promotions.
