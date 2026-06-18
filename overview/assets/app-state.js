"use strict";
  const SVG_NS = "http://www.w3.org/2000/svg";
  const SOURCE_ROOT = "src";

  const I18N = {
    en: {
      title: "Overview",
      starMapView: "Star Map",
      languageTool: "Switch language",
      language: "中文",
      searchPlaceholder: "Search files...",
      collapseAllTool: "Collapse all",
      viewAllFilesTool: "View all files in tree",
      viewWholeFileTool: "View whole changed files",
      resetDataSourceTool: "Choose data source again",
      selectFile: "Select a file to view",
      scopeLabel: "Scope",
      welcomeTitle: "Welcome to Marix Overview",
      welcomeIntro: "Browse repository files from the sidebar.",
      welcomeModified: "Files marked with M have been modified since the last tag.",
      welcomeAdded: "Files marked with A are newly added.",
      welcomeDeleted: "Files marked with D have been deleted.",
      fileUnavailable: "File content not available.",
      statusModified: "Modified",
      statusAdded: "Added",
      statusDeleted: "Deleted",
      statusRenamed: "Renamed",
      diffPanelTitle: "Changed sections",
      diffPanelSubtitle: "Showing only modified parts of this file.",
      fullFileTitle: "Full file with change markers",
      fullFileSubtitle: "Showing the whole file. Added lines are highlighted, existing lines stay visible, and deleted lines appear near their original position.",
      noChangedSections: "No changed sections are available for this file.",
      loadingOverview: "Loading overview...",
      buildingOverview: "Building overview from GitHub tags...",
      overviewLoadFailed: "Unable to load repository data from GitHub.",
      dataSourceTitle: "Choose data source",
      dataSourceIntro: "Load repository data from GitHub or select a local repository folder. This choice is cached for future visits.",
      dataSourceGithub: "Use GitHub",
      dataSourceLocal: "Choose local folder",
      dataSourceLocalUnsupported: "This browser cannot remember local folders. Use Microsoft Edge or Chrome, or choose GitHub.",
      dataSourceLocalMissing: "The saved local folder is unavailable. Choose a data source again.",
      folderChangesTitle: "Folder changes",
      folderChangesSubtitle: "Showing changes for this folder and all nested files.",
      addedLine: "Added",
      existingLine: "Existing",
      deletedLine: "Deleted",
      reason: "Reason",
      reasonPending: "Pending overview-agent annotation.",
      starTitle: "Star Map",
      starDescription: "Browse repository modules as a nested star map. Modules are derived from folder hierarchy, and changed modules are highlighted from marix tag diffs.",
      starHelp: "Wheel to zoom. Drag the canvas to pan. Select a module to inspect it. Use the details panel to expand or collapse submodules.",
      resetView: "Reset view",
      rootModule: "Repository root",
      changed: "Changed",
      unchanged: "Unchanged",
      modulePath: "Module path",
      childModules: "Child modules",
      files: "Files",
      publicInterfaces: "Public interfaces",
      changedFileList: "Changed files",
      interfaces: "Interfaces",
      dataStorage: "Data storage",
      implementations: "Implementation files",
      noItems: "None",
      expand: "Expand",
      collapse: "Collapse",
      moduleDetailsHint: "Select a module in the map to see interfaces, storage, and implementation files.",
      directoryModule: "Directory module",
      rustModuleHint: "Rust module candidates are inferred from folder layers and Rust files such as lib.rs, mod.rs, main.rs, and *.rs.",
      designDetails: "Design details",
      noDesignDetails: "No design details are available for this module."
    },
    zh: {
      title: "总览",
      starMapView: "星图视图",
      languageTool: "切换语言",
      language: "EN",
      searchPlaceholder: "搜索文件...",
      collapseAllTool: "折叠全部",
      viewAllFilesTool: "左侧显示全部文件",
      viewWholeFileTool: "查看改动文件全文",
      resetDataSourceTool: "重新选择数据源",
      selectFile: "选择一个文件查看",
      scopeLabel: "范围",
      welcomeTitle: "欢迎来到 Marix 总览",
      welcomeIntro: "从左侧浏览仓库文件。",
      welcomeModified: "标记为 M 的文件表示从上一个 tag 后被修改。",
      welcomeAdded: "标记为 A 的文件表示新增加。",
      welcomeDeleted: "标记为 D 的文件表示已删除。",
      fileUnavailable: "文件内容不可用。",
      statusModified: "已修改",
      statusAdded: "已新增",
      statusDeleted: "已删除",
      statusRenamed: "已重命名",
      diffPanelTitle: "改动片段",
      diffPanelSubtitle: "当前只展示这个文件中发生变化的部分。",
      fullFileTitle: "带改动标注的完整文件",
      fullFileSubtitle: "当前展示完整文件。新增行会高亮，已有行会保留可见，被删除的行会显示在原位置附近。",
      noChangedSections: "这个文件没有可展示的改动片段。",
      loadingOverview: "正在加载总览...",
      buildingOverview: "正在根据 GitHub tag 构建总览...",
      overviewLoadFailed: "无法从 GitHub 加载仓库数据。",
      dataSourceTitle: "选择数据源",
      dataSourceIntro: "从 GitHub 加载仓库数据，或选择本地仓库文件夹。这个选择会被缓存供后续访问使用。",
      dataSourceGithub: "使用 GitHub 线上版本",
      dataSourceLocal: "选择本地文件夹",
      dataSourceLocalUnsupported: "当前浏览器不能记住本地文件夹。请使用 Microsoft Edge 或 Chrome，或选择 GitHub。",
      dataSourceLocalMissing: "缓存的本地文件夹不可用，请重新选择数据源。",
      folderChangesTitle: "文件夹改动",
      folderChangesSubtitle: "当前展示这个文件夹及其所有子文件中的改动。",
      addedLine: "新增",
      existingLine: "已有",
      deletedLine: "已删除",
      reason: "原因",
      reasonPending: "等待 overview-agent 补充说明。",
      starTitle: "星图视图",
      starDescription: "以嵌套星图浏览仓库模块。模块根据文件夹层级生成，并根据 marix tag diff 高亮改动模块。",
      starHelp: "鼠标滚轮缩放。拖动画布平移。选择模块查看详情。可在右侧面板展开或折叠子模块。",
      resetView: "重置视图",
      rootModule: "仓库根模块",
      changed: "有改动",
      unchanged: "无改动",
      modulePath: "模块路径",
      childModules: "子模块",
      files: "文件",
      publicInterfaces: "公共接口",
      changedFileList: "改动文件",
      interfaces: "接口",
      dataStorage: "数据存储",
      implementations: "实现文件",
      noItems: "无",
      expand: "展开",
      collapse: "折叠",
      moduleDetailsHint: "在星图中选择模块，即可查看接口、数据存储和实现文件。",
      directoryModule: "目录模块",
      rustModuleHint: "Rust 模块候选会根据文件夹层级和 lib.rs、mod.rs、main.rs、*.rs 等 Rust 文件推断。",
      designDetails: "设计详情",
      noDesignDetails: "这个模块没有可用的设计详情。"
    }
  };

  const STORAGE_KEYS = {
    language: "marix-overview-language",
    overviewMode: "marix-overview-mode",
    viewAllFiles: "marix-overview-view-all-files",
    viewWholeFile: "marix-overview-view-whole-file",
    currentFile: "marix-overview-current-file",
    scopePath: "marix-overview-scope-path",
    collapsedModules: "marix-overview-collapsed-modules",
    dataSource: "marix-overview-data-source"
  };

  const GITHUB_OWNER = "DexterDreeeam";
  const GITHUB_REPO = "Marix";
  const MAX_DYNAMIC_FILE_SIZE = 100 * 1024;
  const LOG_PREFIX = "[Marix Overview]";
  const DATA_SOURCE_GITHUB = "github";
  const DATA_SOURCE_LOCAL = "local";
  const LOCAL_DB_NAME = "marix-overview-local-source";
  const LOCAL_DB_STORE = "handles";
  const LOCAL_ROOT_HANDLE_KEY = "root";
  const IMAGE_MIME_TYPES = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    svg: "image/svg+xml",
    webp: "image/webp",
    ico: "image/x-icon"
  };

  let manifest = null;
  let language = localStorage.getItem(STORAGE_KEYS.language) || "en";
  let overviewMode = localStorage.getItem(STORAGE_KEYS.overviewMode) || "file";
  let viewAllFiles = loadBooleanSetting(STORAGE_KEYS.viewAllFiles, false);
  let viewWholeFile = loadBooleanSetting(STORAGE_KEYS.viewWholeFile, false);
  let currentFile = null;
  let currentDirectory = null;
  let scopePath = normalizeScopePath(localStorage.getItem(STORAGE_KEYS.scopePath) || SOURCE_ROOT);
  let moduleRoot = null;
  let selectedModule = null;
  let collapsedModules = loadSetSetting(STORAGE_KEYS.collapsedModules);
  let starTransform = { x: 0, y: 0, scale: 1 };
  let starAutoFit = true;
  let panState = null;
  let tooltipTarget = null;
  let designCodeSnippets = new Map();
  let designCodeCounter = 0;
  let githubRepoApi = "";
  let activeDataSource = "";
  let localRootHandle = null;

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

      const cachedFile = localStorage.getItem(STORAGE_KEYS.currentFile);
      if (overviewMode !== "star" && cachedFile && (manifest.files || {})[cachedFile]) {
        openFile(cachedFile);
      } else if (overviewMode === "star") {
        renderMode();
        renderModuleDetails(selectedModule);
        renderStarMap();
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

  async function resolveDataSourceChoice() {
    const cached = localStorage.getItem(STORAGE_KEYS.dataSource);
    if (cached === DATA_SOURCE_GITHUB) {
      return { kind: DATA_SOURCE_GITHUB };
    }
    if (cached === DATA_SOURCE_LOCAL) {
      try {
        const handle = await loadLocalRootHandle();
        if (!handle) throw new Error("local handle missing");
        const allowed = await requestLocalReadPermission(handle);
        if (!allowed) throw new Error("local handle permission denied");
        await assertLocalRootReadable(handle);
        return { kind: DATA_SOURCE_LOCAL, handle };
      } catch (e) {
        logOverviewError("cached local source unavailable", e);
        await clearCachedLocalSource();
        return promptDataSourceChoice(t("dataSourceLocalMissing"));
      }
    }
    return promptDataSourceChoice();
  }

  function promptDataSourceChoice(message) {
    setLoadingVisible(false);
    const dialog = document.getElementById("data-source-dialog");
    const title = document.getElementById("data-source-title");
    const intro = document.getElementById("data-source-intro");
    const githubButton = document.getElementById("btn-data-source-github");
    const localButton = document.getElementById("btn-data-source-local");
    const error = document.getElementById("data-source-error");

    title.textContent = t("dataSourceTitle");
    intro.textContent = t("dataSourceIntro");
    githubButton.textContent = t("dataSourceGithub");
    localButton.textContent = t("dataSourceLocal");
    error.textContent = message || "";
    dialog.classList.remove("hidden");

    return new Promise(resolve => {
      githubButton.onclick = () => {
        localStorage.setItem(STORAGE_KEYS.dataSource, DATA_SOURCE_GITHUB);
        dialog.classList.add("hidden");
        resolve({ kind: DATA_SOURCE_GITHUB });
      };
      localButton.onclick = async () => {
        if (!window.showDirectoryPicker || !window.indexedDB) {
          error.textContent = t("dataSourceLocalUnsupported");
          return;
        }
        try {
          const handle = await window.showDirectoryPicker({
            id: "marix-overview-local-root",
            mode: "read"
          });
          const allowed = await requestLocalReadPermission(handle);
          if (!allowed) throw new Error("local handle permission denied");
          await assertLocalRootReadable(handle);
          await storeLocalRootHandle(handle);
          localStorage.setItem(STORAGE_KEYS.dataSource, DATA_SOURCE_LOCAL);
          dialog.classList.add("hidden");
          resolve({ kind: DATA_SOURCE_LOCAL, handle });
        } catch (e) {
          logOverviewError("local source selection failed", e);
          error.textContent = t("dataSourceLocalMissing");
        }
      };
    });
  }

  async function buildManifestFromGitHub() {
    const repoApi = `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}`;
    githubRepoApi = repoApi;
    const repo = await fetchJson(repoApi);
    const ref = getRequestedRef() || repo.default_branch || "main";
    logOverview("repository metadata loaded", { defaultBranch: ref });

    const tree = await fetchRepositoryTree(repoApi, ref);
    logOverview("repository tree loaded", { includedFiles: tree.length });

    let diff = { prev_tag: null, latest_tag: null, changes: {} };
    try {
      diff = await fetchTagDiff(repoApi, ref);
      logOverview("tag diff loaded", {
        previousTag: diff.prev_tag,
        latestTag: diff.latest_tag,
        changedFiles: Object.keys(diff.changes || {}).length
      });
    } catch (e) {
      logOverviewError("tag diff load failed; continuing with file tree", e);
    }

    const files = await fetchManifestFiles(repoApi, tree);
    return {
      generated_at: new Date().toISOString(),
      files,
      diff
    };
  }

  async function buildManifestFromLocal(rootHandle) {
    const tree = await fetchLocalRepositoryTree(rootHandle);
    if (tree.length === 0) throw new Error("local source has no readable files");
    logOverview("local repository tree loaded", {
      includedFiles: tree.length,
      visibleSourceFiles: tree.filter(item => isSourcePathName(item.path) && !isHiddenPathName(item.path)).length
    });

    let diff = { prev_tag: null, latest_tag: null, changes: {} };
    try {
      diff = await fetchTagDiff(`https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}`, getRequestedRef() || "main");
      logOverview("tag diff loaded for local source", {
        previousTag: diff.prev_tag,
        latestTag: diff.latest_tag,
        changedFiles: Object.keys(diff.changes || {}).length
      });
    } catch (e) {
      logOverviewError("tag diff load failed for local source; continuing without diff", e);
    }

    return {
      generated_at: new Date().toISOString(),
      files: await fetchManifestFilesFromLocal(tree),
      diff
    };
  }

  async function fetchLocalRepositoryTree(rootHandle) {
    const files = [];
    await collectLocalFiles(rootHandle, "", files);
    return files;
  }

  async function collectLocalFiles(directoryHandle, prefix, files) {
    for await (const [name, handle] of directoryHandle.entries()) {
      const path = prefix ? `${prefix}/${name}` : name;
      if (path.split("/").some(part => isExcludedPathPart(part))) continue;
      if (handle.kind === "directory") {
        await collectLocalFiles(handle, path, files);
      } else if (handle.kind === "file" && shouldIncludeManifestPath(path)) {
        const file = await handle.getFile();
        files.push({
          path,
          size: file.size,
          localHandle: handle
        });
      }
    }
  }

  async function fetchManifestFilesFromLocal(tree) {
    const files = {};
    for (const item of tree) {
      const entry = {
        size: item.size || 0,
        localHandle: item.localHandle
      };
      if (isDesignDocumentPathName(item.path)) {
        if ((item.size || 0) > MAX_DYNAMIC_FILE_SIZE) {
          entry.content = `[File too large: ${item.size} bytes]`;
        } else {
          try {
            entry.content = await readLocalFileText(item.localHandle);
          } catch (e) {
            logOverviewError(`local design content load failed: ${item.path}`, e);
            entry.content = "[Unable to read file]";
          }
        }
      }
      files[item.path] = entry;
    }
    logOverview("local file metadata loaded", {
      files: Object.keys(files).length,
      preloadedDesignFiles: Object.keys(files).filter(path => isDesignDocumentPathName(path) && files[path].content).length
    });
    return files;
  }

  async function fetchRepositoryTree(repoApi, ref) {
    const treeSha = await fetchBranchTreeSha(repoApi, ref);
    logOverview("branch tree resolved", { ref, treeSha });
    const tree = await fetchJson(`${repoApi}/git/trees/${encodeURIComponent(treeSha)}?recursive=1`);
    const files = (tree.tree || [])
      .filter(item => item.type === "blob")
      .filter(item => shouldIncludeManifestPath(item.path));
    const visibleSourceFiles = files.filter(item => isSourcePathName(item.path) && !isHiddenPathName(item.path)).length;
    logOverview("repository tree filtered", {
      totalBlobs: (tree.tree || []).filter(item => item.type === "blob").length,
      includedFiles: files.length,
      visibleSourceFiles,
      scopeRoot: SOURCE_ROOT
    });
    if (visibleSourceFiles === 0) {
      logOverview("source root missing; no files will be shown", { sourceRoot: SOURCE_ROOT });
    }
    return files;
  }

  async function fetchBranchTreeSha(repoApi, ref) {
    const branch = await fetchJson(`${repoApi}/branches/${encodeURIComponent(ref)}`);
    const directTreeSha = branch && branch.commit && branch.commit.commit && branch.commit.commit.tree && branch.commit.commit.tree.sha;
    if (directTreeSha) return directTreeSha;

    const commitSha = branch && branch.commit && branch.commit.sha;
    if (!commitSha) throw new Error(`branch commit unavailable: ${ref}`);
    const commit = await fetchJson(`${repoApi}/git/commits/${encodeURIComponent(commitSha)}`);
    const treeSha = commit && commit.tree && commit.tree.sha;
    if (!treeSha) throw new Error(`branch tree unavailable: ${ref}`);
    return treeSha;
  }

  async function fetchManifestFiles(repoApi, tree) {
    const files = {};
    for (const item of tree) {
      const entry = { size: item.size || 0 };
      if (item.sha) entry.sha = item.sha;
      if (isDesignDocumentPathName(item.path)) {
        if ((item.size || 0) > MAX_DYNAMIC_FILE_SIZE) {
          entry.content = `[File too large: ${item.size} bytes]`;
        } else {
          try {
            entry.content = await fetchBlobText(repoApi, item);
          } catch (e) {
            logOverviewError(`design content load failed: ${item.path}`, e);
            entry.content = "[Unable to read file]";
          }
        }
      }
      files[item.path] = entry;
    }
    logOverview("file metadata loaded", {
      files: Object.keys(files).length,
      preloadedDesignFiles: Object.keys(files).filter(path => isDesignDocumentPathName(path) && files[path].content).length
    });
    return files;
  }

  async function fetchTagDiff(repoApi, ref) {
    const tags = await fetchMarixTags(repoApi);
    const diff = { prev_tag: null, latest_tag: null, changes: {} };
    if (tags.length === 0) return diff;

    const base = tags.length >= 2 ? tags[tags.length - 2].name : tags[0].name;
    const head = tags.length >= 2 ? tags[tags.length - 1].name : ref;
    diff.prev_tag = tags.length >= 2 ? base : null;
    diff.latest_tag = tags.length >= 2 ? head : base;

    const compare = await fetchJson(`${repoApi}/compare/${encodeURIComponent(base)}...${encodeURIComponent(head)}`);
    for (const file of compare.files || []) {
      if (!shouldIncludeManifestPath(file.filename)) continue;
      const parsed = parsePatch(file.patch || "");
      diff.changes[file.filename] = {
        status: normalizeGitHubStatus(file.status),
        diff_lines: parsed.diff_lines,
        hunks: parsed.hunks
      };
    }
    return diff;
  }

  async function fetchMarixTags(repoApi) {
    const tags = await fetchJson(`${repoApi}/tags?per_page=100`);
    const marixTags = tags.filter(tag => String(tag.name || "").startsWith("marix_tag_"));
    logOverview("marix tags listed", { count: marixTags.length });
    return marixTags
      .map(tag => ({ name: tag.name, date: new Date(0) }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  function parsePatch(patch) {
    const diffLines = patch ? patch.split("\n") : [];
    const hunks = diffLines
      .filter(line => line.startsWith("@@"))
      .map(header => ({ header, reason: "" }));
    return {
      diff_lines: diffLines,
      hunks
    };
  }

  function normalizeGitHubStatus(status) {
    const map = {
      added: "A",
      removed: "D",
      renamed: "R",
      modified: "M",
      changed: "M"
    };
    return map[status] || "M";
  }

  function shouldIncludeManifestPath(path) {
    if (!path || isGeneratedPath(path)) return false;
    if (path.split("/").some(part => isExcludedPathPart(part))) return false;
    return isSourcePathName(path);
  }

  function isGeneratedPath(path) {
    return path.endsWith("/manifest.json")
      || path === "manifest.json"
      || path.startsWith("overview/content/")
      || path.startsWith("docs/content/");
  }

  function isExcludedPathPart(part) {
    return [".git", "__pycache__", "node_modules", ".venv", "venv", ".mypy_cache", ".pytest_cache", "target"].includes(part);
  }

  function isHiddenPathName(path) {
    return String(path || "").split("/").some(part => part.startsWith("."));
  }

  function isDesignDocumentPathName(path) {
    return String(path || "").endsWith("/.design.md") || String(path || "").endsWith("/.design.json");
  }

  function isImagePathName(path) {
    const ext = String(path || "").split(".").pop().toLowerCase();
    return Object.prototype.hasOwnProperty.call(IMAGE_MIME_TYPES, ext);
  }

  function getMimeTypeFromPath(path) {
    const ext = String(path || "").split(".").pop().toLowerCase();
    return IMAGE_MIME_TYPES[ext] || "application/octet-stream";
  }

  function getRequestedRef() {
    const params = new URLSearchParams(window.location.search);
    return params.get("ref") || params.get("branch") || "";
  }

  async function fetchBlobData(repoApi, item) {
    if (!item.sha) throw new Error(`blob sha unavailable: ${item.path}`);
    const blob = await fetchJson(`${repoApi}/git/blobs/${encodeURIComponent(item.sha)}`);
    if (blob.encoding !== "base64" || !blob.content) {
      throw new Error(`unsupported blob encoding: ${item.path}`);
    }
    return {
      content: blob.content.replace(/\s/g, ""),
      size: blob.size || item.size || 0
    };
  }

  async function fetchBlobText(repoApi, item) {
    const blob = await fetchBlobData(repoApi, item);
    const binary = atob(blob.content);
    const bytes = Uint8Array.from(binary, char => char.charCodeAt(0));
    return new TextDecoder("utf-8").decode(bytes);
  }

  async function ensureFileContent(path) {
    const entry = (manifest.files || {})[path];
    if (!entry) return null;
    const isImage = isImagePathName(path);
    if (isImage && (entry.base64 || entry.url)) return entry;
    if (!isImage && Object.prototype.hasOwnProperty.call(entry, "content")) return entry;
    if (entry.localHandle) {
      try {
        if (isImage) {
          const file = await entry.localHandle.getFile();
          entry.mime = file.type || getMimeTypeFromPath(path);
          entry.url = URL.createObjectURL(file);
          logOverview("lazy local image content loaded", { path });
        } else {
          entry.content = await readLocalFileText(entry.localHandle);
          logOverview("lazy local file content loaded", { path });
        }
      } catch (e) {
        logOverviewError(`lazy local file content load failed: ${path}`, e);
        entry.content = "[Unable to read file]";
      }
      return entry;
    }
    if (!entry.sha) {
      entry.content = "[Unable to read file]";
      return entry;
    }

    try {
      const repoApi = githubRepoApi || `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}`;
      if (isImage) {
        const blob = await fetchBlobData(repoApi, {
          path,
          sha: entry.sha
        });
        entry.base64 = blob.content;
        entry.mime = getMimeTypeFromPath(path);
        logOverview("lazy image content loaded", { path });
      } else {
        entry.content = await fetchBlobText(repoApi, {
          path,
          sha: entry.sha
        });
        logOverview("lazy file content loaded", { path });
      }
    } catch (e) {
      logOverviewError(`lazy file content load failed: ${path}`, e);
      entry.content = "[Unable to read file]";
    }
    return entry;
  }

  async function readLocalFileText(fileHandle) {
    const file = await fileHandle.getFile();
    return await file.text();
  }

  async function requestLocalReadPermission(handle) {
    const options = { mode: "read" };
    if ((await handle.queryPermission(options)) === "granted") return true;
    return (await handle.requestPermission(options)) === "granted";
  }

  async function assertLocalRootReadable(handle) {
    for await (const _ of handle.entries()) {
      return true;
    }
    throw new Error("local folder is empty");
  }

  async function storeLocalRootHandle(handle) {
    const db = await openLocalSourceDb();
    await writeLocalSourceValue(db, LOCAL_ROOT_HANDLE_KEY, handle);
  }

  async function loadLocalRootHandle() {
    const db = await openLocalSourceDb();
    return await readLocalSourceValue(db, LOCAL_ROOT_HANDLE_KEY);
  }

  async function clearCachedLocalSource() {
    localStorage.removeItem(STORAGE_KEYS.dataSource);
    let db = null;
    try {
      db = await openLocalSourceDb();
      await deleteLocalSourceValue(db, LOCAL_ROOT_HANDLE_KEY);
    } catch (e) {
      logOverviewError("local source cache clear failed", e);
    } finally {
      if (db) db.close();
    }
  }

  function openLocalSourceDb() {
    return new Promise((resolve, reject) => {
      if (!window.indexedDB) {
        reject(new Error("IndexedDB unavailable"));
        return;
      }
      const request = window.indexedDB.open(LOCAL_DB_NAME, 1);
      request.onupgradeneeded = () => {
        request.result.createObjectStore(LOCAL_DB_STORE);
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  }

  function readLocalSourceValue(db, key) {
    return new Promise((resolve, reject) => {
      const request = db.transaction(LOCAL_DB_STORE, "readonly").objectStore(LOCAL_DB_STORE).get(key);
      request.onsuccess = () => resolve(request.result || null);
      request.onerror = () => reject(request.error);
    });
  }

  function writeLocalSourceValue(db, key, value) {
    return new Promise((resolve, reject) => {
      const request = db.transaction(LOCAL_DB_STORE, "readwrite").objectStore(LOCAL_DB_STORE).put(value, key);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  function deleteLocalSourceValue(db, key) {
    return new Promise((resolve, reject) => {
      const request = db.transaction(LOCAL_DB_STORE, "readwrite").objectStore(LOCAL_DB_STORE).delete(key);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  }

  async function fetchJson(url) {
    logOverview("fetch json", { url });
    const resp = await fetch(url, {
      headers: { Accept: "application/vnd.github+json" },
      cache: "no-store"
    });
    if (!resp.ok) throw new Error(`GitHub request failed: ${resp.status}`);
    return await resp.json();
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
    updateToolButton("btn-view-all-files", "viewAllFilesTool", viewAllFiles);
    updateToolButton("btn-view-whole-file", "viewWholeFileTool", viewWholeFile);
    updateDataSourceDependentControls();

    renderWelcome();
    renderMode();
    if (overviewMode === "star") {
      renderModuleDetails(selectedModule || getScopeModule());
      renderStarMap();
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
  }

  function setScopePath(path) {
    scopePath = normalizeScopePath(path);
    localStorage.setItem(STORAGE_KEYS.scopePath, scopePath);
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
    const githubOnlyDiff = activeDataSource === DATA_SOURCE_GITHUB;
    setElementVisible("btn-view-all-files", !githubOnlyDiff);
    setElementVisible("btn-view-whole-file", !githubOnlyDiff);
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
