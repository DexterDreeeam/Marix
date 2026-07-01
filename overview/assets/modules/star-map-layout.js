const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

function layoutScopeStarMap(scopeNode, options) {
  const {
    sourceRoot,
    getParentPath,
    findModule,
    getChange,
    getImmediateFiles,
    isHiddenPath,
    collectDesignElementGroups,
    normalizeStatus,
    getShortElementName
  } = options;

  const layout = [];
  const rootItem = {
    kind: "module",
    node: scopeNode,
    changed: scopeNode.changed,
    depth: 0,
    parent: null,
    visualRadius: 38,
    depthClass: "near",
    x: 0,
    y: 0
  };
  layout.push(rootItem);

  const moduleItems = (scopeNode.children || []).map(child => ({
    kind: "module",
    node: child,
    changed: child.changed,
    edgeKind: "child"
  }));
  const parentPath = getParentPath(scopeNode.path);
  const parentNode = parentPath && parentPath !== scopeNode.path ? findModule(parentPath) : null;
  const parentItems = parentNode ? [{
    kind: "module",
    node: parentNode,
    changed: parentNode.changed,
    edgeKind: "parent",
    nodeRole: "parent"
  }] : [];
  const children = parentItems.concat(moduleItems);

  const count = children.length;
  const radius = Math.max(210, Math.min(360, 130 + count * 18));
  for (let i = 0; i < count; i++) {
    const id = getLayoutItemId(children[i], sourceRoot);
    const segment = count > 0 ? (Math.PI * 2) / count : Math.PI * 2;
    const baseAngle = count === 1 ? -Math.PI / 2 : segment * i - Math.PI / 2;
    const angleJitter = (stableRandom(`${id}:angle`) - 0.5) * segment * 0.58;
    const radiusJitter = 0.84 + stableRandom(`${id}:radius`) * 0.32;
    const z = stableRandom(`${id}:z`);
    const baseVisualRadius = children[i].edgeKind === "parent" ? 33 : 23;
    const visualRadius = baseVisualRadius + Math.round(z * 8);
    layout.push({
      ...children[i],
      depth: 1,
      parent: rootItem,
      visualRadius,
      depthClass: z > 0.66 ? "near" : (z < 0.33 ? "far" : "mid"),
      x: Math.cos(baseAngle + angleJitter) * radius * radiusJitter,
      y: Math.sin(baseAngle + angleJitter) * radius * radiusJitter
    });
  }

  const moduleObstacles = layout.filter(item => item.kind === "module");
  const exposedItems = layoutExposedElements(scopeNode, radius, moduleObstacles, {
    collectDesignElementGroups,
    normalizeStatus,
    getShortElementName
  });
  layout.push(...exposedItems);
  return layout;
}

// Depth-limited element visibility.
// The viewed module's depth D equals the number of path segments in its module
// path (src = 1, src/common = 2, src/agent/engine = 3). The star map renders
// exposed element nodes only for the current level or one level down
// (source_depth === D or D + 1), so a top-level scope such as `src` no longer
// renders every descendant element at once. D is derived from the module path,
// which is robust for aggregator modules (e.g. src, src/common) that own no
// elements themselves.
function getModuleViewDepth(modulePath) {
  return String(modulePath || "").split("/").filter(Boolean).length;
}

function getElementSourceDepth(element) {
  const depth = Number(element && element.source_depth);
  return Number.isFinite(depth) && depth > 0 ? depth : null;
}

function isElementWithinViewDepth(element, viewDepth) {
  const depth = getElementSourceDepth(element);
  // Elements without depth metadata are kept visible (fail-open) so stale data
  // never blanks the map; populated elements follow the strict D / D + 1 rule.
  if (depth === null) return true;
  return depth === viewDepth || depth === viewDepth + 1;
}

