# overview-engineer Experience

## Purpose

Persistent implementation notes for the Marix overview site under `overview/`.
Overview-engineer owns overview implementation and UX only. Source-design companion metadata is maintained by `development-designer`; overview consumes that data but does not refresh or maintain it, and `git-sync` should not invoke overview-engineer unless overview implementation work is explicitly in scope.

## Current UX Contract

- The overview site indexes `src/` only. Content outside `src/` must not appear in the file tree or star-map module graph.
- Every dot-prefixed file or folder under `src/` is companion metadata maintained by `development-designer`, not first-class source content. Visible file systems, file trees, file lists, module graphs, and `marix_tag_*` diffs must ignore all such paths. Internal metadata loaders may consume companion metadata, but dot-prefixed paths must never appear as normal source files.
- Do not add checked-in manifest JSON files. Data is built dynamically in the browser.
- First load asks users to choose a data source:
  - **GitHub**: build metadata from GitHub tree and `marix_tag_*` compare APIs.
  - **Local folder**: use File System Access API and IndexedDB-cached directory handle.
- If the cached local folder cannot be read, clear the cached source and ask again.
- Keep the reset-data-source button immediately to the right of the language switch button.
- The left file tree always shows all visible `src/` files. There is no view-all-files toggle.
- GitHub mode hides the view-whole-file control; local mode can show full file contents.
- Inside each folder, files changed by `marix_tag_*` diff sort above unchanged files.
- Deleted files in diff should still be visible and open diff sections.
- File entries use a left status dot instead of type text chips: unchanged gray, added green, modified yellow, deleted red, renamed accent. Folder entries use a triangle-like arrow with an inward notch on the base so the arrow head is clear; if all changed descendant files are newly added the arrow is green, if any descendant file is modified/renamed/deleted or mixed then the arrow is yellow, and unchanged folders are gray. Unchanged file names are slightly dimmed.

## Star Map UX

- Star-map mode restores across refreshes.
- Refresh should either restore the cached selected file and mark it active in the left tree, or clear stale file selection and reset scope to `src`.
- Left tree active selection should always match the actual star-map view: active folder for module scope, active file for file focus, including after tree rerender.
- Left tree folder clicks are mode-dependent. In file view mode, clicking a folder always expands/collapses it and opens the folder diff view. In star-map mode, clicking an unselected folder only switches star-map scope to that module and marks it active without auto-expanding it; clicking the already selected folder expands/collapses it.
- The map has no title/help overlay.
- Reset view is a small icon in the map canvas top-left.
- Files in the current module appear in the upper-right file list. File names should be high-contrast and visibly heavier than normal body text because the list sits on top of the graph canvas. When file focus is active, sort the focused file to the top, keep it bright with a stronger row treatment, dim every other file name while keeping it readable, and collapse the list after four visible files behind the same full-height `...` hover/focus expansion pattern used by detail-panel element sections.
- Clicking a file in the star-map file list opens the shared popover with complete file contents and inline diff colors.
- Selecting a file in star-map mode should not change module scope when the file belongs to the current scope. If `scopePath` is unchanged, changing file focus must not reset pan/zoom/fit; it should only update element dimming, right-panel row dimming, and the upper-right file-list ordering/dimming. It focuses the map: keep the file's owning module and elements with matching `sourcePath` bright, dim all other modules/elements. In each right-panel type section, all focused-file elements stay visible even when the current scope is an ancestor module such as `src`; if fewer than four rows are focused, fill visible slots up to four with dimmed non-focused rows, and collapse the rest behind the `...` hover/focus expansion. Dimmed non-focused rows must remain visually dim even when hovered or focused, including muted borders.
- Selecting a module node or child-module row clears file focus so every exposed element in that module scope becomes bright again.
- Selecting a module from the star map or right detail panel should expand and activate the matching folder in the left tree. If the diff-only tree lacks that folder, render the full `src/` tree so selection remains visible.
- The star-map canvas has a bottom-left module-path breadcrumb that grows to the right. It shows at most the four nearest module segments for the current scope, eliding older left-side ancestors when the path is deeper. Every visible segment is clickable and must select that module through the unified module-selection entry.
- The current scope center module keeps a red selected border with no red halo. The border itself should pulse faster as a continuous brightness/stroke-width change to remind users which module is the center of the current scope. Keep the pulse phase anchored to `scopePath`, so pan, zoom, and file-focus rerenders do not restart the animation while scope is unchanged. Respect `prefers-reduced-motion` by disabling the animation.
- Keep star-map scope and selection updates centralized in shared helpers. `scopePath` defines the rendered module range, while `starMapSelection` defines whether the active star-map selection is a module or a file. Star-map dimming, right-panel file focus, and left-tree sync must read `starMapSelection`, not `currentFile`, because `currentFile` is also used by the file view.
- The only star-map state transition entry should be `applyStarMapState()`, with `selectStarMapModule()` and `focusStarMapFile()` as thin intent wrappers. The shared render path is `renderStarMapSelectionState()`, which owns left-tree sync, right detail rendering, star-map rendering, and file popover opening for the current selection.
- Keep temporary `[Marix Overview]` state logs available while debugging scope/selection sync. Useful events include `select-module:*`, `focus-file:*`, `canvas-pointerdown`, `mark-file-focus:*`, `render-star-map`, `render-module-details`, and tree sync events.
- Canvas/blank clicks in the star map should clear file focus even when the pointer target is the SVG layer or an edge, not only when `event.target === svg`. Ignore only actual `.star-node` and `.exposed-node` targets.
- File popovers must not nest a second full-file panel/header inside the shared code popover.
- Clicking exposed elements opens the code popover and resets highlight state before syntax highlighting.
- Code highlighting should use common dark editor colors: keywords purple, type/class names teal, variables/properties light blue, function names orange, strings orange-brown, comments green.
- Exposed element names belong above their nodes; hover may enlarge labels.
- Exposed element status colors cover the whole visible shape, not the outline. Unchanged elements keep type fill colors: trait deep-pink triangle, struct larger light-blue square, function/fn purple circle, enum/type-alias/const/static/global dark-blue star. Added/modified/renamed/deleted elements use green/yellow/yellow/red fills; outlines stay type-colored and stable.
- Current exposed element type mapping: trait = deep-pink triangle, struct = larger light-blue square, function/fn = purple circle, enum/type-alias/const/static/global = small dark-blue five-point star.
- Exposed elements should keep the deterministic compact radial distribution and use auto-fit zoom to fill the canvas; avoid forced rectangular scatter because it looks noisy. Enum/star borders should be subtle, not bright white.
- Star-map manual zoom should allow deep zoom-in for close inspection. Browser/container resize must not recompute layout or visually resize elements; instead adjust the star-map transform scale/translation to preserve on-screen element size while `scopePath` and layout stay unchanged.
- Layout uses deterministic positions and D3 force collision when available.
- Unchanged module nodes should render as white/light gray; reserve green/yellow/red module fills for actual status changes.
- Module labels belong inside the module circle, with the existing white text plus shadow treatment.

