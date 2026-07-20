# engineer-of-overview experience — Marix

## Repository data

- Use `app-config.js` mapping helpers for source companions. Sorting raw `src_meta/**` paths reverses parent/child order relative to module-key sorting.
- Local Git readers must support packed objects. Packed inflate output lacks the loose-object header; stop bounded inflation at the declared object size or trailing pack bytes can raise `ERR_TRAILING_JUNK_AFTER_STREAM_END`.
- For dynamic local diffs, set hunk `newStart` from the first slice entry's `newNo`; do not decrement zero-new-count slices. Split base and current text with `/\r?\n/` to avoid CRLF-only whole-file diffs.
- Local auxiliary credentials must be read through the authorized root handle, never indexed as repository data. Keep derived URLs in memory, invalidate them before reset/reload, and resolve them only after a successful source load.

## Layout and browser behavior

- Production currently uses `relaxExposedLayout`, not D3. Validate collision obstacles, edge corridors, and gap-biased placement in that fallback path.
- Anchor code popovers and backdrops under `#viewer`, not scrolling `#viewer-content`; put `.star-active` on `#viewer` and consume backdrop wheel events.
- The active-tab reload banner is passive: do not scan or hash files. Its action calls `reloadOverviewData()` and retains cached source/selection only when still valid.
- Reopening highlighted code requires clearing the highlighter's processed state before applying highlighting again.

## Validation

- Without Node, test pure browser logic with QuickJS by concatenating classic scripts after a sloppy-mode prelude, stubbing browser globals, and suppressing bootstrap. With jsdom, evaluate all scripts once in index order; separate evaluations do not share lexical bindings.
