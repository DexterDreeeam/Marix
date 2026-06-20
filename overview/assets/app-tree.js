"use strict";
  function buildTreeStructure(files) {
    const root = { __children: {}, __files: [] };
    const paths = Object.keys(files).sort();
    for (const p of paths) {
      const parts = p.split("/");
      let node = root;
      for (let i = 0; i < parts.length - 1; i++) {
        if (!node.__children[parts[i]]) {
          node.__children[parts[i]] = { __children: {}, __files: [] };
        }
        node = node.__children[parts[i]];
      }
      node.__files.push({ name: parts[parts.length - 1], path: p });
    }
    return root;
  }

  function renderTree(filter, options = {}) {
    const container = document.getElementById("file-tree");
    container.innerHTML = "";
    const tree = buildTreeStructure(getTreeFiles());
    const filterLower = filter ? filter.toLowerCase() : null;
    renderNode(container, tree, 0, "", filterLower);
    if (!options.skipSync) syncTreeSelectionToStarMapScope();
  }

  function getTreeFiles() {
    if (treeChangedFilesOnly) {
      const changedFiles = {};
      for (const path of getChangedVisiblePaths().sort()) {
        changedFiles[path] = (manifest.files || {})[path] || { size: 0, content: "" };
      }
      return changedFiles;
    }

    const files = getVisibleManifestFiles();
    for (const path of getChangedVisiblePaths().sort()) {
      if (!shouldIncludeVisibleSourcePath(path)) continue;
      files[path] = (manifest.files || {})[path] || { size: 0, content: "" };
    }
    return files;
  }

  function getVisibleManifestFiles() {
    const files = {};
    for (const [path, data] of Object.entries(manifest.files || {})) {
      if (shouldIncludeVisibleSourcePath(path)) files[path] = data;
    }
    return files;
  }

  function collapseAllDirectories() {
    for (const el of document.querySelectorAll(".dir-children")) {
      el.classList.add("collapsed");
    }
    for (const el of document.querySelectorAll(".tree-toggle")) {
      el.classList.add("collapsed");
    }
  }

  function renderNode(parent, node, depth, prefix, filter) {
    for (const dirName of Object.keys(node.__children).sort()) {
      const dirPath = prefix ? `${prefix}/${dirName}` : dirName;
      const child = node.__children[dirName];

      if (filter && !hasMatchingDescendant(child, dirPath, filter)) continue;

      const dirEl = createTreeItem(dirName, depth, true, getTreeDirectoryStatus(child, dirPath), dirPath);
      parent.appendChild(dirEl);

      const childContainer = document.createElement("div");
      childContainer.className = "dir-children";
      parent.appendChild(childContainer);
      dirEl.addEventListener("click", () => {
        if (overviewMode === "star") {
          if (isSelectedStarMapFolder(dirPath)) {
            toggleTreeDirectory(childContainer, dirEl);
          } else {
            openDirectoryModule(dirPath, dirEl);
          }
          return;
        }

        toggleTreeDirectory(childContainer, dirEl);
        openDirectoryChanges(dirPath, dirEl);
      });

      const toggle = dirEl.querySelector(".tree-toggle");
      if (toggle) {
        toggle.addEventListener("click", evt => {
          evt.stopPropagation();
          dirEl.click();
        });
        toggle.addEventListener("dblclick", evt => evt.stopPropagation());
      }

      renderNode(childContainer, child, depth + 1, dirPath, filter);
    }

    for (const file of sortTreeFiles(node.__files)) {
      if (filter && !file.path.toLowerCase().includes(filter) && !file.name.toLowerCase().includes(filter)) continue;
      const el = createTreeItem(file.name, depth, false, getPathChangeStatus(file.path), file.path);
      el.addEventListener("click", () => {
        if (overviewMode === "star") {
          openFileScope(file.path, el, { openPopover: isFocusedStarMapFile(file.path) });
        } else {
          openFile(file.path, el);
        }
      });
      el.addEventListener("dblclick", evt => {
        if (overviewMode === "star") return;
        evt.preventDefault();
        evt.stopPropagation();
        showFilePopover(file.path);
      });
      parent.appendChild(el);
    }
  }

  function sortTreeFiles(files) {
    return files.slice().sort((a, b) => {
      const aRank = getTreeFileChangeRank(a.path);
      const bRank = getTreeFileChangeRank(b.path);
      if (aRank !== bRank) return aRank - bRank;
      return a.name.localeCompare(b.name);
    });
  }

  function getTreeFileChangeRank(path) {
    const status = getPathChangeStatus(path);
    const ranks = { added: 0, modified: 1, renamed: 2, deleted: 3 };
    return ranks[status] ?? 10;
  }

  function getTreeDirectoryStatus(node, dirPath) {
    const folderStatus = getFolderChangeStatus(dirPath);
    if (folderStatus !== "unchanged") return folderStatus;

    const statuses = collectTreeDirectoryFileStatuses(node);
    const changedStatuses = statuses.filter(status => status !== "unchanged");
    if (changedStatuses.length === 0) return "unchanged";
    return statuses.every(status => status === "added") ? "added" : "modified";
  }

  function collectTreeDirectoryFileStatuses(node) {
    const statuses = [];
    for (const file of node.__files) {
      statuses.push(getPathChangeStatus(file.path));
    }
    for (const child of Object.values(node.__children)) {
      statuses.push(...collectTreeDirectoryFileStatuses(child));
    }
    return statuses;
  }

  function isSelectedStarMapFolder(dirPath) {
    return starMapSelection.kind === "module" && starMapSelection.path === dirPath;
  }

  function toggleTreeDirectory(childContainer, dirEl) {
    childContainer.classList.toggle("collapsed");
    const toggle = dirEl.querySelector(".tree-toggle");
    if (toggle) toggle.classList.toggle("collapsed");
  }

  function hasMatchingDescendant(node, prefix, filter) {
    for (const f of node.__files) {
      const fp = `${prefix}/${f.name}`;
      if (fp.toLowerCase().includes(filter) || f.name.toLowerCase().includes(filter)) return true;
    }
    for (const [dn, child] of Object.entries(node.__children)) {
      if (hasMatchingDescendant(child, `${prefix}/${dn}`, filter)) return true;
    }
    return false;
  }

  function syncTreeToModule(modulePath) {
    if (!modulePath) return;
    logStarMapState("sync-tree-module:start", { modulePath });
    if (!findTreeItem(modulePath)) {
      renderTree(document.getElementById("search-input").value.trim(), { skipSync: true });
    }
    expandTreePath(modulePath);
    document.querySelectorAll(".tree-item.active").forEach(el => el.classList.remove("active"));
    const item = findTreeItem(modulePath);
    if (item) {
      item.classList.add("active");
      item.scrollIntoView({ block: "nearest" });
    }
    logStarMapState("sync-tree-module:done", {
      modulePath,
      found: Boolean(item)
    });
  }

  function syncTreeSelectionToStarMapScope() {
    if (overviewMode !== "star") return;
    logStarMapState("sync-tree-selection", { starMapSelection, currentFile, scopePath });
    if (starMapSelection.kind === "file") {
      markTreeSelection(starMapSelection.path);
      return;
    }
    syncTreeToModule(starMapSelection.path || scopePath || SOURCE_ROOT);
  }

  function expandTreePath(path) {
    const parts = String(path || "").split("/").filter(Boolean);
    for (let i = 1; i <= parts.length; i++) {
      const partial = parts.slice(0, i).join("/");
      const item = findTreeItem(partial);
      if (!item) continue;
      const childContainer = item.nextElementSibling;
      if (childContainer && childContainer.classList.contains("dir-children")) {
        childContainer.classList.remove("collapsed");
      }
      const toggle = item.querySelector(".tree-toggle");
      if (toggle) toggle.classList.remove("collapsed");
    }
  }

  function createTreeItem(name, depth, isDir, status, path) {
    const el = document.createElement("div");
    const normalizedStatus = normalizeTreeFileStatus(status);
    el.className = `tree-item${isDir ? " dir" : " file"} ${isDir ? `tree-dir-status-${normalizedStatus}` : `tree-file-status-${normalizedStatus}`}`;
    el.dataset.path = path;

    const indent = document.createElement("span");
    indent.className = "tree-indent";
    indent.style.width = `${depth * 16}px`;
    el.appendChild(indent);

    const icon = document.createElement("span");
    icon.className = isDir ? `tree-icon tree-toggle tree-triangle tree-status-${normalizedStatus}` : `tree-icon tree-status-dot tree-status-${normalizedStatus}`;
    if (isDir) {
      icon.setAttribute("aria-hidden", "true");
    } else {
      icon.setAttribute("aria-hidden", "true");
    }
    el.appendChild(icon);

    const nameEl = document.createElement("span");
    nameEl.className = "tree-name";
    nameEl.textContent = name;
    el.appendChild(nameEl);

    return el;
  }

  function normalizeTreeFileStatus(status) {
    return normalizeChangeStatus(status);
  }