## Detail Panel UX

- The right module panel shows non-empty sections only.
- The right module panel should stay narrow because rows show names only; keep it around 300px wide rather than using a wide detail layout.
- Sections: child modules plus exposed element sections promoted by exact kind. Trait and Function must be separate sections; do not merge them into Public interfaces.
- Module panel type section order is Trait, Struct, Function, Enum, Alias, then other small definitions.
- Each type section shows four rows by default, but all changed rows remain visible even if that exceeds four. Extra unchanged rows are indicated by `...` and expand automatically on hover/focus, then collapse on pointer out with a smooth medium-speed animation. The ellipsis row must have enough line height for the dots to render as full circles, not clipped halves. Use CSS grid row-height transitions so collapsed rows take no layout space; `display: none/block` looks instant and `max-height` can still reserve awkward space.
- Changed items sort above unchanged items inside every section.
- Module panel child module/interface/type rows should show names only; left edge matches the star-map type color and right edge indicates status. Avoid paths, signatures, source paths, and long details in the panel.
- Modules, child modules, interfaces, and exposed types all show change status. Explicit `changeStatus: "unchanged"` must be respected and must not be overridden by source-file diff status.
- Status colors:
  - added: green,
  - modified / renamed: yellow,
  - deleted: red,
  - unchanged: gray.
- If an exposed item has no explicit `changeStatus`, infer status from its `sourcePath` file diff.
- The selected module detail panel uses a status-colored side border.

## Implementation Notes

- Keep frontend logic modular across `overview/assets/app-*.js`.
- Complex star-map layout and collision code belongs in `overview/assets/modules/star-map-layout.js`.
- Avoid eager GitHub blob fetches; preload only required companion metadata through metadata loaders, and lazy-load normal file content.
- Local file content can be read lazily from stored `FileSystemFileHandle`.
- Keep all UI text bilingual through `I18N` loaded from `app-config.js`.
- 2026-06-19: To satisfy `code-evaluate`, keep overview scripts under 500 lines by moving constants, dynamic data-source loading, local handle cache, code rendering, and star-map exposed-node helpers into focused `app-*.js` files. Preserve `overview/index.html` script order: config before state, code rendering before file/design consumers, and star-map elements before star-map/design consumers.
- 2026-06-19: The Rust crate now lives under `src`; overview should show `src/Cargo.toml` as normal visible source/config, while `src/.cargo/`, `src/.target/`, and `src/target/` stay hidden/ignored by the existing source-root, dot-path, and `target` filters. Verified against `overview/assets/app-data-source.js` and `overview/assets/app-local-source.js`.
