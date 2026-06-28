"use strict";
  let workflowRenderCounter = 0;
  let workflowMermaidInitialized = false;

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
    showCodePopover("workflow", renderWorkflowDiagram(diagramId, source), "", "code-popover-workflow");
    await renderWorkflowMermaidDiagram(diagramId, source);
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

  function renderWorkflowDiagram(diagramId, source) {
    return `
      <div class="workflow-diagram-shell">
        <div id="${escapeHtml(diagramId)}" class="mermaid workflow-mermaid">${escapeHtml(source)}</div>
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

  function renderWorkflowSourceFallback(source) {
    return `
      <div class="workflow-source-fallback">
        <p>${escapeHtml(t("workflowRenderUnavailable"))}</p>
        <pre><code>${escapeHtml(source)}</code></pre>
      </div>
    `;
  }
