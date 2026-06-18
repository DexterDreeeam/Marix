/* Marix Overview - App Logic */

(function () {
  "use strict";

  const SVG_NS = "http://www.w3.org/2000/svg";

  const I18N = {
    en: {
      title: "Overview",
      fileView: "File View",
      starMapView: "Star Map",
      language: "中文",
      searchPlaceholder: "Search files...",
      changedFiles: "Changed files",
      changedSections: "Changed sections",
      tagNoTags: "No tags",
      tagSince: "Since",
      selectFile: "Select a file to view",
      welcomeTitle: "Welcome to Marix Overview",
      welcomeIntro: "Browse repository files from the sidebar.",
      welcomeModified: "Files marked with M have been modified since the last tag.",
      welcomeAdded: "Files marked with A are newly added.",
      welcomeDeleted: "Files marked with D have been deleted.",
      fileUnavailable: "File content not available.",
      statusModified: "Modified",
      statusAdded: "Added",
      statusDeleted: "Deleted",
      statusRenamed: "Renamed",
      diffPanelTitle: "Changed sections",
      diffPanelSubtitle: "Showing only modified parts of this file.",
      noChangedSections: "No changed sections are available for this file.",
      reason: "Reason",
      reasonPending: "Pending overview-agent annotation.",
      starTitle: "Star Map",
      starDescription: "Browse repository modules as a nested star map. Modules are derived from folder hierarchy, and changed modules are highlighted from marix tag diffs.",
      starHelp: "Wheel to zoom. Drag the canvas to pan. Select a module to inspect it. Use the details panel to expand or collapse submodules.",
      resetView: "Reset view",
      rootModule: "Repository root",
      changed: "Changed",
      unchanged: "Unchanged",
      modulePath: "Module path",
      childModules: "Child modules",
      files: "Files",
      changedFileList: "Changed files",
      interfaces: "Interfaces",
      dataStorage: "Data storage",
      implementations: "Implementation files",
      noItems: "None",
      expand: "Expand",
      collapse: "Collapse",
      moduleDetailsHint: "Select a module in the map to see interfaces, storage, and implementation files.",
      directoryModule: "Directory module",
      rustModuleHint: "Rust module candidates are inferred from folder layers and Rust files such as lib.rs, mod.rs, main.rs, and *.rs."
    },
    zh: {
      title: "总览",
      fileView: "文件视图",
      starMapView: "星图视图",
      language: "EN",
      searchPlaceholder: "搜索文件...",
      changedFiles: "只看改动文件",
      changedSections: "只看改动片段",
      tagNoTags: "没有 tag",
      tagSince: "起始 tag",
      selectFile: "选择一个文件查看",
      welcomeTitle: "欢迎来到 Marix 总览",
      welcomeIntro: "从左侧浏览仓库文件。",
      welcomeModified: "标记为 M 的文件表示从上一个 tag 后被修改。",
      welcomeAdded: "标记为 A 的文件表示新增加。",
      welcomeDeleted: "标记为 D 的文件表示已删除。",
      fileUnavailable: "文件内容不可用。",
      statusModified: "已修改",
      statusAdded: "已新增",
      statusDeleted: "已删除",
      statusRenamed: "已重命名",
      diffPanelTitle: "改动片段",
      diffPanelSubtitle: "当前只展示这个文件中发生变化的部分。",
      noChangedSections: "这个文件没有可展示的改动片段。",
      reason: "原因",
      reasonPending: "等待 overview-agent 补充说明。",
      starTitle: "星图视图",
      starDescription: "以嵌套星图浏览仓库模块。模块根据文件夹层级生成，并根据 marix tag diff 高亮改动模块。",
      starHelp: "鼠标滚轮缩放。拖动画布平移。选择模块查看详情。可在右侧面板展开或折叠子模块。",
      resetView: "重置视图",
      rootModule: "仓库根模块",
      changed: "有改动",
      unchanged: "无改动",
      modulePath: "模块路径",
      childModules: "子模块",
      files: "文件",
      changedFileList: "改动文件",
      interfaces: "接口",
      dataStorage: "数据存储",
      implementations: "实现文件",
      noItems: "无",
      expand: "展开",
      collapse: "折叠",
      moduleDetailsHint: "在星图中选择模块，即可查看接口、数据存储和实现文件。",
      directoryModule: "目录模块",
      rustModuleHint: "Rust 模块候选会根据文件夹层级和 lib.rs、mod.rs、main.rs、*.rs 等 Rust 文件推断。"
    }
  };

  let manifest = null;
  let language = localStorage.getItem("marix-overview-language") || "en";
  let overviewMode = "file";
  let changedFilesOnly = false;
  let changedSectionsOnly = false;
  let currentFile = null;
  let moduleRoot = null;
  let selectedModule = null;
  let collapsedModules = new Set();
  let starTransform = { x: 0, y: 0, scale: 1 };
  let panState = null;

  async function init() {
    try {
      const resp = await fetch("manifest.json");
      manifest = await resp.json();
    } catch (e) {
      manifest = { files: {}, diff: { prev_tag: null, latest_tag: null, changes: {} }, generated_at: "" };
    }

    moduleRoot = buildModuleTree(manifest.files || {});
    selectedModule = moduleRoot;

    bindEvents();
    applyLanguage();
    renderTree();
    renderStarMap();
    renderModuleDetails(selectedModule);
  }

  function t(key) {
    return I18N[language][key] || I18N.en[key] || key;
  }

  function applyLanguage() {
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
    document.title = `Marix - ${t("title")}`;

    for (const el of document.querySelectorAll("[data-i18n]")) {
      el.textContent = t(el.dataset.i18n);
    }

    const searchInput = document.getElementById("search-input");
    searchInput.placeholder = t("searchPlaceholder");

    document.getElementById("btn-language").textContent = t("language");
    document.getElementById("file-path").textContent = currentFile || t("selectFile");

    renderTagInfo();
    renderWelcome();
    renderMode();
    renderStarMap();
    renderModuleDetails(selectedModule);

    if (currentFile) {
      openFile(currentFile);
    }
  }

  function renderTagInfo() {
    const el = document.getElementById("tag-info");
    const d = manifest.diff || {};
    if (d.prev_tag && d.latest_tag) {
      el.textContent = `${d.prev_tag} -> ${d.latest_tag}`;
    } else if (d.latest_tag) {
      el.textContent = `${t("tagSince")}: ${d.latest_tag}`;
    } else {
      el.textContent = t("tagNoTags");
    }
  }

  function renderWelcome() {
    const welcome = document.getElementById("welcome");
    welcome.innerHTML = `
      <h2>${escapeHtml(t("welcomeTitle"))}</h2>
      <p>${escapeHtml(t("welcomeIntro"))}</p>
      <p>${escapeHtml(t("welcomeModified"))}</p>
      <p>${escapeHtml(t("welcomeAdded"))}</p>
      <p>${escapeHtml(t("welcomeDeleted"))}</p>
    `;
  }

  function renderMode() {
    document.getElementById("main").style.display = overviewMode === "file" ? "flex" : "none";
    document.getElementById("star-map-workspace").style.display = overviewMode === "star" ? "flex" : "none";
    document.getElementById("btn-file-view").classList.toggle("active", overviewMode === "file");
    document.getElementById("btn-star-map-view").classList.toggle("active", overviewMode === "star");
  }

  function buildTreeStructure(files) {
    const root = { __children: {}, __files: [] };
    const paths = Object.keys(files).sort();
    for (const p of paths) {
      const parts = p.split("/");
      let node = root;
      for (let i = 0; i < parts.length - 1; i++) {
        if (!node.__children[parts[i]]) {
          node.__children[parts[i]] = { __children: {}, __files: [] };
        }
        node = node.__children[parts[i]];
      }
      node.__files.push({ name: parts[parts.length - 1], path: p });
    }
    return root;
  }

  function renderTree(filter) {
    const container = document.getElementById("file-tree");
    container.innerHTML = "";
    const tree = buildTreeStructure(manifest.files || {});
    const filterLower = filter ? filter.toLowerCase() : null;
    renderNode(container, tree, 0, "", filterLower);
  }

  function renderNode(parent, node, depth, prefix, filter) {
    for (const dirName of Object.keys(node.__children).sort()) {
      const dirPath = prefix ? `${prefix}/${dirName}` : dirName;
      const child = node.__children[dirName];

      if (filter && !hasMatchingDescendant(child, dirPath, filter)) continue;
      if (changedFilesOnly && !hasDiffDescendant(child, dirPath)) continue;

      const dirEl = createTreeItem(dirName, depth, true, null, dirPath);
      parent.appendChild(dirEl);

      const childContainer = document.createElement("div");
      childContainer.className = "dir-children";
      parent.appendChild(childContainer);

      dirEl.addEventListener("click", () => {
        childContainer.classList.toggle("collapsed");
        const toggle = dirEl.querySelector(".tree-toggle");
        if (toggle) toggle.classList.toggle("collapsed");
      });

      renderNode(childContainer, child, depth + 1, dirPath, filter);
    }

    for (const file of node.__files) {
      if (filter && !file.path.toLowerCase().includes(filter) && !file.name.toLowerCase().includes(filter)) continue;
      const change = getChange(file.path);
      if (changedFilesOnly && !change) continue;

      const el = createTreeItem(file.name, depth, false, change ? change.status : null, file.path);
      el.addEventListener("click", () => openFile(file.path, el));
      parent.appendChild(el);
    }
  }

  function hasMatchingDescendant(node, prefix, filter) {
    for (const f of node.__files) {
      const fp = `${prefix}/${f.name}`;
      if (fp.toLowerCase().includes(filter) || f.name.toLowerCase().includes(filter)) return true;
    }
    for (const [dn, child] of Object.entries(node.__children)) {
      if (hasMatchingDescendant(child, `${prefix}/${dn}`, filter)) return true;
    }
    return false;
  }

  function hasDiffDescendant(node, prefix) {
    for (const f of node.__files) {
      if (getChange(`${prefix}/${f.name}`)) return true;
    }
    for (const [dn, child] of Object.entries(node.__children)) {
      if (hasDiffDescendant(child, `${prefix}/${dn}`)) return true;
    }
    return false;
  }

  function createTreeItem(name, depth, isDir, status, path) {
    const el = document.createElement("div");
    el.className = `tree-item${isDir ? " dir" : ""}`;
    el.dataset.path = path;

    const indent = document.createElement("span");
    indent.className = "tree-indent";
    indent.style.width = `${depth * 16}px`;
    el.appendChild(indent);

    const icon = document.createElement("span");
    icon.className = `tree-icon${isDir ? " tree-toggle" : ""}`;
    icon.textContent = isDir ? "▾" : getFileIcon(name);
    el.appendChild(icon);

    const nameEl = document.createElement("span");
    nameEl.className = "tree-name";
    nameEl.textContent = name;
    el.appendChild(nameEl);

    if (status) {
      const badge = document.createElement("span");
      const labels = { M: ["M", "badge-modified"], A: ["A", "badge-added"], D: ["D", "badge-deleted"], R: ["R", "badge-renamed"] };
      const [label, cls] = labels[status] || ["?", "badge-modified"];
      badge.className = `badge ${cls}`;
      badge.textContent = label;
      el.appendChild(badge);
    }

    return el;
  }

  function getFileIcon(name) {
    const ext = name.split(".").pop().toLowerCase();
    const icons = {
      py: "PY", rs: "RS", md: "MD", yaml: "YML", yml: "YML",
      toml: "TOML", json: "JSON", html: "HTML", css: "CSS", js: "JS",
      png: "IMG", jpg: "IMG", jpeg: "IMG", gif: "IMG", svg: "IMG",
      txt: "TXT", sh: "SH", bat: "BAT", ps1: "PS1"
    };
    return icons[ext] || "FILE";
  }

  function openFile(path, treeEl) {
    if (treeEl) {
      document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
      treeEl.classList.add("active");
    }

    currentFile = path;
    const change = getChange(path);
    const fileData = (manifest.files || {})[path];
    const statusEl = document.getElementById("file-status");

    document.getElementById("file-path").textContent = path;
    statusEl.innerHTML = change ? renderStatusBadge(change.status) : "";
    hideAllViews();

    if (!fileData) {
      showWelcome(t("fileUnavailable"));
      return;
    }

    if (changedSectionsOnly) {
      showChangedSections(path, change || { diff_lines: [], hunks: [] });
      return;
    }

    const ext = path.split(".").pop().toLowerCase();
    if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico"].includes(ext)) {
      showImage(fileData);
    } else if (ext === "md") {
      showMarkdown(fileData.content || "");
    } else {
      showCode(fileData.content || "", ext);
    }
  }

  function renderStatusBadge(status) {
    const labels = {
      M: [t("statusModified"), "badge-modified"],
      A: [t("statusAdded"), "badge-added"],
      D: [t("statusDeleted"), "badge-deleted"],
      R: [t("statusRenamed"), "badge-renamed"]
    };
    const [label, cls] = labels[status] || [status, "badge-modified"];
    return `<span class="badge ${cls}">${escapeHtml(label)}</span>`;
  }

  function hideAllViews() {
    document.getElementById("welcome").style.display = "none";
    document.getElementById("markdown-view").style.display = "none";
    document.getElementById("image-view").style.display = "none";
    document.getElementById("code-view").style.display = "none";
    document.getElementById("diff-view").style.display = "none";
  }

  function showWelcome(msg) {
    const el = document.getElementById("welcome");
    el.style.display = "block";
    if (msg) el.innerHTML = `<p>${escapeHtml(msg)}</p>`;
  }

  function showMarkdown(content) {
    const el = document.getElementById("markdown-view");
    el.style.display = "block";
    el.innerHTML = marked.parse(content);
    el.querySelectorAll("pre code").forEach(block => hljs.highlightElement(block));
  }

  function showImage(fileData) {
    const el = document.getElementById("image-view");
    el.style.display = "flex";
    const img = document.getElementById("image-el");
    img.src = fileData.base64 ? `data:${fileData.mime || "image/png"};base64,${fileData.base64}` : (fileData.url || "");
  }

  function showCode(content, ext) {
    const view = document.getElementById("code-view");
    view.style.display = "block";
    const codeEl = document.getElementById("code-el");
    codeEl.textContent = content;
    codeEl.className = "";
    const langMap = { py: "python", rs: "rust", js: "javascript", ts: "typescript", yml: "yaml", yaml: "yaml", sh: "bash", md: "markdown", html: "html", css: "css", json: "json", toml: "toml" };
    const lang = langMap[ext];
    if (lang) codeEl.classList.add(`language-${lang}`);
    hljs.highlightElement(codeEl);
  }

  function showChangedSections(path, change) {
    const diffView = document.getElementById("diff-view");
    diffView.style.display = "block";

    const sections = splitDiffSections(change.diff_lines || [], change.hunks || []);
    if (sections.length === 0) {
      diffView.innerHTML = `<div class="diff-empty">${escapeHtml(t("noChangedSections"))}</div>`;
      return;
    }

    const panels = sections.map((section, index) => renderDiffPanel(path, section, index + 1)).join("");
    diffView.innerHTML = `
      <div class="diff-view-header">
        <h3>${escapeHtml(t("diffPanelTitle"))}</h3>
        <p>${escapeHtml(t("diffPanelSubtitle"))}</p>
      </div>
      <div class="diff-panel-list">${panels}</div>
    `;
  }

  function splitDiffSections(diffLines, hunks) {
    const sections = [];
    let current = null;
    let hunkIndex = 0;

    for (const line of diffLines) {
      if (line.startsWith("@@")) {
        if (current) sections.push(current);
        const hunk = hunks[hunkIndex] || {};
        current = { header: line, reason: hunk.reason || "", lines: [] };
        hunkIndex += 1;
      } else if (current && (line.startsWith("+") || line.startsWith("-") || line.startsWith(" "))) {
        current.lines.push(line);
      }
    }

    if (current) sections.push(current);
    return sections;
  }

  function renderDiffPanel(path, section, number) {
    const lines = section.lines.map(renderDiffLine).join("");
    const reason = section.reason || t("reasonPending");
    return `
      <section class="diff-panel">
        <header class="diff-panel-header">
          <span class="diff-panel-number">#${number}</span>
          <code>${escapeHtml(section.header || path)}</code>
        </header>
        <div class="diff-panel-reason">
          <strong>${escapeHtml(t("reason"))}:</strong> ${escapeHtml(reason)}
        </div>
        <div class="diff-panel-lines">${lines}</div>
      </section>
    `;
  }

  function renderDiffLine(line) {
    let cls = "diff-line-context";
    if (line.startsWith("+")) cls = "diff-line-add";
    else if (line.startsWith("-")) cls = "diff-line-del";
    return `<div class="diff-line ${cls}"><span class="diff-line-content">${escapeHtml(line)}</span></div>`;
  }

  function buildModuleTree(files) {
    const root = createModuleNode("Marix", "");

    for (const path of Object.keys(files).sort()) {
      const parts = path.split("/");
      let node = root;

      for (let i = 0; i < parts.length - 1; i++) {
        const childPath = parts.slice(0, i + 1).join("/");
        if (!node.childrenMap[parts[i]]) {
          node.childrenMap[parts[i]] = createModuleNode(parts[i], childPath);
        }
        node = node.childrenMap[parts[i]];
      }

      node.files.push(path);
      categorizeModuleFile(node, path);
    }

    markChangedModules(root);
    finalizeModuleTree(root);
    return root;
  }

  function createModuleNode(name, path) {
    return {
      name,
      path,
      files: [],
      changedFiles: [],
      interfaceFiles: [],
      dataStorageFiles: [],
      implementationFiles: [],
      children: [],
      childrenMap: {},
      changed: false,
      hasRust: false
    };
  }

  function categorizeModuleFile(node, path) {
    const fileName = path.split("/").pop().toLowerCase();
    const ext = fileName.includes(".") ? fileName.split(".").pop() : "";

    if (ext === "rs") {
      node.hasRust = true;
      node.implementationFiles.push(path);
      if (["lib.rs", "main.rs", "mod.rs"].includes(fileName) || fileName.endsWith("_api.rs") || fileName.endsWith("_interface.rs")) {
        node.interfaceFiles.push(path);
      }
      return;
    }

    if (ext === "py") {
      node.implementationFiles.push(path);
      if (fileName === "__init__.py" || fileName.endsWith("_api.py") || fileName.endsWith("_interface.py")) {
        node.interfaceFiles.push(path);
      }
      return;
    }

    if (["json", "yaml", "yml", "toml", "sql", "sqlite", "db"].includes(ext)) {
      node.dataStorageFiles.push(path);
    }

    if (["js", "ts", "ps1", "sh", "html", "css"].includes(ext)) {
      node.implementationFiles.push(path);
    }
  }

  function markChangedModules(root) {
    const changes = ((manifest.diff || {}).changes || {});
    for (const path of Object.keys(changes)) {
      const parts = path.split("/");
      let node = root;
      node.changed = true;
      node.changedFiles.push(path);

      for (let i = 0; i < parts.length - 1; i++) {
        if (!node.childrenMap[parts[i]]) break;
        node = node.childrenMap[parts[i]];
        node.changed = true;
        node.changedFiles.push(path);
      }
    }
  }

  function finalizeModuleTree(node) {
    node.children = Object.values(node.childrenMap).sort((a, b) => a.name.localeCompare(b.name));
    delete node.childrenMap;
    for (const child of node.children) {
      finalizeModuleTree(child);
      node.hasRust = node.hasRust || child.hasRust;
    }
  }

  function renderStarMap() {
    const svg = document.getElementById("star-map-svg");
    const layer = document.getElementById("star-map-layer");
    if (!svg || !layer || !moduleRoot) return;

    layer.innerHTML = "";
    layer.setAttribute("transform", `translate(${starTransform.x} ${starTransform.y}) scale(${starTransform.scale})`);

    const layout = [];
    layoutModule(moduleRoot, 0, Math.PI * 2, 0, layout, null);

    for (const item of layout.filter(x => x.parent)) {
      const line = document.createElementNS(SVG_NS, "line");
      line.setAttribute("class", `star-edge${item.node.changed ? " changed" : ""}`);
      line.setAttribute("x1", item.parent.x);
      line.setAttribute("y1", item.parent.y);
      line.setAttribute("x2", item.x);
      line.setAttribute("y2", item.y);
      layer.appendChild(line);
    }

    for (const item of layout) {
      layer.appendChild(createStarNode(item));
    }
  }

  function layoutModule(node, startAngle, endAngle, depth, layout, parent) {
    const radiusStep = 165;
    const angle = (startAngle + endAngle) / 2;
    const radius = depth * radiusStep;
    const item = {
      node,
      depth,
      parent,
      x: depth === 0 ? 0 : Math.cos(angle) * radius,
      y: depth === 0 ? 0 : Math.sin(angle) * radius
    };
    layout.push(item);

    if (collapsedModules.has(node.path)) return;

    const children = node.children || [];
    const count = children.length;
    if (count === 0) return;

    const span = endAngle - startAngle;
    const childSpan = span / count;
    for (let i = 0; i < count; i++) {
      layoutModule(children[i], startAngle + childSpan * i, startAngle + childSpan * (i + 1), depth + 1, layout, item);
    }
  }

  function createStarNode(item) {
    const { node, depth } = item;
    const group = document.createElementNS(SVG_NS, "g");
    const selected = selectedModule && selectedModule.path === node.path;
    group.setAttribute("class", `star-node depth-${depth}${node.changed ? " changed" : ""}${selected ? " selected" : ""}`);
    group.setAttribute("transform", `translate(${item.x} ${item.y})`);
    group.setAttribute("tabindex", "0");

    const radius = depth === 0 ? 30 : Math.max(16, 25 - depth * 2);
    const circle = document.createElementNS(SVG_NS, "circle");
    circle.setAttribute("r", radius);
    group.appendChild(circle);

    if (node.children.length > 0) {
      const marker = document.createElementNS(SVG_NS, "text");
      marker.setAttribute("class", "star-collapse-marker");
      marker.setAttribute("x", radius - 2);
      marker.setAttribute("y", -radius + 8);
      marker.textContent = collapsedModules.has(node.path) ? "+" : "-";
      group.appendChild(marker);
    }

    const label = document.createElementNS(SVG_NS, "text");
    label.setAttribute("class", "star-label");
    label.setAttribute("y", radius + 16);
    label.textContent = node.path ? node.name : "Marix";
    group.appendChild(label);

    group.addEventListener("click", evt => {
      evt.stopPropagation();
      selectedModule = node;
      renderModuleDetails(node);
      renderStarMap();
    });

    group.addEventListener("dblclick", evt => {
      evt.stopPropagation();
      toggleModuleCollapse(node);
    });

    return group;
  }

  function renderModuleDetails(node) {
    if (!node) return;

    const title = document.getElementById("module-detail-title");
    const status = document.getElementById("module-detail-status");
    const body = document.getElementById("module-detail-body");

    title.textContent = node.path || t("rootModule");
    status.textContent = node.changed ? t("changed") : t("unchanged");
    status.className = `module-status ${node.changed ? "changed" : "unchanged"}`;

    const collapseLabel = collapsedModules.has(node.path) ? t("expand") : t("collapse");
    const childButtons = node.children.map(child => `
      <button class="module-link" data-module-path="${escapeHtml(child.path)}">
        <span>${escapeHtml(child.name)}</span>
        <span class="module-link-status ${child.changed ? "changed" : "unchanged"}">${escapeHtml(child.changed ? t("changed") : t("unchanged"))}</span>
      </button>
    `).join("");

    body.innerHTML = `
      <p class="module-hint">${escapeHtml(t("moduleDetailsHint"))}</p>
      <p class="module-hint">${escapeHtml(t("rustModuleHint"))}</p>
      <div class="module-actions">
        <button id="btn-module-collapse" class="toggle-btn" ${node.children.length === 0 ? "disabled" : ""}>${escapeHtml(collapseLabel)}</button>
      </div>
      ${renderDetailRow(t("modulePath"), node.path || t("rootModule"))}
      ${renderDetailRow(t("directoryModule"), node.hasRust ? "Rust-aware" : "Generic")}
      ${renderListSection(t("childModules"), childButtons || escapeHtml(t("noItems")), true)}
      ${renderListSection(t("changedFileList"), renderFileList(node.changedFiles))}
      ${renderListSection(t("interfaces"), renderFileList(node.interfaceFiles))}
      ${renderListSection(t("dataStorage"), renderFileList(node.dataStorageFiles))}
      ${renderListSection(t("implementations"), renderFileList(node.implementationFiles))}
      ${renderListSection(t("files"), renderFileList(node.files))}
    `;

    const collapseButton = document.getElementById("btn-module-collapse");
    if (collapseButton) {
      collapseButton.addEventListener("click", () => toggleModuleCollapse(node));
    }

    for (const link of body.querySelectorAll(".module-link")) {
      link.addEventListener("click", () => {
        const next = findModule(moduleRoot, link.dataset.modulePath);
        if (next) {
          selectedModule = next;
          renderModuleDetails(next);
          renderStarMap();
        }
      });
    }
  }

  function renderDetailRow(label, value) {
    return `
      <div class="module-detail-row">
        <span>${escapeHtml(label)}</span>
        <code>${escapeHtml(value)}</code>
      </div>
    `;
  }

  function renderListSection(title, content, rawContent) {
    const body = rawContent ? content : (content || `<span class="empty-list">${escapeHtml(t("noItems"))}</span>`);
    return `
      <section class="module-detail-section">
        <h4>${escapeHtml(title)}</h4>
        ${body}
      </section>
    `;
  }

  function renderFileList(files) {
    const unique = Array.from(new Set(files || [])).sort();
    if (unique.length === 0) return "";
    return `<ul class="module-file-list">${unique.map(file => `<li><code>${escapeHtml(file)}</code></li>`).join("")}</ul>`;
  }

  function toggleModuleCollapse(node) {
    if (!node || node.children.length === 0) return;
    if (collapsedModules.has(node.path)) {
      collapsedModules.delete(node.path);
    } else {
      collapsedModules.add(node.path);
    }
    renderModuleDetails(node);
    renderStarMap();
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

  function bindEvents() {
    const searchInput = document.getElementById("search-input");
    let searchTimeout;
    searchInput.addEventListener("input", () => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(() => renderTree(searchInput.value.trim()), 200);
    });

    document.getElementById("btn-language").addEventListener("click", () => {
      language = language === "en" ? "zh" : "en";
      localStorage.setItem("marix-overview-language", language);
      applyLanguage();
    });

    document.getElementById("btn-file-view").addEventListener("click", () => {
      overviewMode = "file";
      renderMode();
    });

    document.getElementById("btn-star-map-view").addEventListener("click", () => {
      overviewMode = "star";
      renderMode();
      renderStarMap();
    });

    document.getElementById("btn-toggle-changed-files").addEventListener("click", () => {
      changedFilesOnly = !changedFilesOnly;
      document.getElementById("btn-toggle-changed-files").classList.toggle("active", changedFilesOnly);
      renderTree(searchInput.value.trim());
    });

    document.getElementById("btn-toggle-changed-sections").addEventListener("click", () => {
      changedSectionsOnly = !changedSectionsOnly;
      document.getElementById("btn-toggle-changed-sections").classList.toggle("active", changedSectionsOnly);
      if (currentFile) openFile(currentFile);
    });

    document.getElementById("btn-reset-star-map").addEventListener("click", () => {
      starTransform = { x: 0, y: 0, scale: 1 };
      renderStarMap();
    });

    const svg = document.getElementById("star-map-svg");
    svg.addEventListener("wheel", evt => {
      evt.preventDefault();
      const delta = evt.deltaY > 0 ? 0.9 : 1.1;
      starTransform.scale = Math.max(0.35, Math.min(2.6, starTransform.scale * delta));
      renderStarMap();
    }, { passive: false });

    svg.addEventListener("pointerdown", evt => {
      panState = { x: evt.clientX, y: evt.clientY, startX: starTransform.x, startY: starTransform.y };
      svg.setPointerCapture(evt.pointerId);
    });

    svg.addEventListener("pointermove", evt => {
      if (!panState) return;
      starTransform.x = panState.startX + evt.clientX - panState.x;
      starTransform.y = panState.startY + evt.clientY - panState.y;
      renderStarMap();
    });

    svg.addEventListener("pointerup", () => {
      panState = null;
    });

    svg.addEventListener("pointerleave", () => {
      panState = null;
    });
  }

  document.addEventListener("DOMContentLoaded", init);
})();
