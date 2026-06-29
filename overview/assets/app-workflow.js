"use strict";
  let workflowRenderCounter = 0;
  let workflowMermaidInitialized = false;
  let workflowZoomState = null;
  const WORKFLOW_ZOOM_MIN = 0.05;
  const WORKFLOW_ZOOM_MAX = 8;
  const WORKFLOW_ZOOM_STEP = 1.18;

  async function showWorkflowPopover() {
    const modulePath = getActiveWorkflowModulePath();
    const workflowPath = getWorkflowDocumentPath(modulePath);
    let entry = null;
    try {
      entry = await ensureFileContent(workflowPath);
    } catch (error) {
      logOverviewError(`workflow content load failed: ${workflowPath}`, error);
    }

    const source = entry && typeof entry.content === "string" ? entry.content.trim() : "";
    if (!source) {
      showCodePopover("workflow", renderWorkflowUnavailable(), "", "code-popover-workflow");
      return;
    }

    const diagramId = `workflow-mermaid-${++workflowRenderCounter}`;
    const viewportId = `${diagramId}-viewport`;
    workflowZoomState = null;
    showCodePopover("workflow", renderWorkflowDiagram(diagramId, viewportId, source), "", "code-popover-workflow");
    await renderWorkflowMermaidDiagram(diagramId, source);
    initializeWorkflowDiagramZoom(viewportId);
  }

  function getActiveWorkflowModulePath() {
    if (starMapSelection && starMapSelection.kind === "module" && starMapSelection.path) {
      return normalizeScopePath(starMapSelection.path);
    }
    return normalizeScopePath(scopePath || SOURCE_ROOT);
  }

  function getWorkflowDocumentPath(modulePath) {
    return `${normalizeScopePath(modulePath || SOURCE_ROOT)}/.workflow.mmd`;
  }

  function renderWorkflowUnavailable() {
    return `
      <div class="workflow-empty">
        ${escapeHtml(t("workflowUnavailable"))}
      </div>
    `;
  }

  function renderWorkflowDiagram(diagramId, viewportId, source) {
    return `
      <div class="workflow-viewer">
        <div id="${escapeHtml(viewportId)}" class="workflow-diagram-shell">
          <div class="workflow-stage">
            <div id="${escapeHtml(diagramId)}" class="mermaid workflow-mermaid">${escapeHtml(source)}</div>
          </div>
        </div>
        <div class="workflow-reset-toolset" role="toolbar" aria-label="Workflow reset tools">
          <button class="tool-btn" type="button" data-workflow-zoom="fit" aria-label="${escapeHtml(t("workflowReset"))}"><i class="bi bi-crosshair"></i></button>
        </div>
      </div>
    `;
  }

  async function renderWorkflowMermaidDiagram(diagramId, source) {
    const diagram = document.getElementById(diagramId);
    if (!diagram) return;
    if (!window.mermaid) {
      diagram.outerHTML = renderWorkflowSourceFallback(source);
      return;
    }

    try {
      initializeWorkflowMermaid();
      diagram.removeAttribute("data-processed");
      if (typeof window.mermaid.run === "function") {
        await window.mermaid.run({ nodes: [diagram] });
      } else if (typeof window.mermaid.render === "function") {
        const rendered = await window.mermaid.render(`${diagramId}-svg`, source);
        diagram.innerHTML = rendered.svg || "";
        if (typeof rendered.bindFunctions === "function") rendered.bindFunctions(diagram);
      } else {
        diagram.outerHTML = renderWorkflowSourceFallback(source);
      }
    } catch (error) {
      logOverviewError("workflow mermaid render failed", error);
      const failedDiagram = document.getElementById(diagramId);
      if (failedDiagram) failedDiagram.outerHTML = renderWorkflowSourceFallback(source);
    }
  }

  function initializeWorkflowMermaid() {
    if (workflowMermaidInitialized || !window.mermaid || typeof window.mermaid.initialize !== "function") return;
    window.mermaid.initialize({
      startOnLoad: false,
      theme: "dark",
      securityLevel: "strict"
    });
    workflowMermaidInitialized = true;
  }

  function initializeWorkflowDiagramZoom(viewportId) {
    const viewport = document.getElementById(viewportId);
    const svg = viewport && viewport.querySelector("svg");
    const stage = viewport && viewport.querySelector(".workflow-stage");
    if (!viewport || !svg || !stage) return;

    const size = getWorkflowSvgSize(svg);
    if (!size.width || !size.height) return;

    // Pin the stage to the diagram's intrinsic size and pan/zoom via a single
    // transform, mirroring the star map's translate/scale model. This keeps zoom
    // continuous (no scroll-position jumps) and lets the cursor act as an anchor.
    svg.style.width = `${size.width}px`;
    svg.style.height = `${size.height}px`;
    svg.style.maxWidth = "none";
    svg.style.maxHeight = "none";
    stage.style.width = `${size.width}px`;
    stage.style.height = `${size.height}px`;

    workflowZoomState = {
      viewport,
      stage,
      baseWidth: size.width,
      baseHeight: size.height,
      scale: 1,
      x: 0,
      y: 0,
      pan: null
    };
    bindWorkflowZoomControls(workflowZoomState);
    bindWorkflowPan(workflowZoomState);
    viewport.addEventListener("wheel", handleWorkflowWheelZoom, { passive: false });
    requestAnimationFrame(() => fitWorkflowDiagram(workflowZoomState));
  }

  function getWorkflowSvgSize(svg) {
    const viewBox = String(svg.getAttribute("viewBox") || "").trim().split(/\s+/).map(Number);
    if (viewBox.length === 4 && viewBox.every(Number.isFinite) && viewBox[2] > 0 && viewBox[3] > 0) {
      return { width: viewBox[2], height: viewBox[3] };
    }

    const width = parseSvgDimension(svg.getAttribute("width"));
    const height = parseSvgDimension(svg.getAttribute("height"));
    if (width > 0 && height > 0) return { width, height };

    let box = null;
    try {
      box = svg.getBBox ? svg.getBBox() : null;
    } catch (error) {
      logOverviewError("workflow svg bounds read failed", error);
    }
    return {
      width: box && box.width > 0 ? box.width : 1,
      height: box && box.height > 0 ? box.height : 1
    };
  }

  function parseSvgDimension(value) {
    const parsed = Number.parseFloat(String(value || "").replace("px", ""));
    return Number.isFinite(parsed) ? parsed : 0;
  }

  function bindWorkflowZoomControls(state) {
    const viewer = state.viewport.closest(".workflow-viewer");
    if (!viewer) return;
    for (const button of viewer.querySelectorAll("[data-workflow-zoom]")) {
      button.addEventListener("click", () => {
        if (button.dataset.workflowZoom === "fit") {
          fitWorkflowDiagram(state);
        } else {
          zoomWorkflowDiagramAtCenter(state, button.dataset.workflowZoom === "in" ? WORKFLOW_ZOOM_STEP : 1 / WORKFLOW_ZOOM_STEP);
        }
      });
    }
  }

  function bindWorkflowPan(state) {
    const viewport = state.viewport;
    viewport.addEventListener("pointerdown", evt => {
      if (evt.button !== 0) return;
      evt.preventDefault();
      state.pan = { x: evt.clientX, y: evt.clientY, startX: state.x, startY: state.y };
      viewport.classList.add("workflow-grabbing");
      if (typeof viewport.setPointerCapture === "function") {
        viewport.setPointerCapture(evt.pointerId);
      }
    });
    viewport.addEventListener("pointermove", evt => {
      if (!state.pan) return;
      state.x = state.pan.startX + evt.clientX - state.pan.x;
      state.y = state.pan.startY + evt.clientY - state.pan.y;
      applyWorkflowTransform(state);
    });
    const endPan = () => {
      state.pan = null;
      viewport.classList.remove("workflow-grabbing");
    };
    viewport.addEventListener("pointerup", endPan);
    viewport.addEventListener("pointercancel", endPan);
    viewport.addEventListener("pointerleave", endPan);
  }

  function handleWorkflowWheelZoom(evt) {
    if (!workflowZoomState || evt.currentTarget !== workflowZoomState.viewport) return;
    evt.preventDefault();
    evt.stopPropagation();
    const factor = evt.deltaY > 0 ? 1 / WORKFLOW_ZOOM_STEP : WORKFLOW_ZOOM_STEP;
    zoomWorkflowDiagramAtPointer(workflowZoomState, factor, evt.clientX, evt.clientY);
  }

  function fitWorkflowDiagram(state) {
    if (!state || !state.viewport.isConnected) return;
    const availableWidth = Math.max(1, state.viewport.clientWidth - 48);
    const availableHeight = Math.max(1, state.viewport.clientHeight - 48);
    const scale = clampWorkflowZoom(Math.min(availableWidth / state.baseWidth, availableHeight / state.baseHeight));
    state.scale = scale;
    state.x = (state.viewport.clientWidth - state.baseWidth * scale) / 2;
    state.y = (state.viewport.clientHeight - state.baseHeight * scale) / 2;
    applyWorkflowTransform(state);
  }

  function zoomWorkflowDiagramAtCenter(state, factor) {
    zoomWorkflowDiagramAtPoint(state, factor, state.viewport.clientWidth / 2, state.viewport.clientHeight / 2);
  }

  function zoomWorkflowDiagramAtPointer(state, factor, clientX, clientY) {
    const rect = state.viewport.getBoundingClientRect();
    zoomWorkflowDiagramAtPoint(state, factor, clientX - rect.left, clientY - rect.top);
  }

  function zoomWorkflowDiagramAtPoint(state, factor, pointerX, pointerY) {
    const previousScale = state.scale;
    const nextScale = clampWorkflowZoom(previousScale * factor);
    if (nextScale === previousScale) return;
    // Keep the content point under the cursor fixed: solve for translate so the
    // pointer maps to the same stage coordinate before and after the zoom.
    const contentX = (pointerX - state.x) / previousScale;
    const contentY = (pointerY - state.y) / previousScale;
    state.scale = nextScale;
    state.x = pointerX - contentX * nextScale;
    state.y = pointerY - contentY * nextScale;
    applyWorkflowTransform(state);
  }

  function applyWorkflowTransform(state) {
    state.stage.style.transform = `translate(${state.x}px, ${state.y}px) scale(${state.scale})`;
  }

  function clampWorkflowZoom(scale) {
    return Math.max(WORKFLOW_ZOOM_MIN, Math.min(WORKFLOW_ZOOM_MAX, Number.isFinite(scale) ? scale : 1));
  }

  function renderWorkflowSourceFallback(source) {
    return `
      <div class="workflow-source-fallback">
        <p>${escapeHtml(t("workflowRenderUnavailable"))}</p>
        <pre><code>${escapeHtml(source)}</code></pre>
      </div>
    `;
  }
