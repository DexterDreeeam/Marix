"use strict";
  let overviewEventsBound = false;

  function bindEvents() {
    if (overviewEventsBound) return;
    overviewEventsBound = true;
    const searchInput = document.getElementById("search-input");
    let searchTimeout;
    bindGlobalTooltips();
    bindActiveReloadBannerEvents();

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
      treeChangedFilesOnly = !treeChangedFilesOnly;
      saveBooleanSetting(STORAGE_KEYS.treeChangedFilesOnly, treeChangedFilesOnly);
      updateTreeFilterButton();
      renderTree(searchInput.value.trim());
    });

    document.getElementById("btn-hide-private-code").addEventListener("click", () => {
      hidePrivateCode = !hidePrivateCode;
      saveBooleanSetting(STORAGE_KEYS.hidePrivateCode, hidePrivateCode);
      updatePrivateCodeButton();
      hideCodePopover();
      if (overviewMode === "star") {
        renderStarMapSelectionState({ syncTree: false });
      } else {
        restoreFileView();
      }
    });

    document.getElementById("code-popover-backdrop").addEventListener("click", hideCodePopover);
    document.getElementById("btn-close-code-popover").addEventListener("click", hideCodePopover);
    bindCodePopoverScrollGuard();
    document.addEventListener("pointerdown", handleCodePopoverOutsidePointer);
    document.addEventListener("keydown", evt => {
      if (evt.key === "Escape") hideCodePopover();
    });

    document.getElementById("btn-reset-star-map").addEventListener("click", () => {
      requestStarMapFit();
      renderStarMap();
    });

    const svg = document.getElementById("star-map-svg");
    bindStarMapResizeObserver(svg);
    svg.addEventListener("wheel", evt => {
      evt.preventDefault();
      const delta = evt.deltaY > 0 ? 0.9 : 1.1;
      starTransform.scale = clampStarMapZoom(starTransform.scale * delta);
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

  function bindCodePopoverScrollGuard() {
    const backdrop = document.getElementById("code-popover-backdrop");
    const popover = document.getElementById("code-popover");
    backdrop.addEventListener("wheel", blockWheelBehindPopover, { passive: false });
    popover.addEventListener("wheel", evt => {
      evt.stopPropagation();
      if (!canScrollInsidePopover(evt.target, popover, evt.deltaX, evt.deltaY)) {
        evt.preventDefault();
      }
    }, { passive: false });
  }

  function blockWheelBehindPopover(evt) {
    evt.preventDefault();
    evt.stopPropagation();
  }

  function canScrollInsidePopover(target, boundary, deltaX, deltaY) {
    let element = target instanceof Element ? target : target && target.parentElement;
    while (element && element !== boundary.parentElement) {
      if (element instanceof HTMLElement && canElementScrollByDelta(element, deltaX, deltaY)) {
        return true;
      }
      if (element === boundary) break;
      element = element.parentElement;
    }
    return false;
  }

  function canElementScrollByDelta(element, deltaX, deltaY) {
    const style = window.getComputedStyle(element);
    return canElementScrollAxis(element, style.overflowY, element.scrollTop, element.clientHeight, element.scrollHeight, deltaY)
      || canElementScrollAxis(element, style.overflowX, element.scrollLeft, element.clientWidth, element.scrollWidth, deltaX);
  }

  function canElementScrollAxis(element, overflow, position, viewportSize, contentSize, delta) {
    if (!delta || !isScrollableOverflow(overflow) || contentSize <= viewportSize + 1) return false;
    if (delta < 0) return position > 0;
    return position + viewportSize < contentSize - 1;
  }

  function isScrollableOverflow(overflow) {
    return overflow === "auto" || overflow === "scroll" || overflow === "overlay";
  }

  function bindStarMapResizeObserver(svg) {
    if (!window.ResizeObserver) return;
    const observer = new ResizeObserver(() => preserveStarMapScreenScale());
    observer.observe(svg);
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
