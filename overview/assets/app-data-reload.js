"use strict";
  let overviewDataReloadInProgress = false;

  async function reloadOverviewData(reason = "manual", options = {}) {
    if (overviewDataReloadInProgress) return false;
    overviewDataReloadInProgress = true;

    const snapshot = captureOverviewReloadState();
    const previousState = captureOverviewDataState();
    try {
      if (typeof hideActiveReloadBanner === "function") hideActiveReloadBanner();
      if (typeof hideCodePopover === "function") hideCodePopover();
      setLoadingVisible(true);

      const dataSource = options.dataSource || await resolveOverviewReloadSource();
      setLoadingVisible(true);
      setLoadingMessage(dataSource.kind === DATA_SOURCE_LOCAL ? t("loadingOverview") : t("buildingOverview"));
      await loadOverviewDataFromSource(dataSource);
      reconcileOverviewSelectionAfterReload(snapshot, options);
      if (options.bindEventsAfterLoad) bindEvents();
      await renderOverviewAfterDataReload(snapshot);

      logOverview("overview data reloaded", {
        reason,
        source: activeDataSource,
        files: Object.keys(manifest.files || {}).length,
        changedFiles: Object.keys(((manifest.diff || {}).changes) || {}).length,
        ref: getRequestedRef() || null
      });
      return true;
    } catch (error) {
      logOverviewError("overview data reload failed", error);
      restoreOverviewDataState(previousState);
      if (manifest) {
        await renderOverviewAfterDataReload(snapshot);
      } else {
        initializeEmptyOverview(t("overviewLoadFailed"));
        if (options.bindEventsAfterLoad) bindEvents();
      }
      return false;
    } finally {
      setLoadingVisible(false);
      overviewDataReloadInProgress = false;
    }
  }

  async function loadOverviewDataFromSource(dataSource) {
    const kind = normalizeDataSourceKind(dataSource && dataSource.kind);
    if (!kind) throw new Error("overview data source is not selected");

    activeDataSource = kind;
    if (kind === DATA_SOURCE_LOCAL) {
      localRootHandle = dataSource.handle || localRootHandle || await resolveCachedLocalHandle();
      if (!localRootHandle) throw new Error("saved local source handle is unavailable");
      manifest = await buildManifestFromLocal(localRootHandle);
    } else {
      localRootHandle = null;
      manifest = await buildManifestFromGitHub();
    }

    designStatusChanges = null;
    designCodeSnippets = new Map();
    designCodeCounter = 0;
    moduleRoot = buildModuleTree(manifest.files || {});
    selectedModule = getScopeModule();
  }

  async function resolveOverviewReloadSource() {
    const kind = normalizeDataSourceKind(activeDataSource) || getCachedDataSourceKind();
    if (kind === DATA_SOURCE_LOCAL) {
      const handle = await resolveOverviewLocalReloadHandle();
      if (!handle) {
        await clearCachedDataSource();
        return await promptDataSourceChoice(t("dataSourceLocalMissing"));
      }
      return { kind, handle };
    }
    if (kind === DATA_SOURCE_GITHUB) return { kind };
    return await resolveDataSourceChoice();
  }

  async function resolveOverviewLocalReloadHandle() {
    if (localRootHandle) {
      try {
        const allowed = await requestLocalReadPermission(localRootHandle);
        if (allowed) {
          await assertLocalRootReadable(localRootHandle);
          return localRootHandle;
        }
      } catch (error) {
        logOverviewError("local source reload handle validation failed", error);
      }
      await deleteLocalRootHandle();
      localRootHandle = null;
    }
    return await resolveCachedLocalHandle();
  }

  function captureOverviewReloadState() {
    const searchInput = document.getElementById("search-input");
    return {
      overviewMode,
      currentFile,
      currentDirectory,
      scopePath,
      starMapSelection: { ...starMapSelection },
      search: searchInput ? searchInput.value.trim() : ""
    };
  }

  function captureOverviewDataState() {
    return {
      manifest,
      moduleRoot,
      selectedModule,
      activeDataSource,
      localRootHandle,
      designStatusChanges,
      githubRepoApi,
      scopePath,
      currentFile,
      currentDirectory,
      starMapSelection: { ...starMapSelection },
      storedCurrentFile: localStorage.getItem(STORAGE_KEYS.currentFile),
      storedScopePath: localStorage.getItem(STORAGE_KEYS.scopePath)
    };
  }

  function restoreOverviewDataState(state) {
    manifest = state.manifest;
    moduleRoot = state.moduleRoot;
    selectedModule = state.selectedModule;
    activeDataSource = state.activeDataSource;
    localRootHandle = state.localRootHandle;
    designStatusChanges = state.designStatusChanges;
    githubRepoApi = state.githubRepoApi;
    scopePath = state.scopePath;
    currentFile = state.currentFile;
    currentDirectory = state.currentDirectory;
    starMapSelection = { ...state.starMapSelection };
    restoreStoredReloadValue(STORAGE_KEYS.currentFile, state.storedCurrentFile);
    restoreStoredReloadValue(STORAGE_KEYS.scopePath, state.storedScopePath);
  }

  function restoreStoredReloadValue(key, value) {
    if (value == null) {
      localStorage.removeItem(key);
    } else {
      localStorage.setItem(key, value);
    }
  }

  function reconcileOverviewSelectionAfterReload(snapshot, options = {}) {
    overviewMode = snapshot.overviewMode === "star" ? "star" : "file";

    if (options.resetSelection) {
      resetOverviewReloadSelection(overviewMode === "star");
      return;
    }

    if (overviewMode === "star") {
      reconcileStarMapSelectionAfterReload(snapshot);
      return;
    }

    reconcileFileViewSelectionAfterReload(snapshot);
  }

  function reconcileStarMapSelectionAfterReload(snapshot) {
    const selection = snapshot.starMapSelection || {};
    if (selection.kind === "file") {
      if (isOverviewReloadFileSelectable(selection.path)) {
        applyStarMapState({
          selectionKind: "file",
          filePath: selection.path,
          scopePath: getValidReloadScopePath(snapshot.scopePath, getParentPath(selection.path))
        }, { eventName: "reload-star-file", render: false, syncTree: false });
        return;
      }
      resetOverviewReloadSelection(true);
      return;
    }

    applyStarMapState({
      selectionKind: "module",
      modulePath: getValidReloadScopePath(selection.path || snapshot.scopePath)
    }, { eventName: "reload-star-module", render: false, syncTree: false });
  }

  function reconcileFileViewSelectionAfterReload(snapshot) {
    if (snapshot.currentFile) {
      if (isOverviewReloadFileSelectable(snapshot.currentFile)) {
        applyStarMapState({
          selectionKind: "file",
          filePath: snapshot.currentFile,
          scopePath: getParentPath(snapshot.currentFile)
        }, { eventName: "reload-file-view-file", render: false, syncTree: false });
        return;
      }
      resetOverviewReloadSelection(false);
      return;
    }

    if (snapshot.currentDirectory && isOverviewReloadModuleSelectable(snapshot.currentDirectory)) {
      applyStarMapState({
        selectionKind: "module",
        modulePath: snapshot.currentDirectory
      }, { eventName: "reload-file-view-directory", render: false, syncTree: false });
      return;
    }

    resetOverviewReloadSelection(false);
  }

  function resetOverviewReloadSelection(keepRootDirectory) {
    currentFile = null;
    currentDirectory = keepRootDirectory ? SOURCE_ROOT : null;
    localStorage.removeItem(STORAGE_KEYS.currentFile);
    setScopePath(SOURCE_ROOT);
    selectedModule = getScopeModule();
    starMapSelection = { kind: "module", path: selectedModule.path || SOURCE_ROOT };
  }

  function getValidReloadScopePath(...candidates) {
    for (const candidate of candidates) {
      const path = normalizeScopePath(candidate || SOURCE_ROOT);
      if (isOverviewReloadModuleSelectable(path)) return path;
    }
    return SOURCE_ROOT;
  }

  function isOverviewReloadFileSelectable(path) {
    return shouldIncludeVisibleSourcePath(path) && Boolean((manifest.files || {})[path] || getChange(path));
  }

  function isOverviewReloadModuleSelectable(path) {
    return Boolean(path && findModule(moduleRoot, path));
  }

  async function renderOverviewAfterDataReload(snapshot) {
    const searchInput = document.getElementById("search-input");
    const filter = searchInput ? searchInput.value.trim() : snapshot.search;
    renderTree(filter);
    renderMode();

    if (overviewMode === "star") {
      requestStarMapFit();
      renderStarMapSelectionState({ syncTree: true });
      return;
    }

    if (currentFile) {
      await openFile(currentFile, findTreeItem(currentFile));
      return;
    }

    if (currentDirectory) {
      openDirectoryChanges(currentDirectory, findTreeItem(currentDirectory));
      return;
    }

    hideAllViews();
    renderWelcome();
    document.getElementById("welcome").style.display = "block";
    document.getElementById("file-path").textContent = t("selectFile");
    document.getElementById("file-status").innerHTML = "";
  }

  function initializeEmptyOverview(message) {
    manifest = { files: {}, diff: { prev_tag: null, latest_tag: null, changes: {} }, generated_at: "" };
    designStatusChanges = null;
    moduleRoot = buildModuleTree(manifest.files);
    resetOverviewReloadSelection(false);
    applyLanguage();
    renderTree();
    showWelcome(message);
  }
