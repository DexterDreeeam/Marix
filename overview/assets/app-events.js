"use strict";
  function bindEvents() {
    const searchInput = document.getElementById("search-input");
    let searchTimeout;
    bindGlobalTooltips();

    searchInput.addEventListener("input", () => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(() => renderTree(searchInput.value.trim()), 200);
    });

    document.getElementById("btn-language").addEventListener("click", () => {
      language = language === "en" ? "zh" : "en";
      localStorage.setItem(STORAGE_KEYS.language, language);
      applyLanguage();
    });

    document.getElementById("btn-reset-data-source").addEventListener("click", () => {
      resetDataSourceChoice();
    });

    document.getElementById("btn-star-map-view").addEventListener("click", () => {
      setOverviewMode(overviewMode === "star" ? "file" : "star");
    });

    document.getElementById("btn-collapse-all").addEventListener("click", () => {
      collapseAllDirectories();
    });

    document.getElementById("btn-view-whole-file").addEventListener("click", () => {
      viewWholeFile = !viewWholeFile;
      saveBooleanSetting(STORAGE_KEYS.viewWholeFile, viewWholeFile);
      updateToolButton("btn-view-whole-file", "viewWholeFileTool", viewWholeFile);
      if (overviewMode === "star") return;
      if (currentFile) openFile(currentFile);
      else if (currentDirectory) renderDirectoryChanges(currentDirectory);
    });

    document.getElementById("code-popover-backdrop").addEventListener("click", hideCodePopover);
    document.getElementById("btn-close-code-popover").addEventListener("click", hideCodePopover);

    document.getElementById("btn-reset-star-map").addEventListener("click", () => {
      requestStarMapFit();
      renderStarMap();
    });

    const svg = document.getElementById("star-map-svg");
    svg.addEventListener("wheel", evt => {
      evt.preventDefault();
      const delta = evt.deltaY > 0 ? 0.9 : 1.1;
      starTransform.scale = Math.max(0.35, Math.min(2.6, starTransform.scale * delta));
      starAutoFit = false;
      applyStarMapTransform();
    }, { passive: false });

    svg.addEventListener("pointerdown", evt => {
      const interactiveNode = evt.target.closest && evt.target.closest(".star-node, .exposed-node");
      logStarMapState("canvas-pointerdown", {
        targetTag: evt.target && evt.target.tagName,
        targetClass: evt.target && evt.target.getAttribute && evt.target.getAttribute("class"),
        interactive: Boolean(interactiveNode)
      });
      if (interactiveNode) return;

      selectStarMapModule(scopePath || SOURCE_ROOT, { fit: false });
      panState = { x: evt.clientX, y: evt.clientY, startX: starTransform.x, startY: starTransform.y };
      svg.setPointerCapture(evt.pointerId);
    });

    svg.addEventListener("pointermove", evt => {
      if (!panState) return;
      starTransform.x = panState.startX + evt.clientX - panState.x;
      starTransform.y = panState.startY + evt.clientY - panState.y;
      starAutoFit = false;
      applyStarMapTransform();
    });

    svg.addEventListener("pointerup", () => {
      panState = null;
    });

    svg.addEventListener("pointerleave", () => {
      panState = null;
    });
  }

  function bindGlobalTooltips() {
    document.addEventListener("pointerover", evt => {
      const target = evt.target.closest("[data-tooltip]");
      if (target) showTooltip(target);
    });

    document.addEventListener("pointerout", evt => {
      const target = evt.target.closest("[data-tooltip]");
      if (target && !target.contains(evt.relatedTarget)) hideTooltip(target);
    });

    document.addEventListener("focusin", evt => {
      const target = evt.target.closest("[data-tooltip]");
      if (target) showTooltip(target);
    });

    document.addEventListener("focusout", evt => {
      const target = evt.target.closest("[data-tooltip]");
      if (target) hideTooltip(target);
    });
  }

  document.addEventListener("DOMContentLoaded", init);
