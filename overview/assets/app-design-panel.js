"use strict";
  let codePopoverFindMatches = [];
  let codePopoverFindIndex = -1;

  function renderModuleDetails(node) {
    if (!node) return;

    const title = document.getElementById("module-detail-title");
    const status = document.getElementById("module-detail-status");
    const body = document.getElementById("module-detail-body");
    const documents = collectDesignDocuments(node.path);
    const primary = getDesignDocumentForModule(node.path) || documents[0] || null;
    const moduleStatus = getModuleDetailStatus(node, primary && primary.document);

    title.textContent = primary ? primary.document.module.path : (node.path || t("rootModule"));
    status.textContent = getModuleDetailStatusLabel(moduleStatus);
    status.className = `module-status ${moduleStatus}`;
    setModuleDetailPanelStatus(moduleStatus);

    resetDesignCodeSnippets();
    const focusedFile = getFocusedFileForModule(node);
    setModuleDetailSelectionState(focusedFile);
    body.innerHTML = renderModuleOverview(node, primary && primary.document, focusedFile);
    if (!focusedFile) clearModuleDetailFileFocus(body);
    bindModuleDetailControls(body);
    logStarMapState("render-module-details", getModuleDetailsDebugSummary(node, focusedFile, body));

    for (const link of body.querySelectorAll(".module-link")) {
      link.addEventListener("click", () => {
        const next = findModule(moduleRoot, link.dataset.modulePath);
        if (next) {
          selectStarMapModule(next.path);
        }
      });
    }
  }

  function renderListSection(title, content) {
    if (!content) return "";
    return `
      <section class="module-detail-section">
        <h4>${escapeHtml(title)}</h4>
        ${content}
      </section>
    `;
  }

  function bindModuleDetailControls(root) {
    bindDesignCodeButtons(root);
  }

  function setModuleDetailSelectionState(focusedFile) {
    const panel = document.querySelector(".star-map-details");
    if (!panel) return;
    panel.dataset.selectionKind = focusedFile ? "file" : "module";
    panel.classList.toggle("file-selection-active", Boolean(focusedFile));
  }

  function clearModuleDetailFileFocus(root) {
    for (const item of root.querySelectorAll(".file-focus-dimmed")) {
      item.classList.remove("file-focus-dimmed");
    }
    for (const list of root.querySelectorAll(".file-focus-list")) {
      list.classList.remove("file-focus-list");
    }
  }

  function renderModuleOverview(node, document, focusedFile) {
    const elements = collectModuleExposedElements(node.path);
    const typeGroups = groupElementsByType(elements);
    const children = collectChildModules(node, document);
    const sections = [];

    if (children.length > 0) {
      sections.push(renderListSection(t("childModules"), renderChildModuleList(children)));
    }
    for (const [type, items] of typeGroups) {
      sections.push(renderListSection(type, renderElementList(items, focusedFile)));
    }

    return sections.join("");
  }

  function renderChildModuleList(children) {
    return children.map(child => `
      <button class="module-link module-link-status-${escapeHtml(child.status)}" data-module-path="${escapeHtml(child.path)}">
        <span>${escapeHtml(child.name)}</span>
      </button>
    `).join("");
  }

  function collectChildModules(node, document) {
    const children = new Map();
    for (const child of (document && document.childModules) || []) {
      children.set(child.path, {
        name: child.name || child.path,
        path: child.path,
        status: normalizeStatus(child.changeStatus)
      });
    }

    for (const child of node.children || []) {
      if (!children.has(child.path)) {
        children.set(child.path, {
          name: child.name || child.path,
          path: child.path,
          status: child.changed ? "modified" : "unchanged"
        });
      }
    }

    return Array.from(children.values()).sort((a, b) => compareChangedFirst(a, b, item => item.status, item => item.path));
  }

  function collectModuleExposedElements(modulePath) {
    const seen = new Set();
    return collectDesignElementGroups(modulePath)
      .flatMap(group => (group.elements || []).map(element => ({
        ...element,
        groupName: group.name
      })))
      .filter(element => {
        const key = element.id || `${getDesignElementPrimarySourcePath(element) || ""}:${element.name || ""}:${getDesignElementType(element)}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .sort(compareElementsForGrouping);
  }

  function groupElementsByType(elements) {
    const groups = new Map();
    for (const element of elements) {
      const type = getElementTypeName(element);
      if (!groups.has(type)) groups.set(type, []);
      groups.get(type).push(element);
    }

    return Array.from(groups.entries())
      .sort(([a], [b]) => getElementTypeRank(a) - getElementTypeRank(b) || a.localeCompare(b))
      .map(([type, items]) => [type, items.sort(compareElementsWithinSection)]);
  }

  function renderElementList(elements, focusedFile) {
    const sorted = elements.slice().sort(compareElementsWithinSection);
    if (focusedFile) {
      const focused = sorted.filter(element => getDesignElementPrimarySourcePath(element) === focusedFile);
      const others = sorted.filter(element => getDesignElementPrimarySourcePath(element) !== focusedFile);
      const visibleOtherCount = Math.max(0, 4 - focused.length);
      const visibleOthers = others.slice(0, visibleOtherCount);
      const hiddenOthers = others.slice(visibleOtherCount);
      const visibleRows = focused
        .map(element => renderElementSummary(element, false))
        .concat(visibleOthers.map(element => renderElementSummary(element, true)))
        .join("");
      const hiddenRows = hiddenOthers.map(element => renderElementSummary(element, true)).join("");
      const hiddenContent = hiddenRows ? `<div class="section-extra-items">${hiddenRows}</div>` : "";
      const more = hiddenRows ? `<div class="design-section-more" aria-hidden="true">...</div>` : "";
      return `
        <div class="design-summary-list file-focus-list">
          ${visibleRows}
          ${hiddenContent}
          ${more}
        </div>
      `;
    }

    const visibleCount = getCollapsedVisibleCount(sorted);
    const hiddenCount = Math.max(0, sorted.length - visibleCount);
    const visibleRows = sorted.slice(0, visibleCount).map(renderElementSummary).join("");
    const hiddenRows = sorted.slice(visibleCount).map(element => renderElementSummary(element, false)).join("");
    const hiddenContent = hiddenRows
      ? `<div class="section-extra-items">${hiddenRows}</div>`
      : "";
    const more = hiddenCount > 0 ? `<div class="design-section-more" aria-hidden="true">...</div>` : "";
    return `
      <div class="design-summary-list">
        ${visibleRows}
        ${hiddenContent}
        ${more}
      </div>
    `;
  }

  function renderElementSummary(element, dimmed) {
    const typeClass = getElementTypeClass(element);
    const status = getElementStatus(element);
    const codeSegments = getDesignElementCodeSegments(element);
    const codeId = codeSegments.length ? storeDesignCodeSnippet(getCodeTitle(element), codeSegments, status) : "";
    const openAttrs = codeId ? ` tabindex="0" data-code-id="${escapeHtml(codeId)}"` : "";
    return `
      <article class="design-summary-item design-type-${escapeHtml(typeClass)} design-status-${escapeHtml(status)}${dimmed ? " file-focus-dimmed" : ""}"${openAttrs}>
        <div class="design-summary-header">
          <strong>${escapeHtml(element.name || "")}</strong>
        </div>
      </article>
    `;
  }

  function getElementTypeName(element) {
    const value = getDesignElementType(element).replace(/-/g, " ").trim();
    return value ? value.charAt(0).toUpperCase() + value.slice(1) : "Item";
  }

  function getElementTypeRank(type) {
    const value = String(type || "").toLowerCase().replace(/\s+/g, "-");
    const ranks = {
      trait: 0,
      struct: 1,
      class: 1,
      function: 2,
      fn: 2,
      method: 2,
      enum: 3,
      "type-alias": 4,
      alias: 4,
      const: 5,
      static: 5,
      global: 5,
      "global-variable": 5
    };
    return ranks[value] ?? 20;
  }

  function getCollapsedVisibleCount(elements) {
    const changedCount = elements.filter(element => getStatusRank(getElementStatus(element)) < 10).length;
    return Math.max(4, changedCount);
  }

  function getElementTypeClass(element) {
    return getDesignElementType(element)
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "item";
  }

  function getElementSource(element) {
    const segments = getDesignElementCodeSegments(element);
    if (segments.length === 0) return "";
    const first = segments[0];
    return `${first.sourcePath}:${first.lineStart}-${first.lineEnd}`;
  }

  function getModuleDetailStatus(node, document) {
    const fromDesign = getExplicitChangeStatus(document && document.module);
    if (fromDesign) return fromDesign;
    return node.changed ? "modified" : "unchanged";
  }

  function getModuleDetailStatusLabel(status) {
    const labels = {
      added: "statusAdded",
      modified: "statusModified",
      renamed: "statusRenamed",
      deleted: "statusDeleted",
      unchanged: "unchanged"
    };
    return t(labels[status] || "unchanged");
  }

  function setModuleDetailPanelStatus(status) {
    const panel = document.querySelector(".star-map-details");
    if (!panel) return;
    panel.classList.remove(
      "module-detail-status-added",
      "module-detail-status-modified",
      "module-detail-status-renamed",
      "module-detail-status-deleted",
      "module-detail-status-unchanged"
    );
    panel.classList.add(`module-detail-status-${status}`);
  }

  function compareElementsForGrouping(a, b) {
    const typeCompare = getElementTypeName(a).localeCompare(getElementTypeName(b));
    if (typeCompare !== 0) return typeCompare;
    return compareElementsWithinSection(a, b);
  }

  function compareElementsWithinSection(a, b) {
    return compareChangedFirst(a, b, getElementStatus, item => String(item.name || ""));
  }

  function compareChangedFirst(a, b, getStatus, getName) {
    const statusCompare = getStatusRank(getStatus(a)) - getStatusRank(getStatus(b));
    if (statusCompare !== 0) return statusCompare;
    return getName(a).localeCompare(getName(b));
  }

  function getStatusRank(status) {
    const value = normalizeStatus(status);
    if (value === "unchanged") return 10;
    const ranks = { added: 0, modified: 1, renamed: 2, deleted: 3 };
    return ranks[value] ?? 9;
  }

  function getElementStatus(element) {
    const fromDesign = getExplicitChangeStatus(element);
    if (fromDesign) return fromDesign;

    const sourcePath = getDesignElementPrimarySourcePath(element);
    return sourcePath ? normalizeStatus(getPathChangeStatus(sourcePath)) : "unchanged";
  }

  function getExplicitChangeStatus(item) {
    if (!item || typeof item.changeStatus !== "string" || item.changeStatus.trim() === "") return null;
    return normalizeStatus(item.changeStatus);
  }

  function getFocusedFileForModule(node) {
    if (!node || starMapSelection.kind !== "file") return "";
    const focusedFile = starMapSelection.path;
    return node.path && (focusedFile === node.path || focusedFile.startsWith(`${node.path}/`)) ? focusedFile : "";
  }

  function getModuleDetailsDebugSummary(node, focusedFile, body) {
    return {
      module: node && node.path,
      focusedFile,
      sections: body.querySelectorAll(".module-detail-section").length,
      rows: body.querySelectorAll(".design-summary-item").length,
      dimmedRows: body.querySelectorAll(".design-summary-item.file-focus-dimmed").length,
      hiddenGroups: body.querySelectorAll(".section-extra-items").length
    };
  }

  function resetDesignCodeSnippets() {
    designCodeSnippets = new Map();
    designCodeCounter = 0;
  }

  function storeDesignCodeSnippet(title, segments, status = "unchanged") {
    const id = `code-${++designCodeCounter}`;
    designCodeSnippets.set(id, { title, segments, status });
    return id;
  }

  function getCodeTitle(item) {
    return item.name || "code";
  }

  function bindDesignCodeButtons(root) {
    for (const item of root.querySelectorAll(".design-summary-item[data-code-id]")) {
      item.addEventListener("click", async evt => {
        evt.stopPropagation();
        const snippet = designCodeSnippets.get(item.dataset.codeId);
        if (snippet) await showCodeSegmentsPopover(snippet.title, snippet.segments, snippet.status);
      });
      item.addEventListener("keydown", evt => {
        if (evt.key !== "Enter" && evt.key !== " ") return;
        evt.preventDefault();
        item.click();
      });
    }
  }

  async function showCodeSegmentsPopover(title, segments, status = "unchanged") {
    const blocks = [];
    for (const segment of segments) {
      const entry = await ensureFileContent(segment.sourcePath);
      const content = (entry && entry.content) || "";
      const lines = content.split(/\r?\n/).slice(segment.lineStart - 1, segment.lineEnd);
      const languageName = segment.language || getLanguageFromExt(segment.sourcePath.split(".").pop().toLowerCase());
      blocks.push(renderCodeSegment(segment, lines, languageName, status));
    }
    showCodePopover(title, blocks.join(""), "", "code-popover-file");
  }

  function renderCodeSegment(segment, lines, languageName, elementStatus) {
    const label = `${segment.sourcePath}:${segment.lineStart}-${segment.lineEnd}`;
    const status = normalizeStatus(segment.changeStatus || elementStatus);
    const body = lines.map((line, index) => {
      const lineNumber = segment.lineStart + index;
      return renderCodeSegmentLine(lineNumber, line, getCodeSegmentLineClass(segment, status, lineNumber), languageName);
    }).join("");
    return `
      <section class="code-segment-panel">
        <div class="code-segment-label">${escapeHtml(label)}</div>
        <div class="full-file-lines full-file-lines-embedded">${body}</div>
      </section>
    `;
  }

  function renderCodeSegmentLine(lineNumber, content, lineClass, languageName) {
    return `
      <div class="full-file-line ${lineClass}">
        <span class="full-line-number">${lineNumber}</span>
        <span class="full-line-content">${highlightLine(content, languageName)}</span>
      </div>
    `;
  }

  function getCodeSegmentLineClass(segment, status, lineNumber) {
    if (status === "added") return "full-line-add";
    if (isLineInCodeSegmentRanges(lineNumber, segment.addedLines)) return "full-line-add";
    if (isLineInCodeSegmentRanges(lineNumber, segment.modifiedLines)) return "full-line-modified";
    return "full-line-existing";
  }

  function isLineInCodeSegmentRanges(lineNumber, ranges) {
    return (ranges || []).some(range => lineNumber >= range.lineStart && lineNumber <= range.lineEnd);
  }

  function showCodePopover(title, contentHtml, languageName, contentClass = "code-popover-code") {
    const backdrop = document.getElementById("code-popover-backdrop");
    const popover = document.getElementById("code-popover");
    const codeEl = document.getElementById("code-popover-content");
    document.getElementById("code-popover-title").textContent = title;
    resetCodePopoverFindState();
    codeEl.className = `code-popover-content ${contentClass}`;
    codeEl.removeAttribute("data-highlighted");
    if (languageName) codeEl.classList.add(`language-${languageName}`);
    codeEl.innerHTML = languageName ? highlightSource(contentHtml, languageName) : contentHtml;
    backdrop.style.display = "block";
    popover.style.display = "flex";
  }

  async function showFilePopover(path) {
    const entry = await ensureFileContent(path);
    const change = getChange(path) || { diff_lines: [], hunks: [] };
    const ext = path.split(".").pop().toLowerCase();
    const backdrop = document.getElementById("code-popover-backdrop");
    const popover = document.getElementById("code-popover");
    const codeEl = document.getElementById("code-popover-content");
    document.getElementById("code-popover-title").textContent = path;
    resetCodePopoverFindState();
    codeEl.className = "code-popover-content code-popover-file";
    codeEl.removeAttribute("data-highlighted");
    codeEl.innerHTML = renderFullFilePanel(path, (entry && entry.content) || "", change, ext, { embedded: true });
    backdrop.style.display = "block";
    popover.style.display = "flex";
  }

  function hideCodePopover() {
    document.getElementById("code-popover-backdrop").style.display = "none";
    document.getElementById("code-popover").style.display = "none";
    resetCodePopoverFindState();
  }

  function bindCodePopoverFindEvents() {
    const input = document.getElementById("code-popover-find-input");
    const previousButton = document.getElementById("btn-code-popover-find-prev");
    const nextButton = document.getElementById("btn-code-popover-find-next");
    input.addEventListener("input", updateCodePopoverFind);
    input.addEventListener("keydown", evt => {
      if (evt.key !== "Enter") return;
      evt.preventDefault();
      moveCodePopoverFindSelection(evt.shiftKey ? -1 : 1);
    });
    previousButton.addEventListener("click", () => moveCodePopoverFindSelection(-1));
    nextButton.addEventListener("click", () => moveCodePopoverFindSelection(1));
  }

  function handleCodePopoverFindShortcut(evt) {
    if (!(evt.ctrlKey || evt.metaKey) || evt.shiftKey || evt.altKey || evt.key.toLowerCase() !== "f") return false;
    if (!isCodePopoverVisible()) return false;
    evt.preventDefault();
    focusCodePopoverFind();
    return true;
  }

  function focusCodePopoverFind() {
    const input = document.getElementById("code-popover-find-input");
    input.focus();
    input.select();
  }

  function isCodePopoverVisible() {
    return document.getElementById("code-popover").style.display !== "none";
  }

  function resetCodePopoverFindState() {
    const input = document.getElementById("code-popover-find-input");
    const content = document.getElementById("code-popover-content");
    if (content) clearCodePopoverFindHighlights(content);
    if (input) input.value = "";
    codePopoverFindMatches = [];
    codePopoverFindIndex = -1;
    updateCodePopoverFindCount();
  }

  function updateCodePopoverFind() {
    const input = document.getElementById("code-popover-find-input");
    const content = document.getElementById("code-popover-content");
    const query = input.value;
    clearCodePopoverFindHighlights(content);
    codePopoverFindMatches = [];
    codePopoverFindIndex = -1;
    if (!query) {
      updateCodePopoverFindCount();
      return;
    }

    const textNodes = collectCodePopoverFindTextNodes(content, query);
    for (const textNode of textNodes) {
      highlightCodePopoverTextNode(textNode, query);
    }
    if (codePopoverFindMatches.length > 0) {
      codePopoverFindIndex = 0;
      applyCodePopoverFindSelection({ scroll: true });
    }
    updateCodePopoverFindCount();
  }

  function collectCodePopoverFindTextNodes(root, query) {
    const nodes = [];
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        if (!node.nodeValue || !node.nodeValue.includes(query)) return NodeFilter.FILTER_REJECT;
        const parent = node.parentElement;
        if (!parent || parent.closest(".full-line-number")) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      }
    });
    while (walker.nextNode()) nodes.push(walker.currentNode);
    return nodes;
  }

  function highlightCodePopoverTextNode(textNode, query) {
    const text = textNode.nodeValue;
    let cursor = 0;
    let lastAppended = 0;
    let foundMatch = false;
    const fragment = document.createDocumentFragment();
    while (cursor < text.length) {
      const index = text.indexOf(query, cursor);
      if (index === -1) break;
      const end = index + query.length;
      if (!isWholeWordCodePopoverFindMatch(text, query, index, end)) {
        cursor = index + 1;
        continue;
      }
      fragment.append(document.createTextNode(text.slice(lastAppended, index)));
      const match = document.createElement("mark");
      match.className = "code-popover-find-match";
      match.textContent = text.slice(index, end);
      fragment.append(match);
      codePopoverFindMatches.push(match);
      cursor = end;
      lastAppended = end;
      foundMatch = true;
    }
    if (!foundMatch) return;
    fragment.append(document.createTextNode(text.slice(lastAppended)));
    textNode.replaceWith(fragment);
  }

  function isWholeWordCodePopoverFindMatch(text, query, start, end) {
    const first = query.charAt(0);
    const last = query.charAt(query.length - 1);
    const before = start > 0 ? text.charAt(start - 1) : "";
    const after = end < text.length ? text.charAt(end) : "";
    const beforeAllowed = !isCodePopoverWordCharacter(first) || before === "" || !isCodePopoverWordCharacter(before);
    const afterAllowed = !isCodePopoverWordCharacter(last) || after === "" || !isCodePopoverWordCharacter(after);
    return beforeAllowed && afterAllowed;
  }

  function isCodePopoverWordCharacter(character) {
    if (!character) return false;
    const code = character.charCodeAt(0);
    return (code >= 48 && code <= 57) || (code >= 65 && code <= 90) || (code >= 97 && code <= 122) || character === "_";
  }

  function clearCodePopoverFindHighlights(root) {
    if (!root) return;
    for (const match of Array.from(root.querySelectorAll(".code-popover-find-match"))) {
      const parent = match.parentNode;
      match.replaceWith(document.createTextNode(match.textContent || ""));
      if (parent) parent.normalize();
    }
  }

  function moveCodePopoverFindSelection(delta) {
    if (codePopoverFindMatches.length === 0) return;
    codePopoverFindIndex = (codePopoverFindIndex + delta + codePopoverFindMatches.length) % codePopoverFindMatches.length;
    applyCodePopoverFindSelection({ scroll: true });
    updateCodePopoverFindCount();
  }

  function applyCodePopoverFindSelection({ scroll } = {}) {
    for (const match of codePopoverFindMatches) match.classList.remove("current");
    const current = codePopoverFindMatches[codePopoverFindIndex];
    if (!current) return;
    current.classList.add("current");
    if (scroll) current.scrollIntoView({ block: "center", inline: "nearest" });
  }

  function updateCodePopoverFindCount() {
    const count = document.getElementById("code-popover-find-count");
    if (!count) return;
    if (codePopoverFindMatches.length === 0) {
      count.textContent = document.getElementById("code-popover-find-input")?.value ? "0/0" : "";
      return;
    }
    count.textContent = `${codePopoverFindIndex + 1}/${codePopoverFindMatches.length}`;
  }

  function getDesignDocumentForModule(modulePath) {
    const basePath = modulePath || SOURCE_ROOT;
    return parseDesignDocument(`${basePath}/.design.json`);
  }

  function collectDesignDocuments(modulePath) {
    const prefix = modulePath ? `${modulePath}/` : "";
    const preferredDocuments = new Map();
    for (const [path] of Object.entries(manifest.files || {})
      .filter(([path]) => isDesignDocumentPath(path))
      .filter(([path]) => !prefix || path.startsWith(prefix))) {
      const moduleKey = path.replace(/\/\.design\.json$/i, "");
      preferredDocuments.set(moduleKey, path);
    }

    return Array.from(preferredDocuments.values())
      .map(path => parseDesignDocument(path))
      .filter(Boolean)
      .sort((a, b) => a.path.localeCompare(b.path));
  }

  function isDesignDocumentPath(path) {
    return path.endsWith("/.design.json");
  }

  function parseDesignDocument(path) {
    const data = (manifest.files || {})[path];
    if (!data || !data.content) return null;

    try {
      return { path, document: parseDesignDocumentContent(data.content) };
    } catch (e) {
      return null;
    }
  }

  function parseDesignDocumentContent(content) {
    const trimmed = String(content || "").trim();
    if (trimmed.startsWith("{")) return JSON.parse(trimmed);

    const fencedJson = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/i);
    if (fencedJson) return JSON.parse(fencedJson[1].trim());

    return JSON.parse(trimmed);
  }

  function findModule(node, path) {
    if (node.path === path) return node;
    for (const child of node.children || []) {
      const found = findModule(child, path);
      if (found) return found;
    }
    return null;
  }

  function escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text == null ? "" : String(text);
    return div.innerHTML;
  }
