"use strict";
  let manifest = null;
  let language = localStorage.getItem(STORAGE_KEYS.language) || "en";
  let overviewMode = localStorage.getItem(STORAGE_KEYS.overviewMode) || "file";
  let viewWholeFile = false;
  let treeChangedFilesOnly = loadBooleanSetting(STORAGE_KEYS.treeChangedFilesOnly, false);
  let starMapShowAllFiles = loadBooleanSetting(STORAGE_KEYS.starMapShowAllFiles, false);
  let currentFile = null;
  let currentDirectory = null;
  let scopePath = normalizeScopePath(localStorage.getItem(STORAGE_KEYS.scopePath) || SOURCE_ROOT);
  let starMapSelection = { kind: "module", path: scopePath };
  let scopePulsePath = scopePath;
  let scopePulseStartedAt = getAnimationNow();
  let moduleRoot = null;
  let selectedModule = null;
  let collapsedModules = loadSetSetting(STORAGE_KEYS.collapsedModules);
  let starTransform = { x: 0, y: 0, scale: 1 };
  let starViewportScale = 0;
  let starAutoFit = true;
  let panState = null;
  let tooltipTarget = null;
  let designCodeSnippets = new Map();
  let designCodeCounter = 0;
  let githubRepoApi = "";
  let activeDataSource = "";
  let localRootHandle = null;
  let designStatusChanges = null;

  if (!["file", "star"].includes(overviewMode)) {
    overviewMode = "file";
  }

  async function init() {
    try {
      const dataSource = await resolveDataSourceChoice();
      activeDataSource = dataSource.kind;
      localRootHandle = dataSource.handle || null;
      setLoadingVisible(true);
      setLoadingMessage(activeDataSource === DATA_SOURCE_LOCAL ? t("loadingOverview") : t("buildingOverview"));
      logOverview("initializing dynamic overview");
      manifest = activeDataSource === DATA_SOURCE_LOCAL
        ? await buildManifestFromLocal(localRootHandle)
        : await buildManifestFromGitHub();
      logOverview("manifest ready", {
        source: activeDataSource,
        files: Object.keys(manifest.files || {}).length,
        changedFiles: Object.keys(((manifest.diff || {}).changes) || {}).length,
        previousTag: (manifest.diff || {}).prev_tag,
        latestTag: (manifest.diff || {}).latest_tag
      });

      moduleRoot = buildModuleTree(manifest.files || {});
      selectedModule = getScopeModule();

      bindEvents();
      applyLanguage();
      renderTree();

      const cachedFile = resolveCachedFileSelection();
      if (overviewMode !== "star" && cachedFile) {
        openFile(cachedFile);
      } else if (overviewMode === "star") {
        renderMode();
        renderStarMapSelectionState({ syncTree: true });
      }
    } catch (e) {
      logOverviewError("dynamic overview load failed", e);
      if (activeDataSource === DATA_SOURCE_LOCAL) {
        await clearCachedLocalSource();
        setLoadingVisible(false);
        await promptDataSourceChoice(t("dataSourceLocalMissing"));
        window.location.reload();
        return;
      }
      if (activeDataSource === DATA_SOURCE_GITHUB) {
        localStorage.removeItem(STORAGE_KEYS.dataSource);
        setLoadingVisible(false);
        await promptDataSourceChoice(t("overviewLoadFailed"));
        window.location.reload();
        return;
      }
      manifest = { files: {}, diff: { prev_tag: null, latest_tag: null, changes: {} }, generated_at: "" };
      moduleRoot = buildModuleTree(manifest.files);
      selectedModule = getScopeModule();
      bindEvents();
      applyLanguage();
      renderTree();
      showWelcome(t("overviewLoadFailed"));
    } finally {
      setLoadingVisible(false);
    }
  }

  function setLoadingVisible(visible) {
    const overlay = document.getElementById("loading-overlay");
    if (overlay) overlay.classList.toggle("hidden", !visible);
  }

  function setLoadingMessage(message) {
    const el = document.getElementById("loading-message");
    if (el) el.textContent = message;
  }

  function logOverview(message, details) {
    if (details === undefined) {
      console.log(LOG_PREFIX, message);
    } else {
      console.log(LOG_PREFIX, message, details);
    }
  }

  function logOverviewError(message, error) {
    console.error(LOG_PREFIX, message, error);
  }

  function logStarMapState(event, details = {}) {
    if (!STAR_MAP_DEBUG) return;
    const payload = {
      scopePath,
      starMapSelection: { ...starMapSelection },
      currentFile,
      currentDirectory,
      selectedModule: selectedModule && selectedModule.path,
      starAutoFit,
      ...details
    };
    console.log(LOG_PREFIX, `${event} ${JSON.stringify(payload)}`);
  }

  function t(key) {
    return I18N[language][key] || I18N.en[key] || key;
  }

  function loadBooleanSetting(key, fallback) {
    const value = localStorage.getItem(key);
    if (value === null) return fallback;
    return value === "true";
  }

  function saveBooleanSetting(key, value) {
    localStorage.setItem(key, value ? "true" : "false");
  }

  function loadSetSetting(key) {
    try {
      const value = JSON.parse(localStorage.getItem(key) || "[]");
      return new Set(Array.isArray(value) ? value : []);
    } catch (e) {
      return new Set();
    }
  }

  function saveSetSetting(key, value) {
    localStorage.setItem(key, JSON.stringify(Array.from(value)));
  }

  function applyLanguage() {
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
    document.title = `Marix - ${t("title")}`;

    for (const el of document.querySelectorAll("[data-i18n]")) {
      el.textContent = t(el.dataset.i18n);
    }

    const searchInput = document.getElementById("search-input");
    searchInput.placeholder = t("searchPlaceholder");

    if (overviewMode !== "star") {
      document.getElementById("file-path").textContent = currentFile || currentDirectory || t("selectFile");
    }
    updateToolButton("btn-star-map-view", "starMapView", overviewMode === "star");
    updateActionButton("btn-language", "languageTool");
    updateActionButton("btn-collapse-all", "collapseAllTool");
    updateActionButton("btn-reset-star-map", "resetView");
    updateActionButton("btn-reset-data-source", "resetDataSourceTool");
    updateTreeFilterButton();
    updateDataSourceDependentControls();

    renderWelcome();
    renderMode();
    if (overviewMode === "star") {
      renderStarMapSelectionState({ syncTree: false });
    } else if (!currentFile && !currentDirectory) {
      document.getElementById("welcome").style.display = "block";
    }
  }

  function renderWelcome() {
    const welcome = document.getElementById("welcome");
    welcome.innerHTML = `
      <h2>${escapeHtml(t("welcomeTitle"))}</h2>
      <p>${escapeHtml(t("welcomeIntro"))}</p>
      <p>${escapeHtml(t("welcomeModified"))}</p>
      <p>${escapeHtml(t("welcomeAdded"))}</p>
      <p>${escapeHtml(t("welcomeDeleted"))}</p>
    `;
  }

  function renderMode() {
    const isStar = overviewMode === "star";
    document.getElementById("main").style.display = "flex";
    document.getElementById("star-map-workspace").style.display = isStar ? "flex" : "none";
    document.getElementById("viewer-header").style.display = isStar ? "none" : "flex";
    document.getElementById("viewer-content").classList.toggle("star-active", isStar);
    document.getElementById("btn-star-map-view").classList.toggle("active", isStar);

    if (isStar) {
      hideFileViews();
    } else {
      document.getElementById("star-map-workspace").style.display = "none";
      if (!currentFile) {
        hideFileViews();
        renderWelcome();
        document.getElementById("welcome").style.display = "block";
      }

    }
    updateTreeFilterButton();
    updateDataSourceDependentControls();
  }

  function setScopePath(path) {
    const nextScopePath = normalizeScopePath(path);
    if (scopePath !== nextScopePath) {
      scopePulsePath = nextScopePath;
      scopePulseStartedAt = getAnimationNow();
    }
    scopePath = nextScopePath;
    localStorage.setItem(STORAGE_KEYS.scopePath, scopePath);
  }

  function getScopePulseAnimationDelay(modulePath) {
    if (modulePath !== scopePulsePath) {
      scopePulsePath = modulePath || scopePath;
      scopePulseStartedAt = getAnimationNow();
    }
    const elapsedSeconds = ((getAnimationNow() - scopePulseStartedAt) / 1000) % SCOPE_CENTER_PULSE_SECONDS;
    return `-${elapsedSeconds.toFixed(3)}s`;
  }

  function getAnimationNow() {
    return (window.performance && typeof window.performance.now === "function") ? window.performance.now() : Date.now();
  }

  function setOverviewMode(nextMode) {
    overviewMode = nextMode === "star" ? "star" : "file";
    localStorage.setItem(STORAGE_KEYS.overviewMode, overviewMode);
    renderMode();
    if (overviewMode === "star") {
      applyStarMapState(starMapSelection, {
        eventName: "set-overview-mode",
        fit: true,
        forceFit: true,
        syncTree: true
      });
    } else {
      restoreFileView();
    }
  }

  function selectStarMapModule(modulePath, options = {}) {
    return applyStarMapState({
      selectionKind: "module",
      modulePath: modulePath || SOURCE_ROOT
    }, {
      ...options,
      eventName: "select-module",
      fit: options.fit !== false,
      syncTree: options.syncTree !== false
    });
  }

  function focusStarMapFile(filePath, options = {}) {
    if (!filePath) return null;
    return applyStarMapState({
      selectionKind: "file",
      filePath,
      scopePath: options.setScopeToParent ? getParentPath(filePath) : options.scopePath
    }, {
      ...options,
      eventName: "focus-file",
      fit: options.fit === true,
      syncTree: options.syncTree !== false
    });
  }

  function applyStarMapState(intent = {}, options = {}) {
    const eventName = options.eventName || "apply-star-state";
    logStarMapState(`${eventName}:start`, { intent, options });

    const previousScopePath = scopePath;
    const selectionKind = intent.selectionKind || intent.kind || (intent.filePath ? "file" : "module");
    if (selectionKind === "file") {
      const filePath = intent.filePath || intent.path || intent.selectionPath;
      if (!filePath) return null;
      const nextScopePath = intent.scopePath ? normalizeScopePath(intent.scopePath) : scopePath;
      setScopePath(nextScopePath || getParentPath(filePath));
      currentFile = filePath;
      currentDirectory = getParentPath(filePath);
      localStorage.setItem(STORAGE_KEYS.currentFile, filePath);
      selectedModule = getScopeModule();
      starMapSelection = { kind: "file", path: filePath };
    } else {
      const modulePath = intent.modulePath || intent.scopePath || intent.path || intent.selectionPath || scopePath || SOURCE_ROOT;
      setScopePath(modulePath);
      currentFile = null;
      currentDirectory = scopePath;
      localStorage.removeItem(STORAGE_KEYS.currentFile);
      selectedModule = getScopeModule();
      starMapSelection = { kind: "module", path: selectedModule.path || SOURCE_ROOT };
    }

    if (options.fit && (options.forceFit || previousScopePath !== scopePath)) requestStarMapFit();
    if (options.render !== false) renderStarMapSelectionState(options);
    logStarMapState(`${eventName}:done`);
    return { scopePath, selection: { ...starMapSelection }, selectedModule };
  }

  function renderStarMapSelectionState(options = {}) {
    if (overviewMode !== "star") return;
    if (!selectedModule) selectedModule = getScopeModule();
    if (options.syncTree !== false) {
      if (starMapSelection.kind === "file") {
        markTreeSelection(starMapSelection.path);
      } else {
        syncTreeToModule(starMapSelection.path || selectedModule.path || SOURCE_ROOT);
      }
    }
    renderModuleDetails(selectedModule);
    renderStarMap();
    if (options.openPopover && starMapSelection.kind === "file") {
      showFilePopover(starMapSelection.path);
    }
  }

  function resolveCachedFileSelection() {
    const cachedFile = localStorage.getItem(STORAGE_KEYS.currentFile);
    if (!cachedFile) return "";
    if (shouldIncludeVisibleSourcePath(cachedFile) && (manifest.files || {})[cachedFile]) {
      applyStarMapState({
        selectionKind: "file",
        filePath: cachedFile
      }, {
        eventName: "restore-cached-file",
        render: false,
        syncTree: false
      });
      return cachedFile;
    }

    applyStarMapState({
      selectionKind: "module",
      modulePath: SOURCE_ROOT
    }, {
      eventName: "reset-stale-cached-file",
      render: false,
      syncTree: false
    });
    return "";
  }

  function markTreeSelection(path) {
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    const item = findTreeItem(path);
    if (item) item.classList.add("active");
  }

  function normalizeScopePath(path) {
    return path && isSourcePath(path) ? path : SOURCE_ROOT;
  }

  function isSourcePath(path) {
    return isSourcePathName(path);
  }

  function isSourcePathName(path) {
    return path === SOURCE_ROOT || path.startsWith(`${SOURCE_ROOT}/`);
  }

  function getChange(path) {
    return ((manifest.diff || {}).changes || {})[path]
      || getDesignStatusChanges().files[path]
      || getDirectAddedFolderDesignChange(path)
      || null;
  }

  function normalizeChangeStatus(status) {
    const key = String(status || "").trim();
    const values = {
      A: "added",
      M: "modified",
      R: "renamed",
      D: "deleted",
      added: "added",
      modified: "modified",
      renamed: "renamed",
      deleted: "deleted",
      removed: "deleted",
      changed: "modified",
      unchanged: "unchanged"
    };
    return values[key] || values[key.toLowerCase()] || "unchanged";
  }

  function getPathChangeStatus(path) {
    const change = getChange(path);
    return change ? normalizeChangeStatus(change.status) : "unchanged";
  }

  function isPathChanged(path) {
    return getPathChangeStatus(path) !== "unchanged";
  }

  function getChangedVisiblePaths() {
    const paths = new Set([
      ...Object.keys(((manifest.diff || {}).changes || {})),
      ...Object.keys(getDesignStatusChanges().files),
      ...getDesignChangedFolderFiles()
    ]);
    return Array.from(paths)
      .filter(path => shouldIncludeVisibleSourcePath(path))
      .filter(path => {
        const status = getPathChangeStatus(path);
        return status !== "unchanged" && status !== "deleted";
      });
  }

  function getDesignStatusChanges() {
    if (designStatusChanges) return designStatusChanges;

    const changes = { files: {}, folders: {} };
    for (const [designPath, data] of Object.entries(manifest.files || {})) {
      if (!designPath.endsWith("/.design.json") || !data || !data.content) continue;

      let document = null;
      try {
        document = JSON.parse(String(data.content || "").trim());
      } catch (error) {
        logOverviewError(`design status load failed: ${designPath}`, error);
        continue;
      }

      const modulePath = (document.module && document.module.path) || designPath.replace(/\/\.design\.json$/i, "");
      collectDesignStatusEntries(changes, modulePath, document, "added");
      collectDesignStatusEntries(changes, modulePath, document, "modified");
      collectDesignStatusEntries(changes, modulePath, document, "deleted");
      collectDesignStatusEntries(changes, modulePath, document, "renamed");
    }

    designStatusChanges = changes;
    return changes;
  }

  function collectDesignStatusEntries(changes, modulePath, document, status) {
    const entries = Array.isArray(document[status]) ? document[status] : [];
    for (const entry of entries) {
      const name = String(entry || "").trim();
      if (!name) continue;
      if (name === ".") {
        changes.folders[modulePath] = createSyntheticDesignChange(status);
      } else if (!name.includes("/") && !name.includes("\\")) {
        changes.files[`${modulePath}/${name}`] = createSyntheticDesignChange(status);
      }
    }
  }

  function getDirectAddedFolderDesignChange(path) {
    const parentPath = getPathParent(path);
    const change = getDesignStatusChanges().folders[parentPath] || null;
    return normalizeChangeStatus(change && change.status) === "added" ? change : null;
  }

  function getDesignChangedFolderFiles() {
    const files = [];
    for (const [folderPath, change] of Object.entries(getDesignStatusChanges().folders)) {
      const status = normalizeChangeStatus(change.status);
      if (status !== "added") continue;
      for (const path of Object.keys(manifest.files || {})) {
        if (shouldIncludeVisibleSourcePath(path) && getPathParent(path) === folderPath) {
          files.push(path);
        }
      }
    }
    return files;
  }

  function getFolderChangeStatus(path) {
    const change = getDesignStatusChanges().folders[path] || null;
    return normalizeChangeStatus(change && change.status);
  }

  function getPathParent(path) {
    const index = String(path || "").lastIndexOf("/");
    return index >= 0 ? path.slice(0, index) : "";
  }

  function createSyntheticDesignChange(status) {
    return {
      status,
      diff_lines: [],
      hunks: []
    };
  }

  function updateTreeFilterButton() {
    updateToolButton(
      "btn-view-whole-file",
      treeChangedFilesOnly ? "treeAllFilesTool" : "treeChangedFilesOnlyTool",
      treeChangedFilesOnly
    );
  }

  function updateToolButton(id, labelKey, active) {
    const el = document.getElementById(id);
    el.dataset.tooltip = t(labelKey);
    el.setAttribute("aria-label", t(labelKey));
    el.classList.toggle("active", active);
    el.setAttribute("aria-pressed", active ? "true" : "false");
  }

  function updateActionButton(id, labelKey) {
    const el = document.getElementById(id);
    el.dataset.tooltip = t(labelKey);
    el.setAttribute("aria-label", t(labelKey));
    el.classList.remove("active");
    el.removeAttribute("aria-pressed");
  }

  function updateDataSourceDependentControls() {
    setElementVisible("btn-view-whole-file", true);
  }

  function setElementVisible(id, visible) {
    const el = document.getElementById(id);
    if (el) el.style.display = visible ? "" : "none";
  }

  async function resetDataSourceChoice() {
    await clearCachedLocalSource();
    localStorage.removeItem(STORAGE_KEYS.currentFile);
    localStorage.removeItem(STORAGE_KEYS.scopePath);
    window.location.reload();
  }

  function showTooltip(target) {
    const text = target.dataset.tooltip;
    if (!text) return;

    const tooltip = document.getElementById("global-tooltip");
    tooltipTarget = target;
    tooltip.textContent = text;
    tooltip.style.display = "block";
    tooltip.style.opacity = "0";

    requestAnimationFrame(() => {
      const targetRect = target.getBoundingClientRect();
      const tooltipRect = tooltip.getBoundingClientRect();
      const left = Math.max(8, Math.min(window.innerWidth - tooltipRect.width - 8, targetRect.left + targetRect.width / 2 - tooltipRect.width / 2));
      const top = Math.max(8, targetRect.top - tooltipRect.height - 8);
      tooltip.style.left = `${left}px`;
      tooltip.style.top = `${top}px`;
      tooltip.style.opacity = "1";
    });
  }

  function hideTooltip(target) {
    if (target && tooltipTarget !== target) return;
    const tooltip = document.getElementById("global-tooltip");
    tooltipTarget = null;
    tooltip.style.opacity = "0";
    tooltip.style.display = "none";
  }
