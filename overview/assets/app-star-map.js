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
    renderStarMapFileList(scopeNode);
    if (starAutoFit) {
      fitStarMapToLayout(layout);
      starAutoFit = false;
    }

    layer.innerHTML = "";
    renderStarMapDefs(layer);
    layer.setAttribute("transform", `translate(${starTransform.x} ${starTransform.y}) scale(${starTransform.scale})`);

    for (const item of layout.filter(x => x.parent)) {
      const edge = computeEdgePath(item.parent, item, SOURCE_ROOT);
      const path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("class", `star-edge edge-${item.edgeKind || "child"}${item.changed ? " changed" : ""}`);
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
  }









  function getExposedLabelOffset(item) {
    return {
      x: 0,
      y: getNodeRadius(item) + 14,
      anchor: "middle"
    };
  }

  function collectExposedGroups(modulePath) {
    const documents = collectDesignDocuments(modulePath);
    return documents.flatMap(({ path, document }) => (document.exposedGroups || [])
      .map((group, index) => ({
        ...group,
        name: group.name || `${path}#${index}`,
        elements: (group.elements || []).filter(isPublicExposedElement)
      }))
      .filter(group => group.elements.length > 0));
  }

  function isPublicExposedElement(element) {
    const signature = String(element.signature || "").trim();
    const kind = String(element.kind || "").toLowerCase();
    if (/^mod\s+/.test(signature)) return false;
    if (kind === "module" || kind === "re-export") return false;
    if (/^pub\s+use\b/.test(signature) || /^pub\s+mod\b/.test(signature)) return false;
    if (isTupleWrapperElement(element)) return false;
    return /^pub\s+(trait|struct|enum|fn|type|const|static)\b/.test(signature) || element.public === true;
  }

  function isTupleWrapperElement(element) {
    const definition = String(element.code || element.signature || "").trim();
    return String(element.kind || "").toLowerCase() === "struct"
      && /^pub\s+struct\s+[A-Za-z_][A-Za-z0-9_]*\s*\(\s*pub\s+[^)]+\)\s*;$/.test(definition);
  }






  function renderStarMapFileList(scopeNode) {
    const container = document.getElementById("star-map-file-list");
    if (!container) return;

    const design = getDesignDocumentForModule(scopeNode.path);
    const designFiles = design ? (design.document.files || []).map(file => file.path) : [];
    const files = Array.from(new Set(designFiles.concat(getImmediateFiles(scopeNode)))).filter(path => !isHiddenPath(path)).sort();
    const rows = files.map(path => {
      const status = getFileStatus(path);
      return `
        <button class="star-map-file-item" data-file-path="${escapeHtml(path)}">
          <span class="status-chip status-${escapeHtml(status)}"></span>
          <code>${escapeHtml(path.split("/").pop())}</code>
        </button>
      `;
    }).join("");

    container.innerHTML = `
      ${rows || `<span class="empty-list">${escapeHtml(t("noItems"))}</span>`}
    `;

    for (const button of container.querySelectorAll(".star-map-file-item")) {
      button.addEventListener("click", () => {
        currentFile = button.dataset.filePath;
        currentDirectory = getParentPath(currentFile);
        localStorage.setItem(STORAGE_KEYS.currentFile, currentFile);
        setScopePath(currentDirectory);
        selectedModule = getScopeModule();
        renderModuleDetails(selectedModule);
        renderStarMap();
      });
    }
  }

  function getFileStatus(path) {
    const design = getDesignDocumentForModule(getParentPath(path));
    const file = design ? (design.document.files || []).find(item => item.path === path) : null;
    const fromDesign = normalizeStatus(file && file.changeStatus);
    if (fromDesign !== "unchanged") return fromDesign;

    const change = getChange(path);
    if (!change) return "unchanged";
    const statusMap = { A: "added", M: "modified", D: "deleted", R: "renamed" };
    return statusMap[change.status] || "modified";
  }

  function normalizeStatus(status) {
    const value = String(status || "unchanged").toLowerCase();
    return ["added", "modified", "deleted", "renamed", "unchanged"].includes(value) ? value : "unchanged";
  }

  function fitStarMapToLayout(layout) {
    if (!layout || layout.length === 0) {
      starTransform = { x: 0, y: 0, scale: 1 };
      return;
    }

    const maxDistance = layout.reduce((max, item) => Math.max(max, Math.hypot(item.x || 0, item.y || 0)), 0);
    const targetRadius = Math.max(260, maxDistance + 90);
    const scale = Math.max(0.85, Math.min(1.9, 620 / targetRadius));
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
    group.setAttribute("class", `star-node depth-${depth} depth-${item.depthClass || "mid"} status-${status}${node.changed ? " changed" : ""}${selected ? " selected" : ""}`);
    group.setAttribute("transform", `translate(${item.x} ${item.y})`);
    group.setAttribute("tabindex", "0");

    const radius = getNodeRadius(item);
    const circle = document.createElementNS(SVG_NS, "circle");
    circle.setAttribute("r", radius);
    group.appendChild(circle);

    const highlight = document.createElementNS(SVG_NS, "circle");
    highlight.setAttribute("class", "star-node-highlight");
    highlight.setAttribute("cx", -radius * 0.28);
    highlight.setAttribute("cy", -radius * 0.30);
    highlight.setAttribute("r", Math.max(5, radius * 0.28));
    group.appendChild(highlight);

    function getModuleStatus(node) {
      const design = getDesignDocumentForModule(node.path);
      const status = normalizeStatus(design && design.document && design.document.module && design.document.module.changeStatus);
      if (status !== "unchanged") return status;
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
      setScopePath(node.path);
      currentFile = null;
      currentDirectory = node.path;
      requestStarMapFit();
      selectedModule = node;
      renderModuleDetails(node);
      renderStarMap();
    });

    return group;
  }

  function createExposedElementNode(item) {
    const element = item.element || {};
    const shape = getExposedElementShape(element);
    const status = getExposedElementStatus(element);
    const typeClass = getExposedElementTypeClass(element);
    const radius = getNodeRadius(item);
    const group = document.createElementNS(SVG_NS, "g");
    group.setAttribute("class", `exposed-node exposed-${shape} exposed-type-${typeClass} status-${status}`);
    group.setAttribute("transform", `translate(${item.x} ${item.y})`);
    group.setAttribute("tabindex", "0");

    const hit = document.createElementNS(SVG_NS, "circle");
    hit.setAttribute("class", "exposed-hit-target");
    hit.setAttribute("r", Math.max(24, radius * 2.2));
    group.appendChild(hit);

    if (shape === "square") {
      const size = radius * 1.8;
      const rect = document.createElementNS(SVG_NS, "rect");
      rect.setAttribute("x", -size / 2);
      rect.setAttribute("y", -size / 2);
      rect.setAttribute("width", size);
      rect.setAttribute("height", size);
      rect.setAttribute("rx", 3);
      group.appendChild(rect);
    } else if (shape === "triangle") {
      const points = [
        `0,${-radius}`,
        `${radius * 0.95},${radius * 0.75}`,
        `${-radius * 0.95},${radius * 0.75}`
      ].join(" ");
      const polygon = document.createElementNS(SVG_NS, "polygon");
      polygon.setAttribute("points", points);
      group.appendChild(polygon);
    } else {
      const circle = document.createElementNS(SVG_NS, "circle");
      circle.setAttribute("r", radius);
      group.appendChild(circle);
    }

    const title = document.createElementNS(SVG_NS, "title");
    title.textContent = `${element.name || "exposed"} (${item.groupName || "group"})`;
    group.appendChild(title);

    const label = document.createElementNS(SVG_NS, "text");
    label.setAttribute("class", "exposed-label");
    label.setAttribute("x", item.labelX || 0);
    label.setAttribute("y", item.labelY || radius + 12);
    label.setAttribute("text-anchor", item.labelAnchor || "middle");
    label.textContent = item.label || getShortElementName(element);
    group.appendChild(label);

    group.addEventListener("click", evt => {
      evt.stopPropagation();
      showCodePopover(getCodeTitle(element), element.code || element.signature || "", element.language || "rust");
    });

    return group;
  }

  function getExposedElementShape(element) {
    const explicit = String(element.shape || "").toLowerCase();
    if (["circle", "square", "triangle"].includes(explicit)) return explicit;

    const kind = String(element.kind || "").toLowerCase();
    if (["enum", "struct", "data", "type-alias", "global", "global-variable", "const", "static"].includes(kind)) return "square";
    if (["global-interface", "public-api", "public-global-interface"].includes(kind)) return "triangle";
    return "circle";
  }

  function getShortElementName(element) {
    const raw = String(element.name || element.signature || "exposed").replace(/`/g, "").trim();
    return raw || "exposed";
  }

  function getExposedElementTypeClass(element) {
    return String(element.kind || element.category || "item")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "item";
  }

  function getExposedElementStatus(element) {
    const fromDesign = normalizeStatus(element.changeStatus);
    if (fromDesign !== "unchanged") return fromDesign;

    const change = element.sourcePath ? getChange(element.sourcePath) : null;
    if (!change) return "unchanged";
    const statusMap = { A: "added", M: "modified", D: "deleted", R: "renamed" };
    return normalizeStatus(statusMap[change.status] || "modified");
  }

  function createStarFileNode(item) {
    const group = document.createElementNS(SVG_NS, "g");
    group.setAttribute("class", `star-node star-file-node${item.changed ? " changed" : ""}`);
    group.setAttribute("transform", `translate(${item.x} ${item.y})`);

    const rect = document.createElementNS(SVG_NS, "rect");
    rect.setAttribute("x", -14);
    rect.setAttribute("y", -16);
    rect.setAttribute("width", 28);
    rect.setAttribute("height", 32);
    rect.setAttribute("rx", 5);
    group.appendChild(rect);

    const label = document.createElementNS(SVG_NS, "text");
    label.setAttribute("class", "star-label");
    label.setAttribute("y", 30);
    label.textContent = item.name;
    group.appendChild(label);

    group.addEventListener("click", evt => {
      evt.stopPropagation();
      setScopePath(getParentPath(item.path));
      currentFile = item.path;
      currentDirectory = getParentPath(item.path);
      localStorage.setItem(STORAGE_KEYS.currentFile, item.path);
      requestStarMapFit();
      selectedModule = getScopeModule();
      renderModuleDetails(selectedModule);
      renderStarMap();
    });

    return group;
  }
