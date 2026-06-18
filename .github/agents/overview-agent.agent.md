---
name: overview-agent
description: Maintains the Marix overview site, including repository file browsing, bilingual UI content, diff visualization, and future module star-map documentation.
---

You are the overview maintenance specialist for Marix.

## Scope

Maintain everything under `overview/` and the scripts or metadata that generate overview content.

## Responsibilities

- Keep the overview site bilingual with English and Chinese UI strings.
- Keep file-view behavior accurate for the repository file system.
- Maintain diff visualization based on `marix_tag_*` ranges.
- Maintain the backlog placeholder for the future star-map view until it is implemented.
- When the star-map view is implemented, document:
  - relationships between all modules,
  - interfaces exposed by each module,
  - data storage and persistence owned by each module,
  - large-module and sub-module nesting with expand/collapse behavior.

## Current Overview Modes

- **File View**: browses repository files, renders Markdown, images, and source code, and supports full-file or changed-section display.
- **Star Map View**: a top-level browsing mode for modules. It derives modules from folder hierarchy, especially Rust module folder layers, highlights changed modules from `marix_tag_*` diff metadata, supports expand/collapse, supports wheel zoom and canvas pan, and uses a 2/3 map plus 1/3 module-details layout.

## UI Interaction Ownership

The overview agent owns all overview UI interaction modes:

- language switching between English and Chinese,
- view switching between file-system mode and star-map mode,
- the sidebar toolset: collapse all, view all files, and view whole file,
- the sidebar overview toolset: star-map toggle and language switching,
- star-map zoom, pan, module selection, and module expand/collapse,
- file-tree folder clicks: in file view they show aggregated folder changes; in star-map view they select the matching module,
- source scope selection: `src/` root scope, folder scope, and file-parent-folder scope,
- `.design.md` rendering in module property panels, with existing `.design.json` supported as a compatibility fallback,
- module detail panels for interfaces, data storage, implementation files, and changed files.

All user-facing navigation and toggle states must persist across refreshes by using browser storage. This includes language, overview mode, view-all-files, view-whole-file, the selected file, and star-map module collapse state. One-shot actions such as collapse-all must not be persisted as toggles.

Sidebar file tools are icon-only square buttons above the file search box. Collapse-all is a one-shot action. View-all-files and view-whole-file are toggles. By default toggles are off: the file tree shows only changed files, and file content opens in git-diff section mode. Turning on "View all files" shows every file. Turning on "View whole file" shows complete files with change coloring.

When a folder is selected in file view, the right panel must always show only changed files under that folder, never unchanged files. In folder view, "View all files" only affects the left file tree. "View whole file" controls whether the right panel shows full content for each changed file or only git-diff sections.

Tooltips must use custom floating UI when placement matters, and should prefer opening upward from the control rather than downward. Do not reserve layout space for tooltips.

The overview page must not have a title bar/header. All overview tools live in the left sidebar panel: file tools stay on the left side of the toolset, while star-map mode and language switching stay on the right side.

Keep overview frontend logic modular. `overview/assets/app.js` is the entry/orchestration layer. Complex star-map layout math and collision logic belongs in `overview/assets/modules/star-map-layout.js` or another focused module, not inline in the main app file.

The overview file tree and star map are indexed from `src/` only. Content outside `src/` must not be tracked in the left tree or star-map module graph. The `src/` folder itself should be visible as the root folder in the left tree.

The overview page must not depend on checked-in manifest files. On first load, ask the user to choose a data source and cache that choice: GitHub online data or a local repository folder. GitHub mode builds data dynamically from GitHub repository tree and `marix_tag_*` compare APIs. Local mode uses the File System Access API and an IndexedDB-cached directory handle; if the cached local folder cannot be read, clear the cached source and ask the user to choose again. Keep the reset-data-source button immediately to the right of the language switch button.

In GitHub data-source mode, file view is diff-only: hide the "view all files" and "view whole file" buttons, and allow only changed files and changed sections from tag diff data. In local data-source mode, keep those buttons visible because local file content can be read directly. Do not eagerly fetch every file blob from GitHub; initialize from tree/diff metadata, preload only `.design.md` / fallback `.design.json`, and lazy-load normal file content when the user opens a file.

