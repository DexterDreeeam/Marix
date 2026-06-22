"use strict";
  const localRouteHandles = new Map();

  async function resolveDataSourceChoice() {
    const route = getCurrentSourceRoute();
    if (route.kind === DATA_SOURCE_GITHUB) {
      navigateToDataSourceRoute({ kind: DATA_SOURCE_GITHUB });
      return { kind: DATA_SOURCE_GITHUB };
    }
    if (route.kind === DATA_SOURCE_LOCAL) {
      const handle = await resolveLocalRouteHandle(route.localPath);
      if (handle) {
        navigateToDataSourceRoute({ kind: DATA_SOURCE_LOCAL, localPath: route.localPath });
        return { kind: DATA_SOURCE_LOCAL, handle, localPath: route.localPath };
      }
      return promptDataSourceChoice(t("dataSourceLocalMissing"), route);
    }
    return promptDataSourceChoice(route.reason ? t("dataSourceRouteInvalid") : "");
  }

  async function resolveLocalRouteHandle(localPath) {
    let handle = localRouteHandles.get(localPath) || null;
    if (!handle) {
      try {
        handle = await loadLocalRootHandle(localPath);
      } catch (e) {
        logOverviewError("local route handle load failed", e);
        return null;
      }
    }
    if (!handle) return null;
    if (!isLocalHandleCompatibleWithPath(handle, localPath)) {
      await deleteLocalRootHandle(localPath);
      return null;
    }
    let allowed = false;
    try {
      allowed = await requestLocalReadPermission(handle);
    } catch (e) {
      logOverviewError("local route permission check failed", e);
    }
    if (!allowed) {
      await deleteLocalRootHandle(localPath);
      return null;
    }
    try {
      await assertLocalRootReadable(handle);
    } catch (e) {
      logOverviewError("local route handle is unreadable", e);
      await deleteLocalRootHandle(localPath);
      return null;
    }
    localRouteHandles.set(localPath, handle);
    return handle;
  }

  function promptDataSourceChoice(message, route = null) {
    setLoadingVisible(false);
    const dialog = document.getElementById("data-source-dialog");
    const title = document.getElementById("data-source-title");
    const intro = document.getElementById("data-source-intro");
    const githubButton = document.getElementById("btn-data-source-github");
    const localButton = document.getElementById("btn-data-source-local");
    const localPathLabel = document.getElementById("data-source-local-path-label");
    const localPathInput = document.getElementById("data-source-local-path");
    const error = document.getElementById("data-source-error");

    title.textContent = t("dataSourceTitle");
    intro.textContent = t("dataSourceIntro");
    githubButton.textContent = t("dataSourceGithub");
    localButton.textContent = t("dataSourceLocal");
    localPathLabel.textContent = t("dataSourceLocalPathLabel");
    localPathInput.placeholder = t("dataSourceLocalPathPlaceholder");
    localPathInput.value = route && route.kind === DATA_SOURCE_LOCAL
      ? route.localPath
      : localPathInput.value || t("dataSourceLocalPathPlaceholder");
    error.textContent = message || "";
    dialog.classList.remove("hidden");

    return new Promise(resolve => {
      githubButton.onclick = () => {
        navigateToDataSourceRoute({ kind: DATA_SOURCE_GITHUB });
        dialog.classList.add("hidden");
        resolve({ kind: DATA_SOURCE_GITHUB });
      };
      localButton.onclick = async () => {
        if (!window.showDirectoryPicker) {
          error.textContent = t("dataSourceLocalUnsupported");
          return;
        }
        const localPath = normalizeWindowsLocalPath(localPathInput.value);
        if (!localPath) {
          error.textContent = t("dataSourceLocalPathRequired");
          localPathInput.focus();
          return;
        }
        try {
          const handle = await window.showDirectoryPicker({
            id: resolveAliases("{{proj_lower}}-overview-local-root"),
            mode: "read"
          });
          if (!isLocalHandleCompatibleWithPath(handle, localPath)) {
            error.textContent = t("dataSourceLocalPathMismatch");
            return;
          }
          const allowed = await requestLocalReadPermission(handle);
          if (!allowed) throw new Error("local handle permission denied");
          await assertLocalRootReadable(handle);
          await storeLocalRootHandle(localPath, handle);
          navigateToDataSourceRoute({ kind: DATA_SOURCE_LOCAL, localPath });
          dialog.classList.add("hidden");
          resolve({ kind: DATA_SOURCE_LOCAL, handle, localPath });
        } catch (e) {
          logOverviewError("local source selection failed", e);
          error.textContent = t("dataSourceLocalMissing");
        }
      };
    });
  }

  function getCurrentSourceRoute() {
    const split = splitSourceRoutePath();
    const suffix = split.suffixSegments;
    if (suffix.length === 0) return { kind: "picker" };
    const sourceKind = suffix[0].toLowerCase();
    if (sourceKind === DATA_SOURCE_GITHUB && suffix.length === 1) {
      return { kind: DATA_SOURCE_GITHUB };
    }
    if (sourceKind === DATA_SOURCE_LOCAL) {
      const localPath = decodeLocalPathRouteSegments(suffix.slice(1));
      if (localPath) return { kind: DATA_SOURCE_LOCAL, localPath };
      return { kind: "picker", reason: "invalid-local-route" };
    }
    return { kind: "picker", reason: "unknown-source-route" };
  }

  function splitSourceRoutePath() {
    const segments = getDecodedPathSegments(window.location.pathname);
    if ((segments[segments.length - 1] || "").toLowerCase() === "index.html") segments.pop();
    const markerIndex = segments.findIndex(segment => {
      const value = segment.toLowerCase();
      return value === DATA_SOURCE_GITHUB || value === DATA_SOURCE_LOCAL;
    });
    const repoName = String(GITHUB_REPO || "").toLowerCase();
    const baseLength = markerIndex >= 0
      ? markerIndex
      : segments[0] && segments[0].toLowerCase() === repoName
        ? 1
        : segments.length;
    return {
      baseSegments: segments.slice(0, baseLength),
      suffixSegments: segments.slice(baseLength)
    };
  }

  function getDecodedPathSegments(pathname) {
    return String(pathname || "")
      .split("/")
      .filter(Boolean)
      .map(segment => {
        try {
          return decodeURIComponent(segment);
        } catch (e) {
          return "";
        }
      })
      .filter(Boolean);
  }

  function navigateToDataSourceRoute(source) {
    const url = buildDataSourceRouteUrl(source);
    if (window.location.href !== url) window.history.pushState({}, "", url);
  }

  function navigateToSourcePickerRoute() {
    window.location.assign(buildDataSourceRouteUrl({ kind: "picker" }));
  }

  function buildDataSourceRouteUrl(source) {
    const url = new URL(window.location.href);
    const baseSegments = splitSourceRoutePath().baseSegments;
    const routeSegments = source.kind === DATA_SOURCE_GITHUB
      ? [DATA_SOURCE_GITHUB]
      : source.kind === DATA_SOURCE_LOCAL
        ? [DATA_SOURCE_LOCAL, ...encodeLocalPathRouteSegments(source.localPath)]
        : [];
    const fullSegments = [...baseSegments, ...routeSegments];
    const pathBody = fullSegments.map(encodeURIComponent).join("/");
    const trailingSlash = source.kind === DATA_SOURCE_LOCAL || source.kind === "picker";
    url.pathname = pathBody ? `/${pathBody}${trailingSlash ? "/" : ""}` : "/";
    return url.toString();
  }

  function decodeLocalPathRouteSegments(segments) {
    if (!segments || segments.length === 0) return "";
    const decoded = segments;
    const drive = decoded[0];
    if (/^[A-Za-z]$/.test(drive)) {
      const parts = decoded.slice(1).filter(Boolean);
      return `${drive.toUpperCase()}:\\${parts.join("\\")}`;
    }
    if (/^[A-Za-z]:$/.test(drive)) {
      const parts = decoded.slice(1).filter(Boolean);
      return `${drive[0].toUpperCase()}:\\${parts.join("\\")}`;
    }
    if (drive.toLowerCase() === "unc" && decoded.length >= 3) {
      return `\\\\${decoded.slice(1).filter(Boolean).join("\\")}`;
    }
    return "";
  }

  function encodeLocalPathRouteSegments(localPath) {
    const normalized = normalizeWindowsLocalPath(localPath);
    const driveMatch = normalized.match(/^([A-Za-z]):\\?(.*)$/);
    if (driveMatch) {
      const parts = driveMatch[2].split("\\").filter(Boolean);
      return [driveMatch[1].toLowerCase(), ...parts];
    }
    const uncMatch = normalized.match(/^\\\\([^\\]+)\\([^\\]+)(?:\\(.*))?$/);
    if (uncMatch) {
      const rest = (uncMatch[3] || "").split("\\").filter(Boolean);
      return ["unc", uncMatch[1], uncMatch[2], ...rest];
    }
    throw new Error(`Unsupported local path for route: ${localPath}`);
  }

  function normalizeWindowsLocalPath(localPath) {
    let value = String(localPath || "").trim();
    value = value.replace(/^["']|["']$/g, "").replace(/\//g, "\\");
    const driveMatch = value.match(/^([A-Za-z]):\\?(.*)$/);
    if (driveMatch) {
      const parts = driveMatch[2].split("\\").filter(Boolean);
      return `${driveMatch[1].toUpperCase()}:\\${parts.join("\\")}`;
    }
    const uncMatch = value.match(/^\\\\(.+)$/);
    if (uncMatch) {
      const parts = uncMatch[1].split("\\").filter(Boolean);
      return parts.length >= 2 ? `\\\\${parts.join("\\")}` : "";
    }
    return "";
  }

  function isLocalHandleCompatibleWithPath(handle, localPath) {
    const leafName = getLocalPathLeafName(localPath);
    return !leafName || !handle.name || handle.name.toLowerCase() === leafName.toLowerCase();
  }

  function getLocalPathLeafName(localPath) {
    const normalized = normalizeWindowsLocalPath(localPath);
    const parts = normalized.split("\\").filter(part => part && !/^[A-Za-z]:$/.test(part));
    return parts[parts.length - 1] || "";
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
