"use strict";
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


  function normalizeStatus(status) {
    const value = String(status || "unchanged").toLowerCase();
    return ["added", "modified", "deleted", "renamed", "unchanged"].includes(value) ? value : "unchanged";
  }

  function createExposedElementNode(item) {
    const element = item.element || {};
    const shape = getExposedElementShape(element);
    const status = getExposedElementStatus(element);
    const typeClass = getExposedElementTypeClass(element);
    const radius = getNodeRadius(item);
    const group = document.createElementNS(SVG_NS, "g");
    group.setAttribute("class", `exposed-node exposed-${shape} exposed-type-${typeClass} status-${status}${item.focusDimmed ? " dimmed" : ""}`);
    group.setAttribute("transform", `translate(${item.x} ${item.y})`);
    group.setAttribute("tabindex", "0");

    const hit = document.createElementNS(SVG_NS, "circle");
    hit.setAttribute("class", "exposed-hit-target");
    hit.setAttribute("r", Math.max(24, radius * 2.2));
    group.appendChild(hit);

    if (shape === "square") {
      const size = radius * 2.25;
      const rect = document.createElementNS(SVG_NS, "rect");
      rect.setAttribute("x", -size / 2);
      rect.setAttribute("y", -size / 2);
      rect.setAttribute("width", size);
      rect.setAttribute("height", size);
      rect.setAttribute("rx", 3);
      group.appendChild(rect);
    } else if (shape === "star") {
      const polygon = document.createElementNS(SVG_NS, "polygon");
      polygon.setAttribute("points", getStarPoints(radius));
      group.appendChild(polygon);
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
    const kind = String(element.kind || "").toLowerCase();
    if (["trait", "interface", "global-interface", "public-api", "public-global-interface"].includes(kind)) return "triangle";
    if (["struct", "class", "data"].includes(kind)) return "square";
    if (["function", "fn", "method"].includes(kind)) return "circle";
    if (["enum", "type-alias", "global", "global-variable", "const", "static"].includes(kind)) return "star";

    const explicit = String(element.shape || "").toLowerCase();
    if (["circle", "square", "triangle", "star"].includes(explicit)) return explicit;

    return "circle";
  }

  function getStarPoints(radius) {
    const points = [];
    const innerRadius = radius * 0.46;
    for (let i = 0; i < 10; i++) {
      const angle = -Math.PI / 2 + (Math.PI * i) / 5;
      const r = i % 2 === 0 ? radius : innerRadius;
      points.push(`${Math.cos(angle) * r},${Math.sin(angle) * r}`);
    }
    return points.join(" ");
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
    const fromDesign = getExplicitStarMapChangeStatus(element);
    if (fromDesign) return fromDesign;

    const change = element.sourcePath ? getChange(element.sourcePath) : null;
    if (!change) return "unchanged";
    const statusMap = { A: "added", M: "modified", D: "deleted", R: "renamed" };
    return normalizeStatus(statusMap[change.status] || "modified");
  }

  function isElementFromFocusedFile(element, focusedFile) {
    return Boolean(focusedFile && element && element.sourcePath === focusedFile);
  }

  function getExplicitStarMapChangeStatus(item) {
    if (!item || typeof item.changeStatus !== "string" || item.changeStatus.trim() === "") return null;
    return normalizeStatus(item.changeStatus);
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
      focusStarMapFile(item.path, { openPopover: true });
    });

    return group;
  }