function layoutExposedElements(scopeNode, moduleRadius, moduleObstacles, options) {
  const { collectDesignElementGroups, normalizeStatus } = options;
  const groups = collectDesignElementGroups(scopeNode.path);
  const viewDepth = getModuleViewDepth(scopeNode.path);
  const elements = groups.flatMap((group, groupIndex) => (group.elements || []).map((element, elementIndex) => ({
    groupName: group.name,
    groupIndex,
    element,
    elementIndex,
    id: element.id || `${scopeNode.path}:${group.name}:${groupIndex}:${element.name}:${elementIndex}`
  }))).filter(entry => isElementWithinViewDepth(entry.element, viewDepth));
  const count = elements.length;
  if (count === 0) return [];

  const layoutRadiusX = 360 + Math.min(170, count * 4);
  const layoutRadiusY = 240 + Math.min(120, count * 3);

  const initialLayout = elements
    .sort((a, b) => stableHash(a.id) - stableHash(b.id))
    .map((entry, index) => {
      const status = normalizeStatus(entry.element.changeStatus);
      const changedWeight = status === "unchanged" ? 1 : 0.62;
      const t = (index + 0.5) / count;
      const radiusFactor = Math.max(0.22, Math.sqrt(t) * changedWeight * 0.84);
      const angle = index * GOLDEN_ANGLE + stableRandom(`${entry.id}:angle`) * 0.26;
      const jitterX = (stableRandom(`${entry.id}:jx`) - 0.5) * 28;
      const jitterY = (stableRandom(`${entry.id}:jy`) - 0.5) * 22;
      const x = Math.cos(angle) * layoutRadiusX * radiusFactor + jitterX;
      const y = Math.sin(angle) * layoutRadiusY * radiusFactor + jitterY;

      return {
        kind: "exposed",
        groupName: entry.groupName,
        element: entry.element,
        changed: normalizeStatus(entry.element.changeStatus) !== "unchanged",
        depth: 2,
        parent: null,
        visualRadius: 8 + Math.round(stableRandom(`${entry.id}:size`) * 3),
        depthClass: "near",
        x,
        y
      };
    });

  return layoutWithD3Force(initialLayout, moduleRadius, moduleObstacles, options);
}

function layoutWithD3Force(items, moduleRadius, moduleObstacles, options) {
  if (window.d3 && typeof window.d3.forceSimulation === "function") {
    return layoutWithD3ForceSimulation(items, moduleRadius, moduleObstacles, options);
  }
  return relaxExposedLayout(items, moduleRadius, moduleObstacles, options);
}

function layoutWithD3ForceSimulation(items, moduleRadius, moduleObstacles, options) {
  const nodes = items.map(item => {
    const box = getExposedLabelBox(item, options);
    return {
      ...item,
      targetX: item.x,
      targetY: item.y,
      collisionRadius: Math.max(18, Math.hypot(box.width / 2, box.height / 2) * 0.74 + getNodeRadius(item) + 3)
    };
  });
  const obstacles = (moduleObstacles || []).map(item => ({
    obstacle: true,
    x: item.x,
    y: item.y,
    fx: item.x,
    fy: item.y,
    collisionRadius: getNodeRadius(item) + 42
  }));

  const simulation = window.d3.forceSimulation(nodes.concat(obstacles))
    .alpha(1)
    .alphaMin(0.001)
    .velocityDecay(0.52)
    .force("x", window.d3.forceX(d => d.obstacle ? d.x : d.targetX).strength(d => d.obstacle ? 1 : 0.24))
    .force("y", window.d3.forceY(d => d.obstacle ? d.y : d.targetY).strength(d => d.obstacle ? 1 : 0.24))
    .force("collide", window.d3.forceCollide(d => d.collisionRadius).strength(0.82).iterations(3))
    .stop();

  for (let i = 0; i < 110; i++) simulation.tick();
  return finalizeExposedLayout(nodes, options);
}

