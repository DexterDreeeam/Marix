"use strict";
  let cachedLocalRootHandle = null;

  async function resolveDataSourceChoice() {
    const cachedSource = getCachedDataSourceKind();
    if (cachedSource === DATA_SOURCE_GITHUB) {
      return { kind: DATA_SOURCE_GITHUB };
    }
    if (cachedSource === DATA_SOURCE_LOCAL) {
      const handle = await resolveCachedLocalHandle();
      if (handle) {
        return { kind: DATA_SOURCE_LOCAL, handle };
      }
      await clearCachedDataSource();
      return promptDataSourceChoice(t("dataSourceLocalMissing"));
    }
    return promptDataSourceChoice("");
  }

  async function resolveCachedLocalHandle() {
    let handle = cachedLocalRootHandle;
    if (!handle) {
      try {
        handle = await loadLocalRootHandle();
      } catch (e) {
        logOverviewError("local source handle load failed", e);
        return null;
      }
    }
    if (!handle) return null;
    let allowed = false;
    try {
      allowed = await requestLocalReadPermission(handle);
    } catch (e) {
      logOverviewError("local source permission check failed", e);
    }
    if (!allowed) {
      await deleteLocalRootHandle();
      return null;
    }
    try {
      await assertLocalRootReadable(handle);
    } catch (e) {
      logOverviewError("local source handle is unreadable", e);
      await deleteLocalRootHandle();
      return null;
    }
    cachedLocalRootHandle = handle;
    return handle;
  }

  function promptDataSourceChoice(message = "") {
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
        cacheDataSourceKind(DATA_SOURCE_GITHUB);
        dialog.classList.add("hidden");
        resolve({ kind: DATA_SOURCE_GITHUB });
      };
      localButton.onclick = async () => {
        if (!window.showDirectoryPicker) {
          error.textContent = t("dataSourceLocalUnsupported");
          return;
        }
        try {
          const handle = await window.showDirectoryPicker({
            id: resolveAliases("{{proj_lower}}-overview-local-root"),
            mode: "read"
          });
          const allowed = await requestLocalReadPermission(handle);
          if (!allowed) throw new Error("local handle permission denied");
          await assertLocalRootReadable(handle);
          await storeLocalRootHandle(handle);
          cacheDataSourceKind(DATA_SOURCE_LOCAL);
          dialog.classList.add("hidden");
          resolve({ kind: DATA_SOURCE_LOCAL, handle });
        } catch (e) {
          logOverviewError("local source selection failed", e);
          error.textContent = t("dataSourceLocalMissing");
        }
      };
    });
  }

  function getCachedDataSourceKind() {
    return normalizeDataSourceKind(localStorage.getItem(STORAGE_KEYS.dataSource));
  }

  function cacheDataSourceKind(kind) {
    const normalized = normalizeDataSourceKind(kind);
    if (!normalized) throw new Error(`unsupported data source: ${kind}`);
    localStorage.setItem(STORAGE_KEYS.dataSource, normalized);
  }

  function normalizeDataSourceKind(kind) {
    const value = String(kind || "").toLowerCase();
    if (value === DATA_SOURCE_GITHUB || value === "remote") return DATA_SOURCE_GITHUB;
    if (value === DATA_SOURCE_LOCAL) return DATA_SOURCE_LOCAL;
    return "";
  }

  async function clearCachedDataSource() {
    localStorage.removeItem(STORAGE_KEYS.dataSource);
    await clearCachedLocalSource();
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
      visibleSourceFiles: tree.filter(item => shouldIncludeVisibleSourcePath(item.path)).length
    });

    let diff = { prev_tag: null, latest_tag: null, changes: {} };
    try {
      diff = await buildLocalDiffFromLatestTag(rootHandle, tree);
      logOverview("local diff loaded from latest tag", {
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

  async function fetchRepositoryTree(repoApi, ref) {
    const treeSha = await fetchRefTreeSha(repoApi, ref);
    logOverview("ref tree resolved", { ref, treeSha });
    const tree = await fetchJson(`${repoApi}/git/trees/${encodeURIComponent(treeSha)}?recursive=1`);
    const files = (tree.tree || [])
      .filter(item => item.type === "blob")
      .filter(item => shouldIncludeManifestPath(item.path));
    const visibleSourceFiles = files.filter(item => shouldIncludeVisibleSourcePath(item.path)).length;
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

  async function fetchRefTreeSha(repoApi, ref) {
    const commit = await fetchJson(`${repoApi}/commits/${encodeURIComponent(ref)}`);
    const treeSha = commit && commit.commit && commit.commit.tree && commit.commit.tree.sha;
    if (!treeSha) throw new Error(`ref tree unavailable: ${ref}`);
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
    const tags = await fetchProjectTags(repoApi);
    const diff = { prev_tag: null, latest_tag: null, changes: {} };
    if (tags.length === 0) return diff;

    const requestedRef = ref || "main";
    const tagNames = tags.map(tag => tag.name);
    const requestedTagIndex = tagNames.indexOf(requestedRef);
    const head = requestedRef;
    let base = "";

    if (requestedTagIndex >= 0) {
      base = requestedTagIndex > 0 ? tagNames[requestedTagIndex - 1] : "";
    } else {
      base = tagNames[tagNames.length - 1];
    }

    diff.prev_tag = base || null;
    diff.latest_tag = head;
    if (!base || base === head) return diff;

    const compare = await fetchJson(`${repoApi}/compare/${encodeURIComponent(base)}...${encodeURIComponent(head)}`);
    for (const file of compare.files || []) {
      if (!shouldIncludeVisibleSourcePath(file.filename)) continue;
      const parsed = parsePatch(file.patch || "");
      diff.changes[file.filename] = {
        status: normalizeGitHubStatus(file.status),
        diff_lines: parsed.diff_lines,
        hunks: parsed.hunks
      };
    }
    return diff;
  }

  function createSyntheticDiffChange(status) {
    return {
      status,
      diff_lines: [],
      hunks: []
    };
  }

  async function fetchProjectTags(repoApi) {
    const tags = await fetchJson(`${repoApi}/tags?per_page=100`);
    const tagPrefix = resolveAliases("{{proj_lower}}_tag_");
    const projectTags = tags.filter(tag => String(tag.name || "").startsWith(tagPrefix));
    logOverview("project tags listed", { count: projectTags.length });
    return projectTags
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
    if (!isSourcePathName(path)) return false;
    if (isDesignDocumentPathName(path)) return !hasHiddenAncestorPathName(path);
    return !isHiddenPathName(path);
  }

  function shouldIncludeVisibleSourcePath(path) {
    return shouldIncludeManifestPath(path) && !isHiddenPathName(path);
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

  function hasHiddenAncestorPathName(path) {
    const parts = String(path || "").split("/");
    return parts.slice(0, -1).some(part => part.startsWith("."));
  }

  function isDesignDocumentPathName(path) {
    return String(path || "").endsWith("/.design.json");
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

  async function fetchJson(url) {
    logOverview("fetch json", { url });
    const resp = await fetch(url, {
      headers: { Accept: "application/vnd.github+json" },
      cache: "no-store"
    });
    if (!resp.ok) throw new Error(`GitHub request failed: ${resp.status}`);
    return await resp.json();
  }
