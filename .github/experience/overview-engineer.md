# overview-engineer Experience

## Purpose

Persistent implementation notes for the Marix overview site under `overview/`.

## Current UX Contract

- The overview site indexes `src/` only. Content outside `src/` must not appear in the file tree or star-map module graph.
- Do not add checked-in manifest JSON files. Data is built dynamically in the browser.
- First load asks users to choose a data source:
  - **GitHub**: build metadata from GitHub tree and `marix_tag_*` compare APIs.
  - **Local folder**: use File System Access API and IndexedDB-cached directory handle.
- If the cached local folder cannot be read, clear the cached source and ask again.
- Keep the reset-data-source button immediately to the right of the language switch button.
- GitHub mode is diff-only: hide view-all-files and view-whole-file controls.
- Local mode can show all files and full file contents.
- Default file tree is built from changed `src/` files from tag diff.
- Deleted files in diff should still be visible and open diff sections.
- Rust file icons use compact rounded `RS` chips.

## Star Map UX

- Star-map mode restores across refreshes.
- The map has no title/help overlay.
- Reset view is a small icon in the map canvas top-left.
- Files in the current module appear in the upper-right file list.
- Clicking a file in the star-map file list opens the shared popover with complete file contents and inline diff colors.
- File popovers must not nest a second full-file panel/header inside the shared code popover.
- Clicking exposed elements opens the code popover and resets highlight state before syntax highlighting.
- Exposed elements show labels below nodes; hover may enlarge labels.
- Layout uses deterministic positions and D3 force collision when available.

## Detail Panel UX

- The right module panel shows non-empty sections only.
- Sections: child modules, public interfaces, and exposed type sections promoted by kind.
- Changed items sort above unchanged items inside every section.
- Modules, child modules, interfaces, and exposed types all show change status.
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
- Avoid eager GitHub blob fetches; preload only `.design.md` / fallback `.design.json`, lazy-load normal file content.
- Local file content can be read lazily from stored `FileSystemFileHandle`.
- Keep all UI text bilingual through `I18N` in `app-state.js`.
