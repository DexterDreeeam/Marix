"use strict";
  function renderStarMap() {
    const svg = document.getElementById("star-map-svg");
    const layer = document.getElementById("star-map-layer");
    if (!svg || !layer || !moduleRoot) return;

    const scopeNode = getScopeModule();
    const layout = layoutScopeStarMap(scopeNode, {
      sourceRoot: SOURCE_ROOT,
      getParentPath,
      findModule: path => findModule(moduleRoot, path),
      getChange,
      getImmediateFiles,
      isHiddenPath,
      collectExposedGroups,
      normalizeStatus,
      getShortElementName
    });
    markFileFocus(layout);
    logStarMapState("render-star-map", getStarMapDebugSummary(layout, scopeNode));
    renderStarMapBreadcrumb(scopeNode);
    renderStarMapFileList(scopeNode);
    if (starAutoFit) {
      fitStarMapToLayout(layout);
      starAutoFit = false;
    }

    layer.innerHTML = "";
    renderStarMapDefs(layer);
    applyStarMapTransform();

    for (const item of layout.filter(x => x.parent)) {
      const edge = computeEdgePath(item.parent, item, SOURCE_ROOT);
      const path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("class", `star-edge edge-${item.edgeKind || "child"}${item.changed ? " changed" : ""}${item.focusDimmed || (item.parent && item.parent.focusDimmed) ? " dimmed" : ""}`);
      path.setAttribute("marker-end", `url(#arrow-${item.edgeKind || "child"})`);
      path.setAttribute("d", edge);
      layer.appendChild(path);
    }

    function renderStarMapDefs(layer) {
      const defs = document.createElementNS(SVG_NS, "defs");
      defs.innerHTML = `
        <marker id="arrow-child" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#ffffff" fill-opacity="0.58"></path>
        </marker>
        <marker id="arrow-parent" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#ffffff" fill-opacity="0.32"></path>
        </marker>
        <radialGradient id="node-gradient-default" cx="34%" cy="28%" r="70%">
          <stop offset="0%" stop-color="#8fbfff"></stop>
          <stop offset="42%" stop-color="#2f81f7"></stop>
          <stop offset="100%" stop-color="#0d3b66"></stop>
        </radialGradient>
        <radialGradient id="node-gradient-unchanged" cx="34%" cy="28%" r="70%">
          <stop offset="0%" stop-color="var(--node-neutral-a)"></stop>
          <stop offset="48%" stop-color="var(--node-neutral-b)"></stop>
          <stop offset="100%" stop-color="var(--node-neutral-c)"></stop>
        </radialGradient>
        <radialGradient id="node-gradient-added" cx="34%" cy="28%" r="70%">
          <stop offset="0%" stop-color="#8ff0a4"></stop>
          <stop offset="44%" stop-color="#3fb950"></stop>
          <stop offset="100%" stop-color="#14532d"></stop>
        </radialGradient>
        <radialGradient id="node-gradient-modified" cx="34%" cy="28%" r="70%">
          <stop offset="0%" stop-color="#ffe8a3"></stop>
          <stop offset="42%" stop-color="#d29922"></stop>
          <stop offset="100%" stop-color="#5f3f05"></stop>
        </radialGradient>
        <radialGradient id="node-gradient-deleted" cx="34%" cy="28%" r="70%">
          <stop offset="0%" stop-color="#ffaaa5"></stop>
          <stop offset="42%" stop-color="#f85149"></stop>
          <stop offset="100%" stop-color="#681414"></stop>
        </radialGradient>
      `;
      layer.appendChild(defs);
    }

    for (const item of layout) {
      if (item.kind === "exposed") {
        layer.appendChild(createExposedElementNode(item));
      } else {
        layer.appendChild(createStarNode(item));
      }
    }

    function markFileFocus(layout) {
      const focusedFile = starMapSelection.kind === "file" ? starMapSelection.path : "";
      if (!focusedFile) {
        for (const item of layout) item.focusDimmed = false;
        logStarMapState("mark-file-focus:none", {
          layoutItems: layout.length
        });
        return;
      }

      const focusedModulePath = getParentPath(focusedFile);
      let brightModules = 0;
      let dimmedModules = 0;
      let brightElements = 0;
      let dimmedElements = 0;
      for (const item of layout) {
        if (item.kind === "module" && item.node) {
          item.focusDimmed = item.node.path !== focusedModulePath;
          if (item.focusDimmed) dimmedModules += 1;
          else brightModules += 1;
        } else if (item.kind === "exposed") {
          item.focusDimmed = !isElementFromFocusedFile(item.element, focusedFile);
          if (item.focusDimmed) dimmedElements += 1;
          else brightElements += 1;
        } else {
          item.focusDimmed = true;
        }
      }
      logStarMapState("mark-file-focus:applied", {
        focusedModulePath,
        brightModules,
        dimmedModules,
        brightElements,
        dimmedElements
      });
    }
  }

  function getStarMapDebugSummary(layout, scopeNode) {
    return {
      scopeNode: scopeNode && scopeNode.path,
      layoutItems: layout.length,
      modules: layout.filter(item => item.kind === "module").length,
      exposed: layout.filter(item => item.kind === "exposed").length,
      dimmed: layout.filter(item => item.focusDimmed).length
    };
  }









  function renderStarMapFileList(scopeNode) {
    const container = document.getElementById("star-map-file-list");
    if (!container) return;

    const design = getDesignDocumentForModule(scopeNode.path);
    const designFiles = design ? (design.document.files || []).map(file => file.path) : [];
    const focusedFile = starMapSelection.kind === "file" ? starMapSelection.path : "";
    const files = Array.from(new Set(designFiles.concat(getImmediateFiles(scopeNode))))
      .filter(path => !isHiddenPath(path))
      .sort((a, b) => {
        if (a === focusedFile) return -1;
        if (b === focusedFile) return 1;
        return a.localeCompare(b);
      });
    const visibleFiles = files.slice(0, 4);
    const hiddenFiles = files.slice(4);
    const renderFileRow = path => {
      const status = getFileStatus(path);
      const isFocused = path === focusedFile;
      const isDimmed = Boolean(focusedFile && !isFocused);
      return `
        <button class="star-map-file-item${isFocused ? " focused" : ""}${isDimmed ? " file-focus-dimmed" : ""}" data-file-path="${escapeHtml(path)}" title="${escapeHtml(path)}">
          <span class="status-chip status-${escapeHtml(status)}"></span>
          <code>${escapeHtml(path.split("/").pop())}</code>
        </button>
      `;
    };
    const visibleRows = visibleFiles.map(renderFileRow).join("");
    const hiddenRows = hiddenFiles.map(renderFileRow).join("");
    const hiddenContent = hiddenRows ? `<div class="star-map-file-extra-items">${hiddenRows}</div>` : "";
    const more = hiddenRows ? `<div class="star-map-file-more" aria-hidden="true">...</div>` : "";

    container.innerHTML = `
      <div class="star-map-file-list-inner${focusedFile ? " file-focus-list" : ""}">
        ${visibleRows || `<span class="empty-list">${escapeHtml(t("noItems"))}</span>`}
        ${hiddenContent}
        ${more}
      </div>
    `;

    for (const button of container.querySelectorAll(".star-map-file-item")) {
      button.addEventListener("click", () => {
        focusStarMapFile(button.dataset.filePath, { openPopover: true });
      });
    }
  }

  function renderStarMapBreadcrumb(scopeNode) {
    const container = document.getElementById("star-map-breadcrumb");
    if (!container) return;
    const segments = getModulePathSegments(scopeNode && scopeNode.path);
    container.innerHTML = segments.map(segment => `
      <button class="star-map-breadcrumb-item${segment.current ? " current" : ""}" type="button" data-module-path="${escapeHtml(segment.path)}" title="${escapeHtml(segment.path)}">
        ${escapeHtml(segment.name)}
      </button>
    `).join("");

    for (const button of container.querySelectorAll(".star-map-breadcrumb-item")) {
      button.addEventListener("click", () => {
        selectStarMapModule(button.dataset.modulePath);
      });
    }
  }

  function getModulePathSegments(modulePath) {
    const normalized = normalizeScopePath(modulePath || SOURCE_ROOT);
    const parts = normalized.split("/").filter(Boolean);
    const segments = parts.map((name, index) => {
      const path = parts.slice(0, index + 1).join("/");
      return {
        name,
        path,
        current: path === normalized
      };
    });
    return segments.slice(-4);
  }

  function getFileStatus(path) {
    const design = getDesignDocumentForModule(getParentPath(path));
    const file = design ? (design.document.files || []).find(item => item.path === path) : null;
    const fromDesign = getExplicitStarMapChangeStatus(file);
    if (fromDesign) return fromDesign;

    const change = getChange(path);
    if (!change) return "unchanged";
    const statusMap = { A: "added", M: "modified", D: "deleted", R: "renamed" };
    return statusMap[change.status] || "modified";
  }

  function fitStarMapToLayout(layout) {
    if (!layout || layout.length === 0) {
      starTransform = { x: 0, y: 0, scale: 1 };
      return;
    }

    const maxDistance = layout.reduce((max, item) => Math.max(max, Math.hypot(item.x || 0, item.y || 0)), 0);
    const targetRadius = Math.max(190, maxDistance + 34);
    const scale = Math.max(1.15, Math.min(2.65, 780 / targetRadius));
    starTransform = { x: 0, y: 0, scale };
  }

  function requestStarMapFit() {
    starAutoFit = true;
  }

  function getScopeModule() {
    return findModule(moduleRoot, scopePath) || findModule(moduleRoot, SOURCE_ROOT) || moduleRoot;
  }

  function getImmediateFiles(node) {
    return (node.files || []).filter(path => !isHiddenPath(path));
  }

  function isHiddenPath(path) {
    return path.split("/").some(part => part.startsWith("."));
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
    const status = getModuleStatus(node);
    group.setAttribute("class", `star-node depth-${depth} depth-${item.depthClass || "mid"} status-${status}${node.changed ? " changed" : ""}${selected ? " selected" : ""}${item.focusDimmed ? " dimmed" : ""}`);
    group.setAttribute("transform", `translate(${item.x} ${item.y})`);
    group.setAttribute("tabindex", "0");

    const radius = getNodeRadius(item);
    const circle = document.createElementNS(SVG_NS, "circle");
    circle.setAttribute("r", radius);
    if (selected) {
      circle.style.animationDelay = getScopePulseAnimationDelay(node.path);
    }
    group.appendChild(circle);

    const highlight = document.createElementNS(SVG_NS, "circle");
    highlight.setAttribute("class", "star-node-highlight");
    highlight.setAttribute("cx", -radius * 0.28);
    highlight.setAttribute("cy", -radius * 0.30);
    highlight.setAttribute("r", Math.max(5, radius * 0.28));
    group.appendChild(highlight);

    function getModuleStatus(node) {
      const design = getDesignDocumentForModule(node.path);
      const status = getExplicitStarMapChangeStatus(design && design.document && design.document.module);
      if (status) return status;
      return node.changed ? "modified" : "unchanged";
    }

    const label = document.createElementNS(SVG_NS, "text");
    label.setAttribute("class", "star-label");
    label.setAttribute("x", 0);
    label.setAttribute("y", 1);
    label.textContent = node.path ? node.name : "Marix";
    group.appendChild(label);

    group.addEventListener("click", evt => {
      evt.stopPropagation();
      selectStarMapModule(node.path);
    });

    return group;
  }

  function applyStarMapTransform() {
    const layer = document.getElementById("star-map-layer");
    if (!layer) return;
    layer.setAttribute("transform", `translate(${starTransform.x} ${starTransform.y}) scale(${starTransform.scale})`);
  }
