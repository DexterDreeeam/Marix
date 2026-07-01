"use strict";
  function getExposedLabelOffset(item) {
    return {
      x: 0,
      y: getNodeRadius(item) + 14,
      anchor: "middle"
    };
  }

  function collectDesignElementGroups(modulePath) {
    const documents = collectDesignDocuments(modulePath);
    return documents
      .map(({ path, document }) => ({
        name: (document.module && document.module.name) || path,
        elements: (document.elements || []).filter(isPublicExposedElement)
      }))
      .filter(group => group.elements.length > 0);
  }

  function isPublicExposedElement(element) {
    const type = getDesignElementType(element);
    if (type === "module" || type === "re-export") return false;
    return Boolean(element && element.name && type);
  }


  function normalizeStatus(status) {
    const value = String(status || "unchanged").toLowerCase();
    return ["added", "modified", "deleted", "renamed", "unchanged"].includes(value) ? value : "unchanged";
  }

  // Creates the exposed element shape group WITHOUT an inline label.
  // Pass a labelRef object ({ el: null }); after createExposedElementLabel() fills
  // labelRef.el, hover and focus events on this group will toggle "label-hovered"
  // on the paired standalone label in the top-level labels layer.
  function createExposedElementNode(item, labelRef) {
    const element = item.element || {};
    const shape = getExposedElementShape(element);
    const status = getExposedElementStatus(element);
    const typeClass = getExposedElementTypeClass(element);
    const radius = getNodeRadius(item);
    const scale = isFocusHighlightedChangedStatus(status, item) ? 1.2 : 1;
    const group = document.createElementNS(SVG_NS, "g");
    group.setAttribute("class", `exposed-node exposed-${shape} exposed-type-${typeClass} status-${status}${item.focusDimmed ? " dimmed" : ""}${item.focusHighlighted ? " focus-highlighted" : ""}`);
    group.setAttribute("transform", `translate(${item.x} ${item.y}) scale(${scale})`);
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

    // Wire hover/focus events to the paired standalone label (see createExposedElementLabel).
    if (labelRef) {
      const hoverOn = () => { if (labelRef.el) labelRef.el.classList.add("label-hovered"); };
      const hoverOff = () => { if (labelRef.el) labelRef.el.classList.remove("label-hovered"); };
      group.addEventListener("mouseenter", hoverOn);
      group.addEventListener("mouseleave", hoverOff);
      group.addEventListener("focusin", hoverOn);
      group.addEventListener("focusout", hoverOff);
    }

    group.addEventListener("click", async evt => {
      evt.stopPropagation();
      await showCodeSegmentsPopover(getCodeTitle(element), getDesignElementCodeSegments(element), getExposedElementStatus(element));
    });

    return group;
  }

  // Creates a standalone <text> label for an exposed element node, positioned
  // at absolute layer coordinates so it can live in a top-level labels group that
  // renders above all node shapes (preventing symbol-on-label occlusion).
  function createExposedElementLabel(item) {
    const element = item.element || {};
    const radius = getNodeRadius(item);
    const status = getExposedElementStatus(element);
    const classes = ["exposed-label"];
    if (item.focusDimmed) classes.push("dimmed");
    if (item.focusHighlighted) classes.push("focus-highlighted");
    classes.push(`status-${status}`);
    const label = document.createElementNS(SVG_NS, "text");
    label.setAttribute("class", classes.join(" "));
    // Absolute coordinates: item.x/y are in layer space; labelY is negative (above node).
    label.setAttribute("x", item.x + (item.labelX || 0));
    label.setAttribute("y", item.y + (item.labelY !== undefined ? item.labelY : -(radius + 10)));
    label.setAttribute("text-anchor", item.labelAnchor || "middle");
    label.textContent = item.label || getShortElementName(element);
    return label;
  }

  function isFocusHighlightedChangedStatus(status, item) {
    return item.focusHighlighted && ["added", "modified", "renamed"].includes(normalizeStatus(status));
  }

  function getExposedElementShape(element) {
    const type = getDesignElementType(element);
    if (["trait", "interface", "global-interface", "public-api", "public-global-interface"].includes(type)) return "triangle";
    if (["struct", "class", "data"].includes(type)) return "square";
    if (["function", "fn", "method"].includes(type)) return "circle";
    if (["enum", "type-alias", "global", "global-variable", "const", "static"].includes(type)) return "star";

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
    const raw = String(element.name || "exposed").replace(/`/g, "").trim();
    return raw || "exposed";
  }

  function getExposedElementTypeClass(element) {
    return getDesignElementType(element)
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "item";
  }

  function getExposedElementStatus(element) {
    const fromDesign = getExplicitStarMapChangeStatus(element);
    if (fromDesign) return fromDesign;

    const sourcePath = getDesignElementPrimarySourcePath(element);
    return sourcePath ? normalizeStatus(getPathChangeStatus(sourcePath)) : "unchanged";
  }

  function isElementFromFocusedFile(element, focusedFile) {
    return Boolean(focusedFile && getDesignElementPrimarySourcePath(element) === focusedFile);
  }

  function getExplicitStarMapChangeStatus(item) {
    if (!item || typeof item.changeStatus !== "string" || item.changeStatus.trim() === "") return null;
    return normalizeStatus(item.changeStatus);
  }

  function getDesignElementType(element) {
    return String((element && (element.type || element.kind || element.category)) || "item").toLowerCase();
  }

  function getDesignElementCodeSegments(element) {
    if (!element) return [];
    if (Array.isArray(element.codeSegments)) {
      return element.codeSegments
        .filter(segment => segment && segment.sourcePath && segment.lineStart && segment.lineEnd)
        .map(segment => ({
          sourcePath: String(segment.sourcePath),
          lineStart: Number(segment.lineStart),
          lineEnd: Number(segment.lineEnd),
          language: segment.language || "rust",
          changeStatus: segment.changeStatus ? normalizeStatus(segment.changeStatus) : undefined,
          addedLines: normalizeCodeSegmentRanges(segment.addedLines),
          modifiedLines: normalizeCodeSegmentRanges(segment.modifiedLines)
        }));
    }
    if (element.sourcePath && element.lineStart && element.lineEnd) {
      return [{
        sourcePath: String(element.sourcePath),
        lineStart: Number(element.lineStart),
        lineEnd: Number(element.lineEnd),
        language: element.language || "rust",
        changeStatus: element.changeStatus ? normalizeStatus(element.changeStatus) : undefined,
        addedLines: normalizeCodeSegmentRanges(element.addedLines),
        modifiedLines: normalizeCodeSegmentRanges(element.modifiedLines)
      }];
    }
    return [];
  }

  function normalizeCodeSegmentRanges(ranges) {
    if (!Array.isArray(ranges)) return [];
    return ranges
      .map(range => ({
        lineStart: Number(range && range.lineStart),
        lineEnd: Number(range && range.lineEnd)
      }))
      .filter(range => Number.isFinite(range.lineStart) && Number.isFinite(range.lineEnd) && range.lineStart > 0 && range.lineEnd >= range.lineStart);
  }

  function getDesignElementPrimarySourcePath(element) {
    const segments = getDesignElementCodeSegments(element);
    return segments.length > 0 ? segments[0].sourcePath : "";
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
