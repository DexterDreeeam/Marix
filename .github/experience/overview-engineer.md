# overview-engineer experience — Marix

## Implementation gotchas

- Source/meta mapping is centralized in `app-config.js`: `SOURCE_META_ROOT`, `designDocPathForModule`, `workflowDocPathForModule`, and `moduleKeyFromCompanionPath`. Use those helpers; sorting raw `src_meta/**` paths flips parent/child order compared with module-key sorting.
- Local Git object reading must support packed objects. Packed object inflate output is raw content without a loose-object header, and bounded zlib inflate must stop at the expected object size because trailing bytes can trigger `ERR_TRAILING_JUNK_AFTER_STREAM_END`.
- Dynamic local diffs: hunk `newStart` must equal the first slice entry's `newNo`; do not subtract one for zero-newCount slices. Split both base and current text with `/\r?\n/` so CRLF/LF differences do not become whole-file diffs.
- Star-map state transitions should enter through `applyStarMapState()`. Keep `selectStarMapModule()` and `focusStarMapFile()` as intent wrappers, and read `starMapSelection` in star mode instead of relying on `currentFile` alone.
- Production does not load D3 today. Treat fallback `relaxExposedLayout` as the real star-map layout path; validate edge-corridor and gap-biased placement there before considering the dormant D3 path.
- Code popover/backdrop are anchored under `#viewer`, not the scrolling `#viewer-content`. Put `.star-active` on `#viewer` for CSS anchoring and keep the backdrop wheel guard so underlying SVG/file content cannot scroll.
- Private reveal uses the shared `button.private-code-toggle`: collapsed state stays flat and borderless; the public/private divider belongs only on expanded content.
- The active-tab reload banner is passive. It must not scan files or hash content; the reload action calls `reloadOverviewData()` and preserves cached source and selection when still valid.
- Validation without Node can use `quickjs`: concatenate needed classic scripts behind a sloppy-mode prelude, stub browser globals, skip bootstrap/event auto-init, then call pure functions. With jsdom/Node, eval all app scripts once in index order; separate eval calls do not share lexical scope.
- Temporary `[Marix Overview]` logs help debug scope/selection sync, but remove noisy logs once the state path is stable.
