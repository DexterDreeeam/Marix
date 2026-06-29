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
        <div class="workflow-toolbar" aria-label="Workflow zoom controls">
          <button class="workflow-zoom-button" type="button" data-workflow-zoom="out" aria-label="${escapeHtml(t("workflowZoomOut"))}">-</button>
          <button class="workflow-zoom-button workflow-zoom-fit" type="button" data-workflow-zoom="fit" data-i18n="workflowFit">${escapeHtml(t("workflowFit"))}</button>
          <button class="workflow-zoom-button" type="button" data-workflow-zoom="in" aria-label="${escapeHtml(t("workflowZoomIn"))}">+</button>
        </div>
        <div id="${escapeHtml(viewportId)}" class="workflow-diagram-shell">
          <div id="${escapeHtml(diagramId)}" class="mermaid workflow-mermaid">${escapeHtml(source)}</div>
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
    if (!viewport || !svg) return;

    const size = getWorkflowSvgSize(svg);
    if (!size.width || !size.height) return;

    workflowZoomState = {
      viewport,
      svg,
      container: svg.closest(".workflow-mermaid") || svg,
      baseWidth: size.width,
      baseHeight: size.height,
      scale: 1
    };
    bindWorkflowZoomControls(workflowZoomState);
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
    applyWorkflowZoom(state, scale);
    centerWorkflowDiagram(state);
  }

  function zoomWorkflowDiagramAtCenter(state, factor) {
    const centerX = state.viewport.scrollLeft + state.viewport.clientWidth / 2;
    const centerY = state.viewport.scrollTop + state.viewport.clientHeight / 2;
    const previousScale = state.scale;
    applyWorkflowZoom(state, clampWorkflowZoom(previousScale * factor));
    const ratio = state.scale / previousScale;
    state.viewport.scrollLeft = centerX * ratio - state.viewport.clientWidth / 2;
    state.viewport.scrollTop = centerY * ratio - state.viewport.clientHeight / 2;
  }

  function zoomWorkflowDiagramAtPointer(state, factor, clientX, clientY) {
    const rect = state.viewport.getBoundingClientRect();
    const pointerX = clientX - rect.left;
    const pointerY = clientY - rect.top;
    const contentX = state.viewport.scrollLeft + pointerX;
    const contentY = state.viewport.scrollTop + pointerY;
    const previousScale = state.scale;
    applyWorkflowZoom(state, clampWorkflowZoom(previousScale * factor));
    const ratio = state.scale / previousScale;
    state.viewport.scrollLeft = contentX * ratio - pointerX;
    state.viewport.scrollTop = contentY * ratio - pointerY;
  }

  function applyWorkflowZoom(state, scale) {
    state.scale = scale;
    const width = Math.max(1, state.baseWidth * scale);
    const height = Math.max(1, state.baseHeight * scale);
    state.container.style.width = `${width}px`;
    state.container.style.height = `${height}px`;
    state.svg.style.width = `${width}px`;
    state.svg.style.height = `${height}px`;
    state.svg.style.maxWidth = "none";
    state.svg.style.maxHeight = "none";
    state.viewport.classList.toggle("workflow-diagram-overflow", width > state.viewport.clientWidth || height > state.viewport.clientHeight);
  }

  function centerWorkflowDiagram(state) {
    state.viewport.scrollLeft = Math.max(0, (state.container.offsetWidth - state.viewport.clientWidth) / 2);
    state.viewport.scrollTop = Math.max(0, (state.container.offsetHeight - state.viewport.clientHeight) / 2);
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