function relaxExposedLayout(items, moduleRadius, moduleObstacles, options) {
  const relaxed = items.map(item => ({ ...item }));
  for (let iteration = 0; iteration < 18; iteration++) {
    for (let i = 0; i < relaxed.length; i++) {
      for (let j = i + 1; j < relaxed.length; j++) {
        const a = relaxed[i];
        const b = relaxed[j];
        const aBox = getExposedLabelBox(a, options);
        const bBox = getExposedLabelBox(b, options);
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const minX = (aBox.width + bBox.width) / 2 + 8;
        const minY = (aBox.height + bBox.height) / 2 + 6;
        const overlapX = minX - Math.abs(dx);
        const overlapY = minY - Math.abs(dy);
        if (overlapX > 0 && overlapY > 0) {
          const pushAxisX = overlapX < overlapY;
          const direction = pushAxisX
            ? (dx === 0 ? (stableRandom(`${getExposedElementId(a)}:${getExposedElementId(b)}:x`) > 0.5 ? 1 : -1) : Math.sign(dx))
            : (dy === 0 ? (stableRandom(`${getExposedElementId(a)}:${getExposedElementId(b)}:y`) > 0.5 ? 1 : -1) : Math.sign(dy));
          const push = (pushAxisX ? overlapX : overlapY) / 2 + 2;
          if (pushAxisX) {
            a.x -= direction * push;
            b.x += direction * push;
          } else {
            a.y -= direction * push;
            b.y += direction * push;
          }
        }
      }
      for (const obstacle of moduleObstacles || []) {
        const dx = relaxed[i].x - obstacle.x;
        const dy = relaxed[i].y - obstacle.y;
        const distanceToObstacle = Math.max(1, Math.hypot(dx, dy));
        const minDistance = getNodeRadius(obstacle) + getNodeRadius(relaxed[i]) + 42;
        if (distanceToObstacle < minDistance) {
          const scale = minDistance / distanceToObstacle;
          relaxed[i].x = obstacle.x + dx * scale;
          relaxed[i].y = obstacle.y + dy * scale;
        }
      }
    }
  }

  return finalizeExposedLayout(relaxed, options);
}

function finalizeExposedLayout(items, options) {
  return items.map(item => {
    const maxX = 900;
    const maxY = 590;
    item.x = Math.max(-maxX, Math.min(maxX, item.x));
    item.y = Math.max(-maxY, Math.min(maxY, item.y));
    const label = options.getShortElementName(item.element || {});
    item.label = label;
    item.labelX = 0;
    item.labelY = -getNodeRadius(item) - 10;
    item.labelAnchor = "middle";
    return item;
  });
}

function getExposedLabelBox(item, options) {
  const label = options.getShortElementName(item.element || {});
  return {
    width: Math.max(32, label.length * 4.6 + 12),
    height: 24
  };
}

function getExposedElementId(item) {
  const element = item.element || {};
  return element.id || `${item.groupName || "group"}:${element.name || "element"}`;
}

function getLayoutItemId(item, sourceRoot) {
  if (item.kind === "module" && item.node) return item.node.path || sourceRoot;
  return item.path || item.name || "item";
}

function stableHash(value) {
  let hash = 2166136261;
  const text = String(value || "");
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function stableRandom(value) {
  return stableHash(value) / 4294967295;
}

function getNodeRadius(item) {
  return item && item.visualRadius ? item.visualRadius : (item && item.depth === 0 ? 38 : 26);
}

function computeEdgePath(from, to, sourceRoot) {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.max(1, Math.hypot(dx, dy));
  const ux = dx / length;
  const uy = dy / length;
  const startGap = getNodeRadius(from) + 10;
  const endGap = getNodeRadius(to) + 14;
  const x1 = from.x + ux * startGap;
  const y1 = from.y + uy * startGap;
  const x2 = to.x - ux * endGap;
  const y2 = to.y - uy * endGap;
  const midpointX = (x1 + x2) / 2;
  const midpointY = (y1 + y2) / 2;
  const curveKey = `${getLayoutItemId(from, sourceRoot)}->${getLayoutItemId(to, sourceRoot)}:${to.edgeKind || "child"}`;
  const curve = (stableRandom(curveKey) - 0.5) * 72;
  const controlX = midpointX + -uy * curve;
  const controlY = midpointY + ux * curve;
  return `M ${x1.toFixed(2)} ${y1.toFixed(2)} Q ${controlX.toFixed(2)} ${controlY.toFixed(2)} ${x2.toFixed(2)} ${y2.toFixed(2)}`;
}
