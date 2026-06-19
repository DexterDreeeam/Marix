# development-designer Experience

## Purpose

Persistent implementation notes for generating and maintaining Marix source design documents.

## Current Design Contract

- Prefer `.design.md` files under every `src/` folder.
- Existing `.design.json` files are compatibility inputs until migrated.
- Every dot-prefixed file or folder under `src/` is companion metadata owned by development-designer. Maintain these paths beside source files, but never list them as normal source files, child modules, visible diff entries, or visible file-tree entries.
- `.design.md` content should be machine-readable JSON. Raw JSON is acceptable; fenced `json` blocks are also accepted by the overview UI.
- All paths should be repository-rooted and under `src/`.
- Do not list dot-prefixed files or folders as child modules/files.
- Keep design documents concise enough for the overview UI to remain scannable.

## Extraction Rules

- Treat every folder under `src/` as a module.
- Describe direct child modules and direct child source files.
- For each file, list meaningful interfaces, traits, structs, enums, impl blocks, functions, type aliases, constants, statics, and data structures as `items`.
- Every item should include `kind`, `name`, `category`, `signature`, `details`, `code`, and `implements`.
- `code` should contain the complete source definition block, not only the signature.
- `lineStart` and `lineEnd` should point to the same complete definition represented in `code`.

## Exposed Groups

- `exposedGroups` drive star-map exposed elements.
- Include only concrete public definitions owned by the module layer.
- Do not include wiring declarations such as `mod ...`, `pub mod ...`, or `pub use ...` in `exposedGroups`.
- Do not expose single-field tuple wrappers such as `pub struct ModuleId(pub String);` unless they have meaningful behavior beyond wrapping.
- Create one exposed element per public definition; never combine names with `/`, commas, or summary labels.
- Use stable IDs based on source path plus symbol name, for example `src/overview/star_map.rs#starmapprovider`.
- Use `shape: "triangle"` for traits, `shape: "square"` for structs/classes, `shape: "circle"` for functions, and `shape: "star"` for enums, type aliases, constants, statics, and other small definitions.
- For struct exposed elements, keep the struct as the selectable star-map unit and list related impl/method information in `implements`; avoid scattering every impl method unless it is a standalone public function/API.

## Status Rules

- Include `changeStatus` when known on modules, child modules, files, file items, and exposed elements.
- Valid statuses are `unchanged`, `added`, `modified`, `deleted`, and `renamed`.
- Prefer explicit item-level `changeStatus`; the overview UI can infer modified status from `sourcePath` only as a fallback.
- Status values drive overview sorting, badges, side borders, and star-map outlines.
- Source-editing tasks should trigger `development-designer` immediately with changed source paths and changed portions. Do not defer design refresh to `git-sync`.
- When explicit tag comparison data is available, use it as evidence. Otherwise update statuses from the current task's source changes and preserve unaffected definitions as `unchanged`.

## Update Workflow

- When source structure or public interfaces change, update the design document in the affected `src/` folder.
- Keep module-level purpose, file purpose, item details, and exposed group details aligned.
- If migrating from `.design.json` to `.design.md`, keep the JSON payload schema-compatible so the overview parser can read it without custom prose parsing.

## Lessons

- 2026-06-19: Do not mark current source definitions `added` merely because `.design.md` files are newly generated from `.design.json`; when previous-tag comparison data is unavailable, default checked-in source modules, files, items, and exposed elements to `unchanged` until a real add/modify/delete/rename is known. Source: correction request for green/added star-map elements.
- 2026-06-19: Design refresh belongs to the source-editing task, not to `git-sync`. When files under `src/` are changed, the caller should invoke `development-designer` with the changed paths/sections so `.design.md` stays in sync before any later commit or sync.
