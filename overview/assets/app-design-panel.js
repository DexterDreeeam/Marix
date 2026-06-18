"use strict";
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
    body.innerHTML = renderModuleOverview(node, primary && primary.document);
    bindDesignCodeButtons(body);

    for (const link of body.querySelectorAll(".module-link")) {
      link.addEventListener("click", () => {
        const next = findModule(moduleRoot, link.dataset.modulePath);
        if (next) {
          setScopePath(next.path);
          currentFile = null;
          currentDirectory = next.path;
          requestStarMapFit();
          selectedModule = next;
          renderModuleDetails(next);
          renderStarMap();
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

  function renderModuleOverview(node, document) {
    const elements = collectModuleExposedElements(node.path);
    const publicInterfaces = elements.filter(isInterfaceElement);
    const typeGroups = groupElementsByType(elements.filter(element => !isInterfaceElement(element)));
    const children = collectChildModules(node, document);
    const sections = [];

    if (children.length > 0) {
      sections.push(renderListSection(t("childModules"), renderChildModuleList(children)));
    }
    if (publicInterfaces.length > 0) {
      sections.push(renderListSection(t("publicInterfaces"), renderElementList(publicInterfaces)));
    }
    for (const [type, items] of typeGroups) {
      sections.push(renderListSection(type, renderElementList(items)));
    }

    return sections.join("");
  }

  function renderChildModuleList(children) {
    return children.map(child => `
      <button class="module-link" data-module-path="${escapeHtml(child.path)}">
        <span>${escapeHtml(child.name)}</span>
        <span class="module-link-status ${escapeHtml(child.status)}">${escapeHtml(child.path)}</span>
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
    return collectExposedGroups(modulePath)
      .flatMap(group => (group.elements || []).map(element => ({
        ...element,
        groupName: group.name
      })))
      .filter(element => {
        const key = element.id || `${element.sourcePath || ""}:${element.name || ""}:${element.signature || ""}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .sort(compareElementsForGrouping);
  }

  function isInterfaceElement(element) {
    const category = String(element.category || "").toLowerCase();
    const kind = String(element.kind || "").toLowerCase();
    return category === "interface" || ["trait", "function", "fn", "public-api", "global-interface", "public-global-interface"].includes(kind);
  }

  function groupElementsByType(elements) {
    const groups = new Map();
    for (const element of elements) {
      const type = getElementTypeName(element);
      if (!groups.has(type)) groups.set(type, []);
      groups.get(type).push(element);
    }

    return Array.from(groups.entries()).map(([type, items]) => [type, items.sort(compareElementsWithinSection)]);
  }

  function renderElementList(elements) {
    return `<div class="design-summary-list">${elements.slice().sort(compareElementsWithinSection).map(renderElementSummary).join("")}</div>`;
  }

  function renderElementSummary(element) {
    const code = element.code || element.signature || "";
    const codeId = code ? storeDesignCodeSnippet(getCodeTitle(element), code, element.language || "rust") : "";
    const source = getElementSource(element);
    const typeClass = getElementTypeClass(element);
    const status = getElementStatus(element);
    const openAttrs = codeId ? ` tabindex="0" data-code-id="${escapeHtml(codeId)}"` : "";
    return `
      <article class="design-summary-item design-type-${escapeHtml(typeClass)} design-status-${escapeHtml(status)}"${openAttrs}>
        <div class="design-summary-header">
          <span class="design-item-kind">${escapeHtml(getElementTypeName(element))}</span>
          <strong>${escapeHtml(element.name || element.signature || "")}</strong>
          <span class="design-item-status ${escapeHtml(status)}">${escapeHtml(getModuleDetailStatusLabel(status))}</span>
        </div>
        ${element.signature ? `<code class="design-summary-signature">${escapeHtml(element.signature)}</code>` : ""}
        ${source ? `<span class="design-summary-meta">${escapeHtml(source)}</span>` : ""}
        ${element.details ? `<p>${escapeHtml(element.details)}</p>` : ""}
      </article>
    `;
  }

  function getElementTypeName(element) {
    const value = String(element.kind || element.category || "item").replace(/-/g, " ").trim();
    return value ? value.charAt(0).toUpperCase() + value.slice(1) : "Item";
  }

  function getElementTypeClass(element) {
    return String(element.kind || element.category || "item")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "item";
  }

  function getElementSource(element) {
    if (!element.sourcePath) return "";
    if (element.lineStart && element.lineEnd) return `${element.sourcePath}:${element.lineStart}-${element.lineEnd}`;
    if (element.lineStart) return `${element.sourcePath}:${element.lineStart}`;
    return element.sourcePath;
  }

  function getModuleDetailStatus(node, document) {
    const fromDesign = normalizeStatus(document && document.module && document.module.changeStatus);
    if (fromDesign !== "unchanged") return fromDesign;
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
    return compareChangedFirst(a, b, getElementStatus, item => String(item.name || item.signature || ""));
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
    const fromDesign = normalizeStatus(element.changeStatus);
    if (fromDesign !== "unchanged") return fromDesign;

    const change = element.sourcePath ? getChange(element.sourcePath) : null;
    if (!change) return "unchanged";
    const statusMap = { A: "added", M: "modified", D: "deleted", R: "renamed" };
    return normalizeStatus(statusMap[change.status] || "modified");
  }

  function resetDesignCodeSnippets() {
    designCodeSnippets = new Map();
    designCodeCounter = 0;
  }

  function storeDesignCodeSnippet(title, code, language) {
    const id = `code-${++designCodeCounter}`;
    designCodeSnippets.set(id, { title, code, language });
    return id;
  }

  function getCodeTitle(item) {
    const name = item.name || item.signature || "code";
    if (item.sourcePath && item.lineStart && item.lineEnd) {
      return `${name} — ${item.sourcePath}:${item.lineStart}-${item.lineEnd}`;
    }
    if (item.sourcePath && item.lineStart) {
      return `${name} — ${item.sourcePath}:${item.lineStart}`;
    }
    return name;
  }

  function bindDesignCodeButtons(root) {
    for (const item of root.querySelectorAll(".design-summary-item[data-code-id]")) {
      item.addEventListener("click", evt => {
        evt.stopPropagation();
        const snippet = designCodeSnippets.get(item.dataset.codeId);
        if (snippet) showCodePopover(snippet.title, snippet.code, snippet.language);
      });
      item.addEventListener("keydown", evt => {
        if (evt.key !== "Enter" && evt.key !== " ") return;
        evt.preventDefault();
        item.click();
      });
    }
  }

  function showCodePopover(title, code, languageName) {
    const backdrop = document.getElementById("code-popover-backdrop");
    const popover = document.getElementById("code-popover");
    const codeEl = document.getElementById("code-popover-content");
    document.getElementById("code-popover-title").textContent = title;
    codeEl.className = "code-popover-content code-popover-code";
    codeEl.removeAttribute("data-highlighted");
    codeEl.textContent = code;
    if (languageName && window.hljs && hljs.getLanguage(languageName)) {
      codeEl.classList.add(`language-${languageName}`);
      hljs.highlightElement(codeEl);
    }
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
    codeEl.className = "code-popover-content code-popover-file";
    codeEl.removeAttribute("data-highlighted");
    codeEl.innerHTML = renderFullFilePanel(path, (entry && entry.content) || "", change, ext, { embedded: true });
    backdrop.style.display = "block";
    popover.style.display = "flex";
  }

  function hideCodePopover() {
    document.getElementById("code-popover-backdrop").style.display = "none";
    document.getElementById("code-popover").style.display = "none";
  }

  function getDesignDocumentForModule(modulePath) {
    const basePath = modulePath || SOURCE_ROOT;
    return parseDesignDocument(`${basePath}/.design.md`)
      || parseDesignDocument(`${basePath}/.design.json`);
  }

  function collectDesignDocuments(modulePath) {
    const prefix = modulePath ? `${modulePath}/` : "";
    const preferredDocuments = new Map();
    for (const [path] of Object.entries(manifest.files || {})
      .filter(([path]) => isDesignDocumentPath(path))
      .filter(([path]) => !prefix || path.startsWith(prefix))) {
      const moduleKey = path.replace(/\/\.design\.(md|json)$/i, "");
      const current = preferredDocuments.get(moduleKey);
      if (!current || path.endsWith(".design.md")) {
        preferredDocuments.set(moduleKey, path);
      }
    }

    return Array.from(preferredDocuments.values())
      .map(path => parseDesignDocument(path))
      .filter(Boolean)
      .sort((a, b) => a.path.localeCompare(b.path));
  }

  function isDesignDocumentPath(path) {
    return path.endsWith("/.design.md") || path.endsWith("/.design.json");
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

  function getChange(path) {
    return ((manifest.diff || {}).changes || {})[path];
  }

  function escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text == null ? "" : String(text);
    return div.innerHTML;
  }
