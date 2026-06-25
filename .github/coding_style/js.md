# JavaScript Coding Style

This document is the JavaScript coding style source for {{proj}}. Use it when
reviewing, drafting, or changing JavaScript code.

## Scope

- Apply these rules to JavaScript source files only.
- Treat the file-size rule as required during code evaluation.
- Treat naming, hygiene, readability, DOM, and API-shape findings as style findings unless a task explicitly asks for automatic cleanup.
- Do not install new tooling to satisfy this document. Use existing repository tooling only.

## File Size

- Keep every evaluated code file at or below **500 lines**.
- If a file exceeds 500 lines, split it into focused modules or extract shared logic.
- Prefer cohesive modules over arbitrary line-count splitting.
- Generated, build, vendor, cache, and manifest artifacts are outside normal style review unless explicitly requested.

## Formatting and Tooling

- Prefer existing formatter, linter, build, or test scripts when present.
- Do not add ESLint, Prettier, TypeScript, bundlers, or test frameworks only to satisfy style checks.
- If no JavaScript tooling exists, use static review and browser-compatible syntax checks available in the environment.
- Keep browser code dependency-light unless the repository already owns the dependency path.

## Naming

Use English, descriptive, stable terminology. Avoid abbreviations and single-letter
names except for small callback parameters or established DOM/event names.

| Item | Convention | Example |
|------|------------|---------|
| Files and folders | `lower-kebab-case` | `app-design-panel.js` |
| Classes and constructor-like types | `UpperCamelCase` | `RuntimeState` |
| Interfaces and type-like shapes | `IUpperCamelCase` | `IRuntime` |
| Functions and methods | `lowerCamelCase` | `renderModuleDetails` |
| Function parameters | `lowerCamelCase` | `inputParameter1` |
| Local variables | `_` + `lowerCamelCase` for new style-sensitive code | `_messageBytes` |
| Constants | `c_` + `lowerCamelCase` | `c_maxPayloadBytes` |
| Module-level singletons and static-like values | `s_` + `lowerCamelCase` | `s_defaultRuntime` |
| Object member fields that model owned state | `m_` + `lowerCamelCase` | `m_chatText` |
| Globals intentionally attached to shared scope | `g_` + `lowerCamelCase` | `g_runtimeState` |
| DOM element variables | descriptive `lowerCamelCase` | `searchInput` |
| Event variables | `evt` | `evt` |

## DOM and Browser Code

- Bind event listeners from one clear initialization path.
- Keep DOM queries close to the feature they serve unless shared state already owns them.
- Use `textContent` for plain text and escape HTML before inserting untrusted or file-derived content.
- Keep keyboard shortcuts scoped to the visible surface they control.
- Avoid leaking UI state across hidden or destroyed panels.

## Error Handling

- Do not swallow errors silently.
- Surface unexpected states through existing notification, logging, or visible UI patterns.
- Avoid broad `try` / `catch` blocks that convert failures into success-shaped fallbacks.
- Prefer explicit guards and clear failure paths.

## Production Hygiene

Flag these unless locally justified:

- `console.log`
- `console.debug`
- `debugger`
- broad `catch` blocks
- unused variables
- duplicated DOM selectors
- magic strings that should be shared constants

## Readability and API Shape

- Keep functions focused and shallow.
- Avoid deeply nested callbacks and conditionals.
- Extract repeated UI or data-shaping logic into cohesive functions.
- Prefer typed state objects by convention over parallel loose variables when state grows.
- Prefer enums or named mode constants over unclear boolean mode parameters.
- Keep data loading, rendering, event binding, and state mutation boundaries separate.