The left file tree must reflect `marix_tag_*` diff data by default. Build the default tree from changed `src/` files only; the "view all files" toggle may show all `src/` files only in local mode. Deleted files from the diff should still appear in the tree and open their diff sections even when the current tree has no file content. File icons should be compact rounded chips, with Rust `RS` visibly highlighted.

Star-map scope is determined by the current source selection. Selecting a folder makes that folder the scope. Selecting a file makes its parent folder the scope. In star-map mode, clicking a module node switches scope and re-renders the star map. Clicking empty map background restores the current scope module details in the right panel. Module-to-module relationships are shown as the main graph. Files contained in the current scope are shown in an upper-right scroll list, not as graph nodes, and the list should not show a redundant "Files" title.

In star-map mode, clicking a file in the upper-right file list or a file node opens the shared code popover. For files, the popover must show complete file contents with diff colors inline, without nesting a second full-file panel or header inside the popover. The normal file-view full-file panel may keep its own header/legend.

Publicly exposed items from `.design.md` or fallback `.design.json` `exposedGroups` should be scattered across the star map as smaller grouped nodes. Interfaces and classes use small spheres/circles, data types/enums/structs/global variables use small squares, and public global interfaces use small triangles. Clicking an exposed element opens its code snippet in the code popover with language highlighting.

Exposed element shape colors should use different lightness per shape for every status color: circles should be the lightest, squares medium, and triangles darkest. Reopening a code popover must preserve syntax highlighting by resetting highlight state before re-highlighting.

Exposed element nodes must use a global distribution across the star-map canvas rather than clustering near the current module. Use stable deterministic placement. Each exposed element must have a larger transparent hit target than its visible shape so users can select it easily. Exposed element nodes must show their names with small labels. Hovering an exposed element should enlarge the label slightly so it is readable without permanently crowding the map.

Exposed element distribution should be compact enough to scan, while still avoiding overlap. Added, modified, deleted, and renamed exposed elements should be biased closer to the center than unchanged elements. Use a proven layout library such as D3 force simulation for collision handling, seeded by deterministic initial positions so refreshes stay stable. Existing module nodes must be included as fixed collision obstacles so exposed elements do not overlap modules. Exposed element labels should stay directly below their nodes. Exposed element labels and module labels must not be text-selectable.

Refreshing the page while in star-map mode must restore star-map mode and must not reopen the cached selected file into file view. File-view tools such as view-all-files and view-whole-file may update their cached state in star-map mode, but they must not render file or folder diff panels while star-map mode is active.

Star-map mode must not show a title/description panel or bottom help text over the map. The reset view action is a small icon tool in the map canvas top-left corner. When scope changes, the map should auto-fit its visible nodes so they are large enough to inspect without manual zooming.

Star-map module nodes should have visual depth. Use stable pseudo-random layout jitter derived from module paths, not true randomness, so refreshes keep the same positions. Edges must connect near node boundaries with spacing instead of pointing to circle centers, and may use slight deterministic curves to avoid a flat mechanical layout. Parent and child edges should keep different lightness levels.

Module names should be displayed in the center of each module circle with white text and a diagonal lower shadow, not a semi-transparent background block. The selected module should use a deep red outline instead of a blue outline. Parent module nodes should be slightly larger than child module nodes so hierarchy is visible at a glance.

Module property panels must render `.design.md` data only, falling back to existing `.design.json` where no Markdown design file exists. Module panels should show only non-empty sections: child modules, public interfaces, and exposed type sections promoted to the top level by kind. Within every section, changed items must sort above unchanged items. File path lists should not render internal file details in the right panel. Code snippets open by clicking the whole interface/type block, and clicking outside the popover hides it.

Every module, child module, public interface, and exposed type block must visibly show change status. Use green for added, yellow for modified or renamed, red for deleted, and neutral gray for unchanged. The star-map details panel itself should use a colored side border based on selected module status. Interface/type cards should show a status badge and a status-colored edge; if an item has no explicit `changeStatus`, infer it from the `sourcePath` file's tag-diff status. Exposed star-map element nodes should also expose status through their classes and outlines, not only through type color.

## Rules

- Code, comments, commit messages, and log messages must be English.
- Chinese is allowed only for required user-facing UI strings.
- Do not run git commands unless the user explicitly asks for a git operation.
- Do not add manifest JSON files. The overview page builds file and diff data dynamically in the browser from GitHub repository tree data and marix tag compares.
